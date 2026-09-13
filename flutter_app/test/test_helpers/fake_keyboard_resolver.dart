import 'package:flutter_riverpod/misc.dart' show Override;

import 'package:upeg/src/keyboard/keyboard_command_resolver.dart';
import 'package:upeg/src/rust/api/keyboard.dart';

final Override fakeKeyboardResolverOverride = keyboardCommandResolverProvider
    .overrideWithValue(fakeKeyboardResolver);

KeyboardCommandDto? fakeKeyboardResolver({
  required String key,
  required bool ctrl,
  required bool meta,
  required bool shift,
  required bool alt,
  required KeyboardScopeDto scope,
  required bool hasToolFocus,
}) {
  if (alt) return null;
  final primary = ctrl || meta;
  if (primary) {
    return _primaryCommand(key, scope);
  }
  return switch (scope) {
    KeyboardScopeDto.board => _boardCommand(
      key,
      shift: shift,
      hasToolFocus: hasToolFocus,
    ),
    KeyboardScopeDto.detail => _detailCommand(key, shift: shift),
    KeyboardScopeDto.form => _formCommand(key, shift: shift),
    KeyboardScopeDto.settings => _settingsCommand(key, shift: shift),
    KeyboardScopeDto.boardEditor => _boardEditorCommand(key),
    KeyboardScopeDto.confirmDelete => _confirmDeleteCommand(key),
    KeyboardScopeDto.toolPicker => _toolPickerCommand(key),
    KeyboardScopeDto.filterBar => _filterBarCommand(key),
    KeyboardScopeDto.rightPane => _rightPaneCommand(key),
    KeyboardScopeDto.moving => _movingCommand(key),
    KeyboardScopeDto.resize => _resizeCommand(key),
  };
}

KeyboardCommandDto? _primaryCommand(String key, KeyboardScopeDto scope) {
  if ((scope == KeyboardScopeDto.board || scope == KeyboardScopeDto.detail) &&
      (key == 'k' || key == 'K')) {
    return const KeyboardCommandDto.search();
  }
  if (scope == KeyboardScopeDto.board) {
    return switch (key) {
      '[' => const KeyboardCommandDto.movePinPrev(),
      ']' => const KeyboardCommandDto.movePinNext(),
      _ => null,
    };
  }
  return null;
}

KeyboardCommandDto? _boardCommand(
  String key, {
  required bool shift,
  required bool hasToolFocus,
}) {
  return switch (key) {
    'ArrowLeft' ||
    'h' ||
    'H' => const KeyboardCommandDto.move(direction: DirectionDto.left),
    'ArrowRight' ||
    'l' ||
    'L' => const KeyboardCommandDto.move(direction: DirectionDto.right),
    'ArrowUp' ||
    'k' ||
    'K' => const KeyboardCommandDto.move(direction: DirectionDto.up),
    'ArrowDown' ||
    'j' ||
    'J' => const KeyboardCommandDto.move(direction: DirectionDto.down),
    'Enter' => const KeyboardCommandDto.open(),
    ' ' || 'F1' => const KeyboardCommandDto.run(),
    'Escape' => const KeyboardCommandDto.close(),
    '/' => const KeyboardCommandDto.search(),
    '?' => const KeyboardCommandDto.showCheatsheet(),
    'q' || 'Q' => const KeyboardCommandDto.quit(),
    's' || 'S' => const KeyboardCommandDto.openSettings(),
    'b' || 'B' => const KeyboardCommandDto.cycleBoardFilter(),
    't' || 'T' => const KeyboardCommandDto.cycleTagFilter(),
    '0' => const KeyboardCommandDto.clearBoardFilter(),
    '1' => const KeyboardCommandDto.switchBoard(slot: 1),
    '2' => const KeyboardCommandDto.switchBoard(slot: 2),
    '3' => const KeyboardCommandDto.switchBoard(slot: 3),
    '4' => const KeyboardCommandDto.switchBoard(slot: 4),
    '5' => const KeyboardCommandDto.switchBoard(slot: 5),
    '6' => const KeyboardCommandDto.switchBoard(slot: 6),
    '7' => const KeyboardCommandDto.switchBoard(slot: 7),
    '8' => const KeyboardCommandDto.switchBoard(slot: 8),
    '9' => const KeyboardCommandDto.switchBoard(slot: 9),
    'n' => const KeyboardCommandDto.newBoard(),
    'R' => const KeyboardCommandDto.renameBoard(),
    'D' => const KeyboardCommandDto.deleteBoard(),
    'a' || 'A' => const KeyboardCommandDto.openToolPicker(),
    'p' || 'P' when hasToolFocus => const KeyboardCommandDto.togglePin(),
    'm' || 'M' when hasToolFocus => const KeyboardCommandDto.startMove(),
    'e' || 'E' when hasToolFocus => const KeyboardCommandDto.startResize(),
    '[' when hasToolFocus => const KeyboardCommandDto.reorder(
      direction: OrderDirectionDto.previous,
    ),
    ']' when hasToolFocus => const KeyboardCommandDto.reorder(
      direction: OrderDirectionDto.next,
    ),
    'Tab' when shift => const KeyboardCommandDto.focusPrevious(),
    'Tab' => const KeyboardCommandDto.focusNext(),
    _ => null,
  };
}

