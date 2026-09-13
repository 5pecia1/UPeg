part of 'canonical_file_value_codec.dart';

// The constants below restate the canonical `File` output budget declared
// once in Rust at `upeg-core/src/input/file_budget.rs`. There is no shared
// build-time source between Dart and Rust, so these numbers are pinned — not
// merely mirrored — by `upeg-core/tests/file_budget_cross_language_pin.rs`,
// which reads this file by path and fails the build if it drifts from Rust.

const int _bytesPerMebibyte = 1024 * 1024;

/// Maximum decoded bytes accepted across one canonical file tree.
const int canonicalFileMaximumRawBytes = 64 * _bytesPerMebibyte;

/// Maximum nodes accepted across one canonical file tree.
const int canonicalFileMaximumNodes = 128;

/// Maximum UTF-8 bytes used by names and MIME values in one file tree.
const int canonicalFileMaximumMetadataBytes = 16 * 1024;

/// Maximum file nodes allowed on one root-to-leaf path (root depth is 1).
const int canonicalFileMaximumNestingDepth = 64;

enum CanonicalFileValueCodecErrorCode {
  invalidBase64,
  maximumRawBytesExceeded,
  maximumNodesExceeded,
  maximumMetadataBytesExceeded,
  maximumNestingDepthExceeded,
}

final class CanonicalFileValueCodecException implements Exception {
  const CanonicalFileValueCodecException(
    this.code,
    this.message, {
    this.actualBytes,
    this.limitBytes,
    this.actualNodes,
    this.limitNodes,
  });

  final CanonicalFileValueCodecErrorCode code;
  final String message;
  final int? actualBytes;
  final int? limitBytes;
  final int? actualNodes;
  final int? limitNodes;

  @override
  String toString() => 'CanonicalFileValueCodecException: $message';
}

final class _CanonicalFileTreeBudget {
  int _rawBytes = 0;
  int _nodes = 0;
  int _metadataBytes = 0;

  void addNode() {
    final actualNodes = _checkedTotal(
      current: _nodes,
      amount: 1,
      limit: canonicalFileMaximumNodes,
      onExceeded: (actual) => CanonicalFileValueCodecException(
        CanonicalFileValueCodecErrorCode.maximumNodesExceeded,
        'Canonical file tree has $actual nodes; the limit is '
        '$canonicalFileMaximumNodes.',
        actualNodes: actual,
        limitNodes: canonicalFileMaximumNodes,
      ),
    );
    _nodes = actualNodes;
  }

  void addRawBytes(int byteLength) {
    final actualBytes = _checkedTotal(
      current: _rawBytes,
      amount: byteLength,
      limit: canonicalFileMaximumRawBytes,
      onExceeded: (actual) => CanonicalFileValueCodecException(
        CanonicalFileValueCodecErrorCode.maximumRawBytesExceeded,
        'Canonical file raw bytes are $actual bytes; the limit is '
        '$canonicalFileMaximumRawBytes.',
        actualBytes: actual,
        limitBytes: canonicalFileMaximumRawBytes,
      ),
    );
    _rawBytes = actualBytes;
  }

  void addMetadata(String value) {
    for (final rune in value.runes) {
      final runeBytes = switch (rune) {
        <= 0x7f => 1,
        <= 0x7ff => 2,
        <= 0xffff => 3,
        _ => 4,
      };
      final actualBytes = _checkedTotal(
        current: _metadataBytes,
        amount: runeBytes,
        limit: canonicalFileMaximumMetadataBytes,
        onExceeded: (actual) => CanonicalFileValueCodecException(
          CanonicalFileValueCodecErrorCode.maximumMetadataBytesExceeded,
          'Canonical file metadata is larger than '
          '$canonicalFileMaximumMetadataBytes UTF-8 bytes.',
          actualBytes: actual,
          limitBytes: canonicalFileMaximumMetadataBytes,
        ),
      );
      _metadataBytes = actualBytes;
    }
  }
}

