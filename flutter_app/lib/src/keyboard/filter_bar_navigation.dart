/// Shared keyboard navigation for compact filter strips.
///
/// Board tabs, popup tabs, and tag chips all expose the same focused-strip
/// contract: left/right move the selection, Home/End jump to edges, and
/// Enter/Space activate the current item. The Rust `FilterBar` scope owns the
/// key policy; this helper owns only the index math and selection callback.
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/keyboard/keyboard_command_resolver.dart';
import 'package:upeg/src/rust/api/keyboard.dart';

const int _previousItemOffset = -1;
const int _nextItemOffset = 1;

KeyEventResult handleFilterBarKey<T>(
  WidgetRef ref,
  KeyEvent event, {
  required List<T> items,
  required int currentIndex,
  required void Function(T item) onSelect,
}) {
  final cmd = resolveKeyboardCommand(
    ref,
    event,
    scope: KeyboardScopeDto.filterBar,
  );
  return switch (cmd) {
    KeyboardCommandDto_Move(direction: DirectionDto.left) => _selectByOffset(
      items,
      currentIndex,
      _previousItemOffset,
      onSelect,
    ),
    KeyboardCommandDto_Move(direction: DirectionDto.right) => _selectByOffset(
      items,
      currentIndex,
      _nextItemOffset,
      onSelect,
    ),
    KeyboardCommandDto_Home() => _selectEndpoint(
      items,
      last: false,
      onSelect: onSelect,
    ),
    KeyboardCommandDto_End() => _selectEndpoint(
      items,
      last: true,
      onSelect: onSelect,
    ),
    KeyboardCommandDto_Open() =>
      items.isEmpty ? KeyEventResult.ignored : KeyEventResult.handled,
    _ => KeyEventResult.ignored,
  };
}

KeyEventResult _selectByOffset<T>(
  List<T> items,
  int currentIndex,
  int offset,
  void Function(T item) onSelect,
) {
  if (items.isEmpty) return KeyEventResult.ignored;
  final fallbackIndex = offset.isNegative ? 0 : -1;
  final baseIndex = currentIndex < 0 ? fallbackIndex : currentIndex;
  final nextIndex = (baseIndex + offset + items.length) % items.length;
  onSelect(items[nextIndex]);
  return KeyEventResult.handled;
}

KeyEventResult _selectEndpoint<T>(
  List<T> items, {
  required bool last,
  required void Function(T item) onSelect,
}) {
  if (items.isEmpty) return KeyEventResult.ignored;
  onSelect(last ? items.last : items.first);
  return KeyEventResult.handled;
}
