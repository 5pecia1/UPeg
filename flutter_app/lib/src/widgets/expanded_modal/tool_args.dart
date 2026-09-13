/// Typed argument envelope for tool dispatch.
///
/// Flutter forms keep values as a JSON-compatible object until the last
/// boundary before FRB. That keeps `Map<String, dynamic>` from becoming the
/// modal's public API while still matching the Rust dispatcher contract.
library;

import 'dart:convert';

import 'package:flutter/foundation.dart';

@immutable
final class ToolArgs {
  ToolArgs.fromJsonObject(Map<String, Object?> values)
    : _values = Map<String, Object?>.unmodifiable(values);

  static const String emptyJson = '{}';

  static final ToolArgs empty = ToolArgs.fromJsonObject(
    const <String, Object?>{},
  );

  final Map<String, Object?> _values;

  bool get isEmpty => _values.isEmpty;

  bool get isNotEmpty => _values.isNotEmpty;

  Object? operator [](String key) => _values[key];

  Map<String, Object?> toJsonObject() => _values;

  String encodeJson() => jsonEncode(_values);

  static ToolArgs? tryDecodeObject(String json, {bool nullIfEmpty = false}) {
    try {
      final decoded = jsonDecode(json);
      if (decoded is! Map) return null;
      final values = <String, Object?>{};
      for (final entry in decoded.entries) {
        final key = entry.key;
        if (key is! String) return null;
        values[key] = entry.value as Object?;
      }
      if (values.isEmpty && nullIfEmpty) return null;
      return ToolArgs.fromJsonObject(values);
    } on FormatException {
      return null;
    }
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is ToolArgs && _jsonValueEquals(other._values, _values);

  @override
  int get hashCode => _jsonValueHash(_values);
}

bool _jsonValueEquals(Object? a, Object? b) {
  if (identical(a, b)) return true;
  return switch ((a, b)) {
    (final Map<dynamic, dynamic> left, final Map<dynamic, dynamic> right) =>
      _jsonMapEquals(left, right),
    (final List<dynamic> left, final List<dynamic> right) => _jsonListEquals(
      left,
      right,
    ),
    _ => a == b,
  };
}

bool _jsonMapEquals(Map<dynamic, dynamic> left, Map<dynamic, dynamic> right) {
  if (left.length != right.length) return false;
  for (final key in left.keys) {
    if (!right.containsKey(key)) return false;
    if (!_jsonValueEquals(left[key], right[key])) return false;
  }
  return true;
}

bool _jsonListEquals(List<dynamic> left, List<dynamic> right) {
  if (left.length != right.length) return false;
  for (var i = 0; i < left.length; i += 1) {
    if (!_jsonValueEquals(left[i], right[i])) return false;
  }
  return true;
}

int _jsonValueHash(Object? value) {
  return switch (value) {
    final Map<dynamic, dynamic> map => _jsonMapHash(map),
    final List<dynamic> list => Object.hashAll(list.map(_jsonValueHash)),
    _ => value.hashCode,
  };
}

int _jsonMapHash(Map<dynamic, dynamic> map) {
  final entries = map.entries.toList()
    ..sort((a, b) => a.key.toString().compareTo(b.key.toString()));
  return Object.hashAll(
    entries.map((entry) => Object.hash(entry.key, _jsonValueHash(entry.value))),
  );
}
