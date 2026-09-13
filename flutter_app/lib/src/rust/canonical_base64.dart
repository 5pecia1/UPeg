library;

const int _base64CharactersPerBlock = 4;
const int _decodedBytesPerBlock = 3;
const int _singlePaddingCharacter = 1;
const int _doublePaddingCharacters = 2;
const int _singlePaddingTailMask = 0x03;
const int _doublePaddingTailMask = 0x0f;

const int _uppercaseA = 0x41;
const int _uppercaseZ = 0x5a;
const int _lowercaseA = 0x61;
const int _lowercaseZ = 0x7a;
const int _digitZero = 0x30;
const int _digitNine = 0x39;
const int _plus = 0x2b;
const int _slash = 0x2f;
const int _padding = 0x3d;

const int _lowercaseAlphabetOffset = 26;
const int _digitAlphabetOffset = 52;
const int _plusAlphabetValue = 62;
const int _slashAlphabetValue = 63;

/// Returns the decoded byte length when [encoded] has a padded base64 shape.
///
/// This constant-time shape check intentionally does not scan the alphabet.
/// Callers can reject oversized payloads before either a linear validation
/// pass or decoded-byte allocation.
int? paddedBase64DecodedLength(String encoded) {
  if (encoded.isEmpty) return 0;
  if (encoded.length % _base64CharactersPerBlock != 0) return null;

  final paddingCount = _paddingCount(encoded);
  return encoded.length ~/ _base64CharactersPerBlock * _decodedBytesPerBlock -
      paddingCount;
}

/// Validates padded RFC 4648 standard base64 without a quantified RegExp.
///
/// The scan is iterative and uses constant stack space. Canonical tail bits
/// are checked directly, so callers do not need to decode and re-encode.
bool isCanonicalPaddedBase64(String encoded) {
  if (encoded.isEmpty) return true;
  if (encoded.length % _base64CharactersPerBlock != 0) return false;

  final paddingCount = _paddingCount(encoded);
  final dataLength = encoded.length - paddingCount;
  for (var index = 0; index < dataLength; index += 1) {
    if (_base64Value(encoded.codeUnitAt(index)) == null) return false;
  }
  for (var index = dataLength; index < encoded.length; index += 1) {
    if (encoded.codeUnitAt(index) != _padding) return false;
  }

  if (paddingCount == _singlePaddingCharacter) {
    final tailValue = _base64Value(encoded.codeUnitAt(dataLength - 1));
    return tailValue != null && tailValue & _singlePaddingTailMask == 0;
  }
  if (paddingCount == _doublePaddingCharacters) {
    final tailValue = _base64Value(encoded.codeUnitAt(dataLength - 1));
    return tailValue != null && tailValue & _doublePaddingTailMask == 0;
  }
  return true;
}

int _paddingCount(String encoded) {
  if (encoded.codeUnitAt(encoded.length - 1) != _padding) return 0;
  return encoded.codeUnitAt(encoded.length - 2) == _padding
      ? _doublePaddingCharacters
      : _singlePaddingCharacter;
}

int? _base64Value(int character) {
  if (character >= _uppercaseA && character <= _uppercaseZ) {
    return character - _uppercaseA;
  }
  if (character >= _lowercaseA && character <= _lowercaseZ) {
    return character - _lowercaseA + _lowercaseAlphabetOffset;
  }
  if (character >= _digitZero && character <= _digitNine) {
    return character - _digitZero + _digitAlphabetOffset;
  }
  if (character == _plus) return _plusAlphabetValue;
  if (character == _slash) return _slashAlphabetValue;
  return null;
}
