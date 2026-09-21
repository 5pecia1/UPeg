library;

import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/platform/file_picker_bridge.dart';
import 'package:upeg/src/rust/api/embed.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/canonical_file_value_codec.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';
import 'package:upeg/src/widgets/expanded_modal/webview_panel.dart';
import 'package:upeg/src/widgets/expanded_modal/file_output_card.dart';
import 'package:url_launcher/url_launcher.dart';

const String _defaultOutputFileName = 'output.bin';
const String _markdownFencePrefix = '```';
const String _markdownHeadingPrefix = '# ';
const String _markdownListPrefix = '- ';

final RegExp _jsonTokenPattern = RegExp(
  r'"(?:\\.|[^"\\])*"|-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?|\b(?:true|false|null)\b',
);

class StructuredOutputBlock extends ConsumerWidget {
  const StructuredOutputBlock({
    required this.field,
    required this.entry,
    required this.tokens,
    required this.writer,
    this.primary = false,
    super.key,
  });

  final OutputFieldDto field;
  final CanonicalOutputEntry entry;
  final UpegTokens tokens;
  final ClipboardWriter writer;
  final bool primary;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final value = entry.value.structuredValue;
    final copyText = formatStructuredOutputValue(field, value);
    final filePickerBridge = ref.watch(filePickerBridgeProvider);
    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              _OutputLabel(
                label: field.label,
                primary: primary,
                tokens: tokens,
              ),
              const Spacer(),
              CopyToClipboardButton(textToCopy: copyText, writer: writer),
            ],
          ),
          const SizedBox(height: 2),
          _StructuredOutputValue(
            fieldType: field.fieldType,
            value: value,
            fallbackText: copyText,
            tokens: tokens,
            filePickerBridge: filePickerBridge,
          ),
        ],
      ),
    );
  }
}

String formatStructuredOutputValue(OutputFieldDto field, Object? value) {
  if (value == null) {
    return 'null';
  }
  final fieldType = field.fieldType;
  if (fieldType is OutputFieldType_Json) {
    return _formatJsonText(value);
  }
  if (fieldType is OutputFieldType_MultiOptions && value is List<Object?>) {
    return value.map(_formatScalarText).join(', ');
  }
  if (fieldType is OutputFieldType_File) {
    return _fileSummary(value) ?? _formatFileFallback(value);
  }
  if (fieldType is OutputFieldType_Multiline) {
    return _formatJsonText(value);
  }
  if (value is String) {
    return value;
  }
  return _formatJsonText(value);
}

class _StructuredOutputValue extends StatelessWidget {
  const _StructuredOutputValue({
    required this.fieldType,
    required this.value,
    required this.fallbackText,
    required this.tokens,
    required this.filePickerBridge,
  });

  final OutputFieldType fieldType;
  final Object? value;
  final String fallbackText;
  final UpegTokens tokens;
  final FilePickerBridge filePickerBridge;

