import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';

void main() {
  test('errorDetailsText는 details JSON을 들여쓴 텍스트로 보여준다', () {
    // External 실패 봉투는 exit_code와 두 스트림을 details에 담는다.
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
    expect(text!.contains('\n'), isTrue, reason: 'JSON은 들여써서 여러 줄이 된다');
  });

  test('details가 없으면 errorDetailsText는 널이다', () {
    final result = CanonicalToolResult(
      ok: false,
      outputs: const <CanonicalOutputEntry>[],
      error: const CanonicalToolError(code: 'invalid_args', message: 'bad'),
    );

    expect(result.errorDetailsText, isNull);
  });

  test('JSON이 아닌 details는 그대로 보여준다', () {
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

  test('File structuredValue는 typed 인스턴스를 그대로 유지한다', () {
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

  test('canonical JSON을 요청할 때 File을 base64 wire 형식으로 직렬화한다', () {
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
