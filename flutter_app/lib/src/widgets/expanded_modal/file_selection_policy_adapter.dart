library;

import 'package:upeg/src/platform/file_input_resource_limits.dart';
import 'package:upeg/src/rust/api/tools/file_input_policy.dart';
import 'package:upeg/src/widgets/expanded_modal/file_selection_assembler.dart';

const int maxExactFilePolicyInteger = 9007199254740991;
final BigInt _maximumPolicyBytes = BigInt.from(maxExactFilePolicyInteger);
const String _maxCountField = 'maxCount';
const String _maxFileBytesField = 'maxFileBytes';
const String _maxTotalBytesField = 'maxTotalBytes';
const String _positiveCountMessage = 'maxCount must be positive.';
const String _maximumCountMessage =
    'maxCount must not exceed $maxFileInputCount.';
const String _byteRangeMessage = 'Byte limits must be non-negative.';

FileSelectionPolicy fileSelectionPolicyFromDto(FileInputPolicyDto dto) {
  if (dto.maxCount < 1) {
    throw RangeError.value(dto.maxCount, _maxCountField, _positiveCountMessage);
  }
  if (dto.maxCount > maxFileInputCount) {
    throw RangeError.value(dto.maxCount, _maxCountField, _maximumCountMessage);
  }
  return FileSelectionPolicy(
    extensions: List<String>.unmodifiable(dto.extensions),
    maxCount: dto.maxCount,
    maxFileBytes: _policyBytes(dto.maxFileBytes, _maxFileBytesField),
    maxTotalBytes: _policyBytes(dto.maxTotalBytes, _maxTotalBytesField),
  );
}

int? _policyBytes(BigInt? value, String fieldName) {
  if (value == null) return null;
  if (value < BigInt.zero) {
    throw RangeError('$fieldName: $_byteRangeMessage');
  }
  if (value > _maximumPolicyBytes) return maxExactFilePolicyInteger;
  return value.toInt();
}