  @override
  Widget build(BuildContext context) {
    final fieldType = this.fieldType;
    if (fieldType is OutputFieldType_Select) {
      final rawValue = value;
      final selectedValue = rawValue is String ? rawValue : fallbackText;
      return Wrap(
        spacing: 6,
        runSpacing: 6,
        children: [
          for (final option in fieldType.options)
            _OutputChip(
              key: Key('structured-output-select-$option'),
              label: option,
              selected: option == selectedValue,
              tokens: tokens,
            ),
        ],
      );
    }

    if (fieldType is OutputFieldType_MultiOptions) {
      final rawValue = value;
      final selected = rawValue is List<Object?>
          ? rawValue.map(_formatScalarText).toList(growable: false)
          : fallbackText
                .split(',')
                .map((part) => part.trim())
                .where((part) => part.isNotEmpty)
                .toList(growable: false);
      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Wrap(
            spacing: 6,
            runSpacing: 6,
            children: [
              for (final option in selected)
                _OutputChip(
                  key: Key('structured-output-multi-$option'),
                  label: option,
                  selected: true,
                  tokens: tokens,
                ),
            ],
          ),
          const SizedBox(height: 4),
          SelectableText(fallbackText, style: _monoStyle(tokens)),
        ],
      );
    }

    if (fieldType is OutputFieldType_Boolean) {
      final boolValue = value == true || fallbackText == 'true';
      return _OutputChip(
        key: Key('structured-output-boolean-$boolValue'),
        label: boolValue ? 'true' : 'false',
        selected: boolValue,
        tokens: tokens,
      );
    }

    if (fieldType is OutputFieldType_FilePath) {
      return SelectableText(
        fallbackText,
        key: const Key('structured-output-file-path'),
        style: _monoStyle(tokens),
      );
    }

    if (fieldType is OutputFieldType_Url) {
      return _UrlOutput(url: fallbackText, tokens: tokens);
    }

    if (fieldType is OutputFieldType_File) {
      return _FileOutput(
        value: value,
        fallbackText: fallbackText,
        tokens: tokens,
        bridge: filePickerBridge,
      );
    }

    if (fieldType is OutputFieldType_EmbeddedView) {
      final rawValue = value;
      final url = rawValue is String && rawValue.isNotEmpty
          ? rawValue
          : fieldType.url;
      return SizedBox(
        height: 320,
        // Modal-inline embed renders inside a ~560 px modal — mobile UA
        // keeps the layout from overflowing.
        child: WebViewPanel(
          resolution: EmbedResolutionDto(url: url),
          userAgent: kMobileUserAgent,
        ),
      );
    }

    if (fieldType is OutputFieldType_Markdown) {
      return _MarkdownOutput(markdown: fallbackText, tokens: tokens);
    }

    if (fieldType is OutputFieldType_Json) {
      return _JsonOutput(value: value, tokens: tokens);
    }

    if (fieldType is OutputFieldType_DateTime) {
      return _DateTimeOutput(value: fallbackText, tokens: tokens);
    }

    return SelectableText(fallbackText, style: _monoStyle(tokens));
  }
}

class _UrlOutput extends ConsumerWidget {
  const _UrlOutput({required this.url, required this.tokens});

  final String url;
  final UpegTokens tokens;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Row(
      children: [
        Expanded(child: SelectableText(url, style: _monoStyle(tokens))),
        const SizedBox(width: 8),
        TextButton.icon(
          key: const Key('structured-output-url-open'),
          icon: const Icon(Icons.open_in_new, size: 16),
          label: Text(t(ref, 'modal.output.open_url')),
          onPressed: () async {
            await launchUrl(
              Uri.parse(url),
              mode: LaunchMode.externalApplication,
            );
          },
        ),
      ],
    );
  }
}

class _MarkdownOutput extends StatelessWidget {
  const _MarkdownOutput({required this.markdown, required this.tokens});

  final String markdown;
  final UpegTokens tokens;

