library;

import 'dart:convert';

import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/canonical_file_value_codec.dart';

const String _invalidFileOutputErrorCode = 'invalid_file_output';

/// Rebuild a [CanonicalToolResult] from the daemon's JSON so the PWA
/// renders remote outputs through the exact same path as an in-process run.
CanonicalToolResult decodeAttachCanonicalResult(Map<Object?, Object?> json) {
  final outputs = <CanonicalOutputEntry>[];
  final rawOutputs = json['outputs'];
  if (rawOutputs is List) {
    for (final raw in rawOutputs) {
      if (raw is! Map) continue;
      final decoded = _decodeOutputEntry(raw);
      final outputError = decoded.error;
      if (outputError != null) {
        return CanonicalToolResult(
          ok: false,
          primaryOutputId: null,
          outputs: const <CanonicalOutputEntry>[],
          error: outputError,
        );
      }
      outputs.add(decoded.entry!);
    }
  }

  CanonicalToolError? error;
  final rawError = json['error'];
  if (rawError is Map) {
    error = CanonicalToolError(
      code: '${rawError['code'] ?? 'error'}',
      message: '${rawError['message'] ?? ''}',
      details: rawError['details'] == null
          ? null
          : _asJsonText(rawError['details']),
    );
  }
  final primaryOutputId = json['primary_output_id'];
  return CanonicalToolResult(
    ok: json['ok'] == true,
    primaryOutputId: primaryOutputId is String ? primaryOutputId : null,
    outputs: outputs,
    error: error,
  );
}

({CanonicalOutputEntry? entry, CanonicalToolError? error}) _decodeOutputEntry(
  Map<Object?, Object?> raw,
) {
  final id = '${raw['id'] ?? ''}';
  final label = raw['label'];
  final kind = '${raw['kind'] ?? 'string'}';
  final value = _decodeCanonicalValue(kind, raw['value']);
  if (value == null) {
    return (
      entry: null,
      error: const CanonicalToolError(
        code: _invalidFileOutputErrorCode,
        message: '호스트 File 출력 형식 또는 크기 예산이 올바르지 않습니다',
      ),
    );
  }
  return (
    entry: CanonicalOutputEntry(
      id: id,
      label: label is String ? label : null,
      kind: kind,
      value: value,
    ),
    error: null,
  );
}

/// Map a canonical `(kind, value)` pair to the sealed output value.
///
/// `integer` shares the number carrier because the common formatter preserves
/// whole-number display. `file` is promoted directly to the typed carrier so
/// renderers reuse the decoded byte buffer instead of parsing JSON again.
CanonicalOutputValue? _decodeCanonicalValue(String kind, Object? value) {
  switch (kind) {
    case 'number':
    case 'integer':
      final n = value is num ? value : num.tryParse('$value') ?? 0;
      return CanonicalOutputValue.number(value: n.toDouble());
    case 'boolean':
      return CanonicalOutputValue.boolean(value: value == true);
    case 'multi_options':
      return CanonicalOutputValue.multiOptions(
        value: <String>[
          if (value is List)
            for (final item in value) '$item',
        ],
      );
    case 'markdown':
      return CanonicalOutputValue.markdown(value: _asText(value));
    case 'json':
      return CanonicalOutputValue.json(value: _asJsonText(value));
    case 'file':
      try {
        final file = canonicalFileValueFromJson(value);
        return file == null ? null : CanonicalOutputValue.file(value: file);
      } on CanonicalFileValueCodecException {
        return null;
      }
    case 'date_time':
      return CanonicalOutputValue.dateTime(value: _asText(value));
    case 'file_path':
      return CanonicalOutputValue.filePath(value: _asText(value));
    case 'url':
      return CanonicalOutputValue.url(value: _asText(value));
    case 'options':
      return CanonicalOutputValue.options(value: _asText(value));
    case 'embedded_view':
      return CanonicalOutputValue.embeddedView(value: _asText(value));
    case 'string':
    default:
      return CanonicalOutputValue.string(value: _asText(value));
  }
}

String _asText(Object? value) {
  if (value == null) return '';
  if (value is String) return value;
  return _asJsonText(value);
}

String _asJsonText(Object? value) {
  if (value is String) return value;
  return jsonEncode(value);
}
