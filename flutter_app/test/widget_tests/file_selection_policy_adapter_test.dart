library;

import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/platform/file_input_resource_limits.dart';
import 'package:upeg/src/rust/api/tools/file_input_policy.dart';
import 'package:upeg/src/widgets/expanded_modal/file_selection_policy_adapter.dart';

void main() {
  group('FileSelectionPolicy DTO adapter', () {
    test('고정 상한과 같은 maxCount는 경계에서 허용한다', () {
      final dto = FileInputPolicyDto(
        extensions: const <String>[],
        maxCount: maxFileInputCount,
        maxFileBytes: null,
        maxTotalBytes: null,
      );

      expect(fileSelectionPolicyFromDto(dto).maxCount, maxFileInputCount);
    });

    test('생성 DTO의 모든 정책 필드를 hand-written 정책으로 변환한다', () {
      final dto = FileInputPolicyDto(
        extensions: const ['png', 'jpg'],
        maxCount: 3,
        maxFileBytes: BigInt.from(10),
        maxTotalBytes: BigInt.from(20),
      );

      final policy = fileSelectionPolicyFromDto(dto);

      expect(policy.extensions, ['png', 'jpg']);
      expect(policy.maxCount, 3);
      expect(policy.maxFileBytes, 10);
      expect(policy.maxTotalBytes, 20);
    });

    test('Dart 정확 정수 범위를 넘는 유효 u64 제한은 최대 정확 정수로 포화한다', () {
      final coreU64Maximum = BigInt.parse('18446744073709551615');
      final dto = FileInputPolicyDto(
        extensions: const <String>[],
        maxCount: 1,
        maxFileBytes: coreU64Maximum,
        maxTotalBytes: coreU64Maximum,
      );

      final policy = fileSelectionPolicyFromDto(dto);

      expect(policy.maxFileBytes, maxExactFilePolicyInteger);
      expect(policy.maxTotalBytes, maxExactFilePolicyInteger);
    });

    test('음수 byte 제한은 경계에서 거부한다', () {
      final dto = FileInputPolicyDto(
        extensions: const <String>[],
        maxCount: 1,
        maxFileBytes: BigInt.from(-1),
        maxTotalBytes: null,
      );

      expect(() => fileSelectionPolicyFromDto(dto), throwsA(isA<RangeError>()));
    });

    test('0인 maxCount는 경계에서 거부한다', () {
      final dto = FileInputPolicyDto(
        extensions: const <String>[],
        maxCount: 0,
        maxFileBytes: null,
        maxTotalBytes: null,
      );

      expect(() => fileSelectionPolicyFromDto(dto), throwsA(isA<RangeError>()));
    });

    test('고정 상한을 넘는 maxCount는 경계에서 거부한다', () {
      final dto = FileInputPolicyDto(
        extensions: const <String>[],
        maxCount: maxFileInputCount + 1,
        maxFileBytes: null,
        maxTotalBytes: null,
      );

      expect(
        () => fileSelectionPolicyFromDto(dto),
        throwsA(
          isA<RangeError>().having(
            (error) => error.invalidValue,
            '거부된 maxCount',
            maxFileInputCount + 1,
          ),
        ),
      );
    });
  });
}
