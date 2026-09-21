library;

import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/platform/file_input_resource_limits.dart';
import 'package:upeg/src/rust/api/tools/file_input_policy.dart';
import 'package:upeg/src/widgets/expanded_modal/file_selection_policy_adapter.dart';

void main() {
  group('FileSelectionPolicy DTO adapter', () {
    test('a_maxCount_equal_to_the_fixed_cap_is_accepted_at_the_boundary', () {
      final dto = FileInputPolicyDto(
        extensions: const <String>[],
        maxCount: maxFileInputCount,
        maxFileBytes: null,
        maxTotalBytes: null,
      );

      expect(fileSelectionPolicyFromDto(dto).maxCount, maxFileInputCount);
    });

    test(
      'every_generated_DTO_policy_field_maps_to_the_hand_written_policy',
      () {
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
      },
    );

    test(
      'a_valid_u64_limit_beyond_the_Dart_exact_integer_range_saturates_to_the_max_exact_integer',
      () {
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
      },
    );

    test('a_negative_byte_limit_is_rejected_at_the_boundary', () {
      final dto = FileInputPolicyDto(
        extensions: const <String>[],
        maxCount: 1,
        maxFileBytes: BigInt.from(-1),
        maxTotalBytes: null,
      );

      expect(() => fileSelectionPolicyFromDto(dto), throwsA(isA<RangeError>()));
    });

    test('a_zero_maxCount_is_rejected_at_the_boundary', () {
      final dto = FileInputPolicyDto(
        extensions: const <String>[],
        maxCount: 0,
        maxFileBytes: null,
        maxTotalBytes: null,
      );

      expect(() => fileSelectionPolicyFromDto(dto), throwsA(isA<RangeError>()));
    });

    test('a_maxCount_above_the_fixed_cap_is_rejected_at_the_boundary', () {
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
            'rejected maxCount',
            maxFileInputCount + 1,
          ),
        ),
      );
    });
  });
}
