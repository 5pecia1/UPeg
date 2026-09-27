/// App-level identity value objects.
///
/// Generated FRB DTOs still expose raw strings, because that is the wire
/// format. Flutter state and widget seams should promote those strings into
/// these types as early as practical, then unwrap only at the FRB boundary.
library;

import 'package:flutter/foundation.dart';

@immutable
final class ToolId {
  const ToolId._(this.value);

  factory ToolId.parse(String raw) {
    if (raw.isEmpty) {
      throw FormatException('tool id must not be empty', raw);
    }
    if (raw.trim() != raw) {
      throw FormatException('tool id must be canonical and unpadded', raw);
    }
    final separator = raw.indexOf('.');
    if (separator <= 0 || separator == raw.length - 1) {
      throw FormatException('tool id must be `{toolkit}.{local}`', raw);
    }
    return ToolId._(raw);
  }

  static ToolId? tryParse(String? raw) {
    if (raw == null) return null;
    try {
      return ToolId.parse(raw);
    } on FormatException {
      return null;
    }
  }

  final String value;

  @override
  String toString() => value;

  @override
  bool operator ==(Object other) =>
      identical(this, other) || other is ToolId && other.value == value;

  @override
  int get hashCode => value.hashCode;
}

/// Stable identity of one placement instance.
///
/// Unlike [ToolId], a pin id has no catalogue-shaped syntax: Rust owns
/// minting it and old persisted pins deliberately use their tool id during
/// migration. Flutter therefore accepts any non-empty canonical string.
@immutable
final class PinId {
  const PinId._(this.value);

  factory PinId.parse(String raw) {
    if (raw.isEmpty) {
      throw FormatException('pin id must not be empty', raw);
    }
    if (raw.trim() != raw) {
      throw FormatException('pin id must be canonical and unpadded', raw);
    }
    return PinId._(raw);
  }

  static PinId? tryParse(String? raw) {
    if (raw == null) return null;
    try {
      return PinId.parse(raw);
    } on FormatException {
      return null;
    }
  }

  final String value;

  @override
  String toString() => value;

  @override
  bool operator ==(Object other) =>
      identical(this, other) || other is PinId && other.value == value;

  @override
  int get hashCode => value.hashCode;
}

@immutable
final class BoardKey {
  const BoardKey._(this.value);

  factory BoardKey.parse(String raw) {
    if (raw.isEmpty) {
      throw FormatException('board key must not be empty', raw);
    }
    if (raw.trim() != raw) {
      throw FormatException('board key must be canonical and unpadded', raw);
    }
    return BoardKey._(raw);
  }

  static BoardKey? tryParse(String? raw) {
    if (raw == null) return null;
    try {
      return BoardKey.parse(raw);
    } on FormatException {
      return null;
    }
  }

  final String value;

  @override
  String toString() => value;

  @override
  bool operator ==(Object other) =>
      identical(this, other) || other is BoardKey && other.value == value;

  @override
  int get hashCode => value.hashCode;
}