typedef _BudgetExceeded = CanonicalFileValueCodecException Function(int actual);

int _checkedTotal({
  required int current,
  required int amount,
  required int limit,
  required _BudgetExceeded onExceeded,
}) {
  if (amount < 0 || current > limit || amount > limit - current) {
    throw onExceeded(current + amount);
  }
  return current + amount;
}

bool _preflightJsonFileTree(Object? value) {
  return _scanJsonFile(
    value,
    budget: _CanonicalFileTreeBudget(),
    currentDepth: 1,
  );
}

bool _scanJsonFile(
  Object? value, {
  required _CanonicalFileTreeBudget budget,
  required int currentDepth,
}) {
  _ensureDepth(currentDepth);
  if (value is! Map<String, Object?> || !_onlyKnownKeys(value, _fileKeys)) {
    return false;
  }
  final name = value[_nameKey];
  final isDirectory = value[_isDirectoryKey];
  final mime = value[_mimeKey];
  final content = value[_contentKey];
  if (name is! String || isDirectory is! bool) return false;
  if (!value.containsKey(_contentKey)) return false;
  if (value.containsKey(_mimeKey) && mime is! String) return false;
  if (content is! Map<String, Object?>) return false;

  budget
    ..addNode()
    ..addMetadata(name);
  if (mime case final String mimeValue) budget.addMetadata(mimeValue);

  return switch (content[_kindKey]) {
    _bytesKind when !isDirectory => _scanJsonBytes(content, budget),
    _directoryKind when isDirectory => _scanJsonDirectory(
      content,
      budget: budget,
      currentDepth: currentDepth,
    ),
    _ => false,
  };
}

bool _scanJsonBytes(
  Map<String, Object?> content,
  _CanonicalFileTreeBudget budget,
) {
  if (!_hasExactKeys(content, _bytesContentKeys)) return false;
  final encodedBytes = content[_bytesKey];
  if (encodedBytes is! String) return false;
  final decodedLength = paddedBase64DecodedLength(encodedBytes);
  if (decodedLength == null) _throwInvalidBase64();
  budget.addRawBytes(decodedLength);
  if (!isCanonicalPaddedBase64(encodedBytes)) _throwInvalidBase64();
  return true;
}

bool _scanJsonDirectory(
  Map<String, Object?> content, {
  required _CanonicalFileTreeBudget budget,
  required int currentDepth,
}) {
  if (!_hasExactKeys(content, _directoryContentKeys)) return false;
  final entries = content[_entriesKey];
  if (entries is! List<Object?>) return false;
  for (final entry in entries) {
    if (!_scanJsonFile(entry, budget: budget, currentDepth: currentDepth + 1)) {
      return false;
    }
  }
  return true;
}

void _preflightCanonicalFileTree(CanonicalFileValue value) {
  _scanCanonicalFile(
    value,
    budget: _CanonicalFileTreeBudget(),
    currentDepth: 1,
  );
}

void _scanCanonicalFile(
  CanonicalFileValue value, {
  required _CanonicalFileTreeBudget budget,
  required int currentDepth,
}) {
  _ensureDepth(currentDepth);
  budget
    ..addNode()
    ..addMetadata(value.name);
  if (value.mime case final String mimeValue) budget.addMetadata(mimeValue);

  switch (value.content) {
    case CanonicalFileContent_Bytes(:final bytes):
      budget.addRawBytes(bytes.length);
    case CanonicalFileContent_Directory(:final entries):
      for (final entry in entries) {
        _scanCanonicalFile(
          entry,
          budget: budget,
          currentDepth: currentDepth + 1,
        );
      }
  }
}

void _ensureDepth(int currentDepth) {
  if (currentDepth <= canonicalFileMaximumNestingDepth) return;
  throw const CanonicalFileValueCodecException(
    CanonicalFileValueCodecErrorCode.maximumNestingDepthExceeded,
    'Canonical file nesting exceeds the maximum depth of '
    '$canonicalFileMaximumNestingDepth.',
  );
}
