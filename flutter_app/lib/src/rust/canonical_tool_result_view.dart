import 'dart:convert';

import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/canonical_file_value_codec.dart';

extension CanonicalToolResultView on CanonicalToolResult {
  CanonicalOutputEntry? get primaryOutput {
    final primaryId = primaryOutputId;
    if (primaryId != null) {
      for (final output in outputs) {
        if (output.id == primaryId) return output;
      }
    }
    return outputs.isEmpty ? null : outputs.first;
  }

  String? get errorMessage => error?.message;

  /// Human-readable rendering of `error.details`, or `null` when the
  /// failure carries none.
  ///
  /// The External invoker packs a failing command's exit code and both
  /// captured streams in there, which is the only place the diagnostics
  /// exist — `error.message` is just the summary line. JSON is
  /// re-encoded with indentation so a compiler log stays readable in a
  /// monospace block; anything that is not JSON is shown verbatim.
  String? get errorDetailsText {
    final details = error?.details;
    if (details == null || details.isEmpty) return null;
    final decoded = _decodeJsonOrText(details);
    if (decoded is String) return decoded;
    return const JsonEncoder.withIndent('  ').convert(decoded);
  }

  String get primaryOutputText => primaryOutput?.value.displayText ?? '';

  bool get hasDisplayContent =>
      outputs.isNotEmpty || (errorMessage != null && errorMessage!.isNotEmpty);

  Map<String, Object?> get structuredValues => <String, Object?>{
    for (final output in outputs) output.id: output.value.structuredValue,
  };

  Map<String, Object?> get jsonValues => <String, Object?>{
    for (final output in outputs) output.id: output.value.jsonValue,
  };

  List<CanonicalOutputDisplayRow> displayRows(
    List<OutputFieldDto> outputFields,
  ) {
    final fieldsById = <String, OutputFieldDto>{
      for (final field in outputFields) field.key: field,
    };
    final primaryId = primaryOutput?.id;
    return <CanonicalOutputDisplayRow>[
      for (final output in outputs)
        CanonicalOutputDisplayRow(
          id: output.id,
          label: fieldsById[output.id]?.label ?? output.label ?? output.id,
          value: output.value.displayText,
          isPrimary: output.id == primaryId,
          entry: output,
          field: fieldsById[output.id],
        ),
    ];
  }

  String snackbarText(String okFallback) {
    if (!ok) return errorMessage ?? 'dispatch failed';
    final text = primaryOutputText;
    return text.isEmpty ? okFallback : text;
  }

  String canonicalJsonText() {
    return jsonEncode(<String, Object?>{
      'ok': ok,
      'primary_output_id': primaryOutputId,
      'outputs': outputs
          .map(
            (output) => <String, Object?>{
              'id': output.id,
              'label': output.label,
              'kind': output.kind,
              'value': output.value.jsonValue,
            },
          )
          .toList(growable: false),
      if (error != null)
        'error': <String, Object?>{
          'code': error!.code,
          'message': error!.message,
          if (error!.details != null)
            'details': _decodeJsonOrText(error!.details!),
        },
    });
  }
}

class CanonicalOutputDisplayRow {
  const CanonicalOutputDisplayRow({
    required this.id,
    required this.label,
    required this.value,
    required this.isPrimary,
    required this.entry,
    this.field,
  });

  final String id;
  final String label;
  final String value;
  final bool isPrimary;
  final CanonicalOutputEntry entry;
  final OutputFieldDto? field;
}

extension CanonicalOutputValueView on CanonicalOutputValue {
  Object? get structuredValue {
    return switch (this) {
      CanonicalOutputValue_String(:final value) => value,
      CanonicalOutputValue_Number(:final value) => value,
      CanonicalOutputValue_Integer(:final value) => value,
      CanonicalOutputValue_Boolean(:final value) => value,
      CanonicalOutputValue_Options(:final value) => value,
      CanonicalOutputValue_MultiOptions(:final value) => value,
      CanonicalOutputValue_Markdown(:final value) => value,
      CanonicalOutputValue_Json(:final value) => _decodeJsonOrText(value),
      CanonicalOutputValue_DateTime(:final value) => value,
      CanonicalOutputValue_FilePath(:final value) => value,
      CanonicalOutputValue_Url(:final value) => value,
      CanonicalOutputValue_File(:final value) => value,
      CanonicalOutputValue_EmbeddedView(:final value) => value,
    };
  }

  Object? get jsonValue {
    return switch (this) {
      CanonicalOutputValue_File(:final value) => canonicalFileValueToJson(
        value,
      ),
      _ => structuredValue,
    };
  }

  String get displayText {
    return switch (this) {
      CanonicalOutputValue_File(:final value) => canonicalFileValueSummary(
        value,
      ),
      _ => _formatStructuredValue(structuredValue),
    };
  }
}

String canonicalFileValueSummary(CanonicalFileValue file) {
  final parts = <String>[file.name.isEmpty ? 'file' : file.name];
  switch (file.content) {
    case CanonicalFileContent_Directory(:final entries):
      parts.add('directory');
      parts.add('${entries.length} entries');
    case CanonicalFileContent_Bytes(:final bytes):
      final mime = file.mime;
      if (mime != null && mime.isNotEmpty) parts.add(mime);
      parts.add('${bytes.length} bytes');
  }
  return parts.join(' · ');
}

Object? _decodeJsonOrText(String text) {
  try {
    return jsonDecode(text);
  } on FormatException {
    return text;
  }
}

String _formatStructuredValue(Object? value) {
  if (value == null) return '—';
  if (value is bool) return value ? 'true' : 'false';
  if (value is int) return value.toString();
  if (value is double) {
    if ((value - value.truncate()).abs() < 1e-9) {
      return value.toInt().toString();
    }
    return value.toString();
  }
  if (value is String) return value;
  return jsonEncode(value);
}