  @override
  Widget build(BuildContext context) {
    final widgets = <Widget>[];
    final codeLines = <String>[];
    var inCodeBlock = false;

    void flushCode() {
      if (codeLines.isEmpty) {
        return;
      }
      widgets.add(
        Container(
          key: const Key('structured-output-markdown-code'),
          width: double.infinity,
          margin: const EdgeInsets.only(top: 4, bottom: 6),
          padding: const EdgeInsets.all(8),
          decoration: BoxDecoration(
            color: tokens.bg,
            border: Border.all(color: tokens.line),
            borderRadius: BorderRadius.circular(UpegSizing.radius1),
          ),
          child: SelectableText(
            codeLines.join('\n'),
            style: _monoStyle(tokens).copyWith(color: tokens.fg2),
          ),
        ),
      );
      codeLines.clear();
    }

    for (final line in markdown.split('\n')) {
      if (line.startsWith(_markdownFencePrefix)) {
        if (inCodeBlock) {
          flushCode();
        }
        inCodeBlock = !inCodeBlock;
        continue;
      }
      if (inCodeBlock) {
        codeLines.add(line);
        continue;
      }
      if (line.startsWith(_markdownHeadingPrefix)) {
        widgets.add(
          Padding(
            padding: const EdgeInsets.only(bottom: 4),
            child: SelectableText(
              line.substring(_markdownHeadingPrefix.length),
              key: const Key('structured-output-markdown-heading'),
              style: TextStyle(
                fontSize: 15,
                fontWeight: FontWeight.w700,
                color: tokens.fg,
              ),
            ),
          ),
        );
        continue;
      }
      if (line.startsWith(_markdownListPrefix)) {
        widgets.add(
          Padding(
            padding: const EdgeInsets.only(bottom: 3),
            child: Row(
              key: const Key('structured-output-markdown-list-item'),
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text('- ', style: TextStyle(color: tokens.fg3)),
                Expanded(
                  child: SelectableText(
                    line.substring(_markdownListPrefix.length),
                    style: TextStyle(fontSize: 12, color: tokens.fg),
                  ),
                ),
              ],
            ),
          ),
        );
        continue;
      }
      if (line.trim().isEmpty) {
        widgets.add(const SizedBox(height: 4));
        continue;
      }
      widgets.add(
        Padding(
          padding: const EdgeInsets.only(bottom: 3),
          child: SelectableText(
            line,
            style: TextStyle(fontSize: 12, color: tokens.fg),
          ),
        ),
      );
    }
    if (inCodeBlock) {
      flushCode();
    }

    return Column(
      key: const Key('structured-output-markdown'),
      crossAxisAlignment: CrossAxisAlignment.start,
      children: widgets,
    );
  }
}

class _JsonOutput extends StatelessWidget {
  const _JsonOutput({required this.value, required this.tokens});

  final Object? value;
  final UpegTokens tokens;

  @override
  Widget build(BuildContext context) {
    final pretty = _formatJsonText(value);
    return SelectableText.rich(
      TextSpan(style: _monoStyle(tokens), children: _jsonSpans(pretty, tokens)),
      key: const Key('structured-output-json'),
    );
  }
}

class _DateTimeOutput extends StatelessWidget {
  const _DateTimeOutput({required this.value, required this.tokens});

  final String value;
  final UpegTokens tokens;

  @override
  Widget build(BuildContext context) {
    final parsed = DateTime.tryParse(value);
    final human = parsed == null
        ? 'invalid datetime'
        : '${_datePart(parsed.toUtc())} ${_timePart(parsed.toUtc())} UTC';
    return Column(
      key: const Key('structured-output-datetime'),
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SelectableText(value, style: _monoStyle(tokens)),
        const SizedBox(height: 3),
        Text(
          human,
          key: const Key('structured-output-datetime-human'),
          style: TextStyle(fontSize: 11, color: tokens.fg3),
        ),
      ],
    );
  }
}

class _FileOutput extends StatelessWidget {
  const _FileOutput({
    required this.value,
    required this.fallbackText,
    required this.tokens,
    required this.bridge,
  });

  final Object? value;
  final String fallbackText;
  final UpegTokens tokens;
  final FilePickerBridge bridge;

  @override
  Widget build(BuildContext context) {
    final file = _canonicalFile(value);
    final summary = file == null
        ? fallbackText
        : canonicalFileValueSummary(file);
    final bytes = _fileBytes(file);
    final name = _fileName(file);
    return FileOutputCard(
      name: name,
      summary: summary,
      bytes: bytes,
      mime: file?.mime,
      bridge: bridge,
    );
  }
}

class _OutputChip extends StatelessWidget {
  const _OutputChip({
    required this.label,
    required this.selected,
    required this.tokens,
    super.key,
  });

  final String label;
  final bool selected;
  final UpegTokens tokens;

  @override
  Widget build(BuildContext context) {
    final bg = selected ? tokens.accent : tokens.surface2;
    final fg = selected ? tokens.onAccent : tokens.fg2;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
      decoration: BoxDecoration(
        color: bg,
        border: Border.all(color: selected ? tokens.accent : tokens.line),
        borderRadius: BorderRadius.circular(UpegSizing.radius1),
      ),
      child: Text(
        label,
        style: TextStyle(
          fontFamily: upegMonoFontFamily,
          fontFamilyFallback: upegMonoFontFamilyFallback,
          fontSize: 11,
          color: fg,
          fontWeight: selected ? FontWeight.w700 : FontWeight.w500,
        ),
      ),
    );
  }
}