KeyboardCommandDto? _detailCommand(String key, {required bool shift}) {
  return switch (key) {
    'F1' || 'Enter' || 'r' || 'R' => const KeyboardCommandDto.run(),
    'Escape' || 'q' || 'Q' => const KeyboardCommandDto.close(),
    's' || 'S' => const KeyboardCommandDto.openSettings(),
    'm' || 'M' => const KeyboardCommandDto.startMove(),
    '/' => const KeyboardCommandDto.search(),
    'b' || 'B' => const KeyboardCommandDto.cycleBoardFilter(),
    't' || 'T' => const KeyboardCommandDto.cycleTagFilter(),
    '0' => const KeyboardCommandDto.clearBoardFilter(),
    'Tab' when shift => const KeyboardCommandDto.focusPrevious(),
    'Tab' => const KeyboardCommandDto.focusNext(),
    _ => null,
  };
}

KeyboardCommandDto? _formCommand(String key, {required bool shift}) {
  return switch (key) {
    'ArrowDown' => const KeyboardCommandDto.focusNext(),
    'Tab' when !shift => const KeyboardCommandDto.focusNext(),
    'ArrowUp' => const KeyboardCommandDto.focusPrevious(),
    'Tab' when shift => const KeyboardCommandDto.focusPrevious(),
    'ArrowLeft' => const KeyboardCommandDto.move(direction: DirectionDto.left),
    'ArrowRight' => const KeyboardCommandDto.move(
      direction: DirectionDto.right,
    ),
    'Backspace' => const KeyboardCommandDto.backspace(),
    'Enter' || 'F1' => const KeyboardCommandDto.run(),
    'Escape' => const KeyboardCommandDto.close(),
    ' ' => const KeyboardCommandDto.text(ch: ' '),
    _ when key.length == 1 => KeyboardCommandDto.text(ch: key),
    _ => null,
  };
}

KeyboardCommandDto? _settingsCommand(String key, {required bool shift}) {
  return switch (key) {
    'q' || 'Q' || 'Escape' => const KeyboardCommandDto.close(),
    'j' || 'J' || 'ArrowDown' => const KeyboardCommandDto.focusNext(),
    'Tab' when !shift => const KeyboardCommandDto.focusNext(),
    'k' || 'K' || 'ArrowUp' => const KeyboardCommandDto.focusPrevious(),
    'Tab' when shift => const KeyboardCommandDto.focusPrevious(),
    'l' || 'L' || 'ArrowRight' => const KeyboardCommandDto.move(
      direction: DirectionDto.right,
    ),
    'h' ||
    'H' ||
    'ArrowLeft' => const KeyboardCommandDto.move(direction: DirectionDto.left),
    _ => null,
  };
}

KeyboardCommandDto? _boardEditorCommand(String key) {
  return switch (key) {
    'Enter' => const KeyboardCommandDto.commit(),
    'Escape' => const KeyboardCommandDto.cancel(),
    'Backspace' => const KeyboardCommandDto.backspace(),
    ' ' => const KeyboardCommandDto.text(ch: ' '),
    _ when key.length == 1 => KeyboardCommandDto.text(ch: key),
    _ => null,
  };
}

