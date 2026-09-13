library;

import 'dart:convert';
import 'dart:typed_data';

import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/canonical_base64.dart';

part 'canonical_file_value_budget.dart';

const String _nameKey = 'name';
const String _isDirectoryKey = 'is_dir';
const String _mimeKey = 'mime';
const String _contentKey = 'content';
const String _kindKey = 'kind';
const String _bytesKey = 'bytes';
const String _entriesKey = 'entries';
const String _bytesKind = 'bytes';
const String _directoryKind = 'directory';
const String _invalidBase64Message =
    'Canonical file bytes must use padded RFC 4648 standard base64.';
const Set<String> _fileKeys = <String>{
  _nameKey,
  _isDirectoryKey,
  _mimeKey,
  _contentKey,
};
const Set<String> _bytesContentKeys = <String>{_kindKey, _bytesKey};
const Set<String> _directoryContentKeys = <String>{_kindKey, _entriesKey};

/// Decode the JSON shape used by Rust's `InputKind::File` boundary.
///
/// Invalid or internally inconsistent objects return `null`; callers never
/// receive a partially decoded file tree. Root-tree budget violations and
/// invalid canonical byte encodings throw
/// [CanonicalFileValueCodecException].
CanonicalFileValue? canonicalFileValueFromJson(Object? value) {
  if (!_preflightJsonFileTree(value)) return null;
  return _fileValueFromJson(value, currentDepth: 1);
}

CanonicalFileValue? _fileValueFromJson(
  Object? value, {
  required int currentDepth,
}) {
  if (value is! Map<String, Object?> || !_onlyKnownKeys(value, _fileKeys)) {
    return null;
  }
  final name = value[_nameKey];
  final isDirectory = value[_isDirectoryKey];
  final content = value[_contentKey];
  final mime = value[_mimeKey];
  if (name is! String || isDirectory is! bool) return null;
  if (!value.containsKey(_contentKey)) return null;
  if (value.containsKey(_mimeKey) && mime is! String) return null;
  if (content is! Map<String, Object?>) return null;

  final decodedContent = _contentFromJson(
    content,
    isDirectory: isDirectory,
    currentDepth: currentDepth,
  );
  if (decodedContent == null) return null;
  return CanonicalFileValue(
    name: name,
    isDir: isDirectory,
    content: decodedContent,
    mime: mime as String?,
  );
}

CanonicalFileContent? _contentFromJson(
  Map<String, Object?> content, {
  required bool isDirectory,
  required int currentDepth,
}) {
  return switch (content[_kindKey]) {
    _bytesKind when !isDirectory => _bytesContentFromJson(content),
    _directoryKind when isDirectory => _directoryContentFromJson(
      content,
      currentDepth: currentDepth,
    ),
    _ => null,
  };
}

CanonicalFileContent? _bytesContentFromJson(Map<String, Object?> content) {
  if (!_hasExactKeys(content, _bytesContentKeys)) return null;
  final rawBytes = content[_bytesKey];
  if (rawBytes is! String) return null;
  final Uint8List bytes;
  try {
    bytes = base64.decode(rawBytes);
  } on FormatException {
    _throwInvalidBase64();
  }
  return CanonicalFileContent.bytes(bytes: bytes);
}

CanonicalFileContent? _directoryContentFromJson(
  Map<String, Object?> content, {
  required int currentDepth,
}) {
  if (!_hasExactKeys(content, _directoryContentKeys)) return null;
  final rawEntries = content[_entriesKey];
  if (rawEntries is! List<Object?>) return null;
  final entries = <CanonicalFileValue>[];
  for (final rawEntry in rawEntries) {
    final entry = _fileValueFromJson(rawEntry, currentDepth: currentDepth + 1);
    if (entry == null) return null;
    entries.add(entry);
  }
  return CanonicalFileContent.directory(
    entries: List<CanonicalFileValue>.unmodifiable(entries),
  );
}

/// Encode a generated [CanonicalFileValue] to Rust's JSON input shape.
///
/// Root-tree budget violations throw [CanonicalFileValueCodecException].
Map<String, Object?> canonicalFileValueToJson(CanonicalFileValue value) {
  _preflightCanonicalFileTree(value);
  return _fileValueToJson(value, currentDepth: 1);
}

Map<String, Object?> _fileValueToJson(
  CanonicalFileValue value, {
  required int currentDepth,
}) {
  return <String, Object?>{
    _nameKey: value.name,
    _isDirectoryKey: value.isDir,
    if (value.mime != null) _mimeKey: value.mime,
    _contentKey: _contentToJson(value.content, currentDepth: currentDepth),
  };
}

Map<String, Object?> _contentToJson(
  CanonicalFileContent content, {
  required int currentDepth,
}) {
  return switch (content) {
    CanonicalFileContent_Bytes(:final bytes) => _bytesContentToJson(bytes),
    CanonicalFileContent_Directory(:final entries) => <String, Object?>{
      _kindKey: _directoryKind,
      _entriesKey: entries
          .map(
            (entry) => _fileValueToJson(entry, currentDepth: currentDepth + 1),
          )
          .toList(growable: false),
    },
  };
}

Map<String, Object?> _bytesContentToJson(Uint8List bytes) {
  return <String, Object?>{
    _kindKey: _bytesKind,
    _bytesKey: base64.encode(bytes),
  };
}

Never _throwInvalidBase64() {
  throw const CanonicalFileValueCodecException(
    CanonicalFileValueCodecErrorCode.invalidBase64,
    _invalidBase64Message,
  );
}

bool _onlyKnownKeys(Map<String, Object?> value, Set<String> knownKeys) =>
    value.keys.every(knownKeys.contains);

bool _hasExactKeys(Map<String, Object?> value, Set<String> expectedKeys) =>
    value.length == expectedKeys.length &&
    value.keys.every(expectedKeys.contains);