class _OutputLabel extends StatelessWidget {
  const _OutputLabel({
    required this.label,
    required this.primary,
    required this.tokens,
  });

  final String label;
  final bool primary;
  final UpegTokens tokens;

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(
          label,
          style: TextStyle(
            fontFamily: upegMonoFontFamily,
            fontFamilyFallback: upegMonoFontFamilyFallback,
            fontSize: 10,
            color: tokens.fg4,
          ),
        ),
        if (primary) ...[
          const SizedBox(width: 6),
          Text(
            'primary',
            style: TextStyle(
              fontFamily: upegMonoFontFamily,
              fontFamilyFallback: upegMonoFontFamilyFallback,
              fontSize: 9,
              color: tokens.accent,
            ),
          ),
        ],
      ],
    );
  }
}

String _formatScalarText(Object? value) {
  if (value == null) {
    return 'null';
  }
  if (value is String) {
    return value;
  }
  return jsonEncode(value);
}

String _formatJsonText(Object? value) {
  if (value is String) {
    return value;
  }
  return const JsonEncoder.withIndent('  ').convert(value);
}

List<TextSpan> _jsonSpans(String pretty, UpegTokens tokens) {
  final spans = <TextSpan>[];
  var offset = 0;
  for (final match in _jsonTokenPattern.allMatches(pretty)) {
    if (match.start > offset) {
      spans.add(TextSpan(text: pretty.substring(offset, match.start)));
    }
    final token = match.group(0)!;
    spans.add(TextSpan(text: token, style: _jsonTokenStyle(token, tokens)));
    offset = match.end;
  }
  if (offset < pretty.length) {
    spans.add(TextSpan(text: pretty.substring(offset)));
  }
  return spans;
}

TextStyle _jsonTokenStyle(String token, UpegTokens tokens) {
  if (token.startsWith('"')) {
    return TextStyle(color: tokens.accent);
  }
  if (token == 'true' || token == 'false') {
    return TextStyle(color: tokens.warn);
  }
  if (token == 'null') {
    return TextStyle(color: tokens.fg4);
  }
  return TextStyle(color: tokens.fg2);
}

TextStyle _monoStyle(UpegTokens tokens) {
  return TextStyle(
    fontFamily: upegMonoFontFamily,
    fontFamilyFallback: upegMonoFontFamilyFallback,
    fontSize: 11,
    color: tokens.fg,
  );
}

String? _fileSummary(Object? value) {
  final file = _canonicalFile(value);
  return file == null ? null : canonicalFileValueSummary(file);
}

String _formatFileFallback(Object? value) {
  if (value is CanonicalFileValue) {
    return _formatJsonText(canonicalFileValueToJson(value));
  }
  return _formatJsonText(value);
}

String _fileName(CanonicalFileValue? file) {
  final name = file?.name;
  return name == null || name.isEmpty ? _defaultOutputFileName : name;
}

Uint8List? _fileBytes(CanonicalFileValue? file) {
  return switch (file?.content) {
    CanonicalFileContent_Bytes(:final bytes) => bytes,
    _ => null,
  };
}

CanonicalFileValue? _canonicalFile(Object? value) {
  if (value is CanonicalFileValue) return value;
  try {
    return canonicalFileValueFromJson(value);
  } on CanonicalFileValueCodecException {
    return null;
  }
}

String _datePart(DateTime value) {
  final month = _twoDigits(value.month);
  final day = _twoDigits(value.day);
  return '${value.year}-$month-$day';
}

String _timePart(DateTime value) {
  final hour = _twoDigits(value.hour);
  final minute = _twoDigits(value.minute);
  return '$hour:$minute';
}

String _twoDigits(int value) => value.toString().padLeft(2, '0');