KeyboardCommandDto? _confirmDeleteCommand(String key) {
  return switch (key) {
    'Enter' || 'y' || 'Y' => const KeyboardCommandDto.confirm(),
    'Escape' || 'n' || 'N' || 'q' || 'Q' => const KeyboardCommandDto.cancel(),
    _ => null,
  };
}

KeyboardCommandDto? _toolPickerCommand(String key) {
  return switch (key) {
    'ArrowDown' => const KeyboardCommandDto.move(direction: DirectionDto.down),
    'ArrowUp' => const KeyboardCommandDto.move(direction: DirectionDto.up),
    'Enter' => const KeyboardCommandDto.commit(),
    'Escape' => const KeyboardCommandDto.cancel(),
    'Backspace' => const KeyboardCommandDto.backspace(),
    ' ' => const KeyboardCommandDto.text(ch: ' '),
    _ when key.length == 1 => KeyboardCommandDto.text(ch: key),
    _ => null,
  };
}

KeyboardCommandDto? _filterBarCommand(String key) {
  return switch (key) {
    'ArrowLeft' ||
    'h' ||
    'H' => const KeyboardCommandDto.move(direction: DirectionDto.left),
    'ArrowRight' ||
    'l' ||
    'L' => const KeyboardCommandDto.move(direction: DirectionDto.right),
    'Home' => const KeyboardCommandDto.home(),
    'End' => const KeyboardCommandDto.end(),
    'Enter' || ' ' => const KeyboardCommandDto.open(),
    _ => null,
  };
}

KeyboardCommandDto? _rightPaneCommand(String key) {
  return switch (key) {
    'ArrowDown' ||
    'j' ||
    'J' => const KeyboardCommandDto.move(direction: DirectionDto.down),
    'ArrowUp' ||
    'k' ||
    'K' => const KeyboardCommandDto.move(direction: DirectionDto.up),
    'PageDown' => const KeyboardCommandDto.page(
      direction: PageDirectionDto.down,
    ),
    'PageUp' => const KeyboardCommandDto.page(direction: PageDirectionDto.up),
    'Home' => const KeyboardCommandDto.home(),
    'End' => const KeyboardCommandDto.end(),
    _ => null,
  };
}

/// Mirrors the shared resolver's Resize-scope policy: arrows/hjkl emit
/// span DELTAS (direction→delta mapping lives in the resolver), Enter/F1
/// commit, Esc/q cancel, `0` resets to the manifest footprint.
KeyboardCommandDto? _resizeCommand(String key) {
  return switch (key) {
    'ArrowLeft' ||
    'h' ||
    'H' => const KeyboardCommandDto.resizeBy(cols: -1, rows: 0),
    'ArrowRight' ||
    'l' ||
    'L' => const KeyboardCommandDto.resizeBy(cols: 1, rows: 0),
    'ArrowUp' ||
    'k' ||
    'K' => const KeyboardCommandDto.resizeBy(cols: 0, rows: -1),
    'ArrowDown' ||
    'j' ||
    'J' => const KeyboardCommandDto.resizeBy(cols: 0, rows: 1),
    'Enter' || 'F1' => const KeyboardCommandDto.commit(),
    'Escape' || 'q' || 'Q' => const KeyboardCommandDto.cancel(),
    '0' => const KeyboardCommandDto.resetSpan(),
    _ => null,
  };
}

KeyboardCommandDto? _movingCommand(String key) {
  return switch (key) {
    'ArrowLeft' ||
    'h' ||
    'H' => const KeyboardCommandDto.move(direction: DirectionDto.left),
    'ArrowRight' ||
    'l' ||
    'L' => const KeyboardCommandDto.move(direction: DirectionDto.right),
    'ArrowUp' ||
    'k' ||
    'K' => const KeyboardCommandDto.move(direction: DirectionDto.up),
    'ArrowDown' ||
    'j' ||
    'J' => const KeyboardCommandDto.move(direction: DirectionDto.down),
    'Enter' => const KeyboardCommandDto.commit(),
    'Escape' || 'q' || 'Q' => const KeyboardCommandDto.cancel(),
    _ => null,
  };
}
