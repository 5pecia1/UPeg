import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';

void main() {
  test('errorDetailsText renders details JSON as indented text', () {
    // The External failure envelope packs exit_code and both streams into details.
    final result = CanonicalToolResult(
      ok: false,
      outputs: const <CanonicalOutputEntry>[],
      error: const CanonicalToolError(
        code: 'tool_error',
        message: '`cargo` exited with code 1',
        details: '{"exit_code":1,"stdout":"warning: unused","stderr":""}',
      ),
    );

    final text = result.errorDetailsText;
    expect(text, isNotNull);
    expect(text, contains('"exit_code": 1'));
    expect(text, contains('warning: unused'));
    expect(
      text!.contains('\n'),
      isTrue,
      reason: 'indented JSON spans multiple lines',
    );
  });

  test('errorDetailsText is null when details are absent', () {
    final result = CanonicalToolResult(
      ok: false,
      outputs: const <CanonicalOutputEntry>[],
      error: const CanonicalToolError(code: 'invalid_args', message: 'bad'),
    );

    expect(result.errorDetailsText, isNull);
  });

  test('non-JSON details are shown verbatim', () {
    final result = CanonicalToolResult(
      ok: false,
      outputs: const <CanonicalOutputEntry>[],
      error: const CanonicalToolError(
        code: 'tool_error',
        message: 'boom',
        details: 'plain text detail',
      ),
    );

    expect(result.errorDetailsText, 'plain text detail');
  });

  test('File structuredValue keeps the typed instance as-is', () {
    final file = CanonicalFileValue(
      name: 'report.bin',
      isDir: false,
      content: CanonicalFileContent.bytes(
        bytes: Uint8List.fromList(<int>[1, 2, 3]),
      ),
    );
    final output = CanonicalOutputValue.file(value: file);

    expect(identical(output.structuredValue, file), isTrue);
    expect(output.displayText, 'report.bin · 3 bytes');
  });

  test('canonical JSON request serializes File in base64 wire format', () {
    final result = CanonicalToolResult(
      ok: true,
      primaryOutputId: 'file',
      outputs: <CanonicalOutputEntry>[
        CanonicalOutputEntry(
          id: 'file',
          label: 'File',
          kind: 'file',
          value: CanonicalOutputValue.file(
            value: CanonicalFileValue(
              name: 'report.bin',
              isDir: false,
              content: CanonicalFileContent.bytes(
                bytes: Uint8List.fromList(<int>[1, 2, 3]),
              ),
            ),
          ),
        ),
      ],
    );

    final json = jsonDecode(result.canonicalJsonText()) as Map<String, Object?>;
    final outputs = json['outputs']! as List<Object?>;
    final output = outputs.single! as Map<String, Object?>;
    final file = output['value']! as Map<String, Object?>;
    final content = file['content']! as Map<String, Object?>;

    expect(content['bytes'], 'AQID');
  });
}
