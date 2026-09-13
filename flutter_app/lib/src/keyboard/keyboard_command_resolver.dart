/// Flutter key-event bridge for the Rust keyboard policy.
///
/// Widgets decide only their current [KeyboardScopeDto] and contextual
/// gates. Key-label parsing and command resolution stay behind this
/// small bridge so individual surfaces do not grow their own shortcut maps.
library;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/rust/api/keyboard.dart';

/// Dart-side resolver for the keyboard FRB. Swappable so widget tests
/// can stub the keyboard policy without touching the Rust dylib.
typedef KeyboardCommandResolver =
    KeyboardCommandDto? Function({
      required String key,
      required bool ctrl,
      required bool meta,
      required bool shift,
      required bool alt,
      required KeyboardScopeDto scope,
      required bool hasToolFocus,
    });

KeyboardCommandDto? _defaultResolveCommand({
  required String key,
  required bool ctrl,
  required bool meta,
  required bool shift,
  required bool alt,
  required KeyboardScopeDto scope,
  required bool hasToolFocus,
}) {
  return keyboardCommandFor(
    key: key,
    ctrl: ctrl,
    meta: meta,
    shift: shift,
    alt: alt,
    scope: scope,
    hasToolFocus: hasToolFocus,
  );
}

final keyboardCommandResolverProvider = Provider<KeyboardCommandResolver>(
  (ref) => _defaultResolveCommand,
);

final Map<LogicalKeyboardKey, String> _functionKeyLabels = {
  LogicalKeyboardKey.f1: 'F1',
  LogicalKeyboardKey.f2: 'F2',
  LogicalKeyboardKey.f3: 'F3',
  LogicalKeyboardKey.f4: 'F4',
  LogicalKeyboardKey.f5: 'F5',
  LogicalKeyboardKey.f6: 'F6',
  LogicalKeyboardKey.f7: 'F7',
  LogicalKeyboardKey.f8: 'F8',
  LogicalKeyboardKey.f9: 'F9',
  LogicalKeyboardKey.f10: 'F10',
  LogicalKeyboardKey.f11: 'F11',
  LogicalKeyboardKey.f12: 'F12',
  LogicalKeyboardKey.f13: 'F13',
  LogicalKeyboardKey.f14: 'F14',
  LogicalKeyboardKey.f15: 'F15',
  LogicalKeyboardKey.f16: 'F16',
  LogicalKeyboardKey.f17: 'F17',
  LogicalKeyboardKey.f18: 'F18',
  LogicalKeyboardKey.f19: 'F19',
  LogicalKeyboardKey.f20: 'F20',
  LogicalKeyboardKey.f21: 'F21',
  LogicalKeyboardKey.f22: 'F22',
  LogicalKeyboardKey.f23: 'F23',
  LogicalKeyboardKey.f24: 'F24',
};

final Map<LogicalKeyboardKey, String> _logicalLetterLabels = {
  LogicalKeyboardKey.keyA: 'a',
  LogicalKeyboardKey.keyB: 'b',
  LogicalKeyboardKey.keyC: 'c',
  LogicalKeyboardKey.keyD: 'd',
  LogicalKeyboardKey.keyE: 'e',
  LogicalKeyboardKey.keyF: 'f',
  LogicalKeyboardKey.keyG: 'g',
  LogicalKeyboardKey.keyH: 'h',
  LogicalKeyboardKey.keyI: 'i',
  LogicalKeyboardKey.keyJ: 'j',
  LogicalKeyboardKey.keyK: 'k',
  LogicalKeyboardKey.keyL: 'l',
  LogicalKeyboardKey.keyM: 'm',
  LogicalKeyboardKey.keyN: 'n',
  LogicalKeyboardKey.keyO: 'o',
  LogicalKeyboardKey.keyP: 'p',
  LogicalKeyboardKey.keyQ: 'q',
  LogicalKeyboardKey.keyR: 'r',
  LogicalKeyboardKey.keyS: 's',
  LogicalKeyboardKey.keyT: 't',
  LogicalKeyboardKey.keyU: 'u',
  LogicalKeyboardKey.keyV: 'v',
  LogicalKeyboardKey.keyW: 'w',
  LogicalKeyboardKey.keyX: 'x',
  LogicalKeyboardKey.keyY: 'y',
  LogicalKeyboardKey.keyZ: 'z',
};

bool isKeyboardPress(KeyEvent event) =>
    event is KeyDownEvent || event is KeyRepeatEvent;

bool primaryFocusIsEditableText() {
  final focusedContext = FocusManager.instance.primaryFocus?.context;
  if (focusedContext == null) return false;
  if (focusedContext.widget is EditableText) return true;
  return focusedContext.findAncestorWidgetOfExactType<EditableText>() != null;
}

/// Marker wrapper around an inline embed pin body (the `bodyOverride`
/// mounted in `Pin`). The board keyboard handler walks the currently
/// focused node's widget ancestors for this marker to decide whether a
/// plain-letter key should stay text — typed into the embed's form field
/// or its live webview — instead of resolving into a board command.
///
/// Unlike [primaryFocusIsEditableText], this also covers the case where
/// the focused node is the embed's webview platform view (not an
/// `EditableText`): any focus inside the embed body subtree counts.
///
/// The wrapper is a non-focusable [Focus] node (so it never steals focus
/// or a Tab stop) that simply establishes a detectable subtree boundary.
/// See [primaryFocusIsInsideEmbedBody].
class EmbedBodyFocusScope extends StatelessWidget {
  const EmbedBodyFocusScope({required this.child, super.key});

  /// Stable debug label for the underlying [Focus] node. Detection uses
  /// the widget type (see [primaryFocusIsInsideEmbedBody]); this label is
  /// only for devtools/inspector legibility.
  static const String debugLabel = 'EmbedBodyFocusScope';

  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Focus(
      debugLabel: debugLabel,
      canRequestFocus: false,
      skipTraversal: true,
      child: child,
    );
  }
}

/// Whether the primary focus lives inside an [EmbedBodyFocusScope] — i.e.
/// the user is interacting with an inline embed pin body (form field or
/// live webview). Walks the focused node's widget ancestors for the
/// marker so the board can yield plain-letter keys to the embed while
/// keeping Cmd/Ctrl+K and F-keys global.
bool primaryFocusIsInsideEmbedBody() {
  final focusedContext = FocusManager.instance.primaryFocus?.context;
  if (focusedContext == null) return false;
  if (focusedContext.widget is EmbedBodyFocusScope) return true;
  return focusedContext.findAncestorWidgetOfExactType<EmbedBodyFocusScope>() !=
      null;
}

bool isPlainTextEntryKeyEvent(KeyEvent event) {
  if (!isKeyboardPress(event)) return false;
  final keyboard = HardwareKeyboard.instance;
  if (keyboard.isControlPressed || keyboard.isMetaPressed) return false;
  final character = event.character;
  return (character != null && character.isNotEmpty) ||
      _logicalLetterLabels.containsKey(event.logicalKey);
}

/// Whether [event] is a *global* board shortcut that must stay reserved
/// for the board even while the user is typing into an inline embed body
/// or another editable-text field. The complement of this set is yielded
/// to the focused widget (issue 6 gap 1): so arrows, Home/End,
/// PageUp/PageDown, Backspace, Delete, and plain letters/digits all
/// become text/caret motion inside the field, while these stay global:
///
///   * any Cmd/Ctrl-modified chord (e.g. Cmd/Ctrl+K opens the palette),
///   * the function keys F1..F24,
///   * Escape — its two-stage focus-return contract is handled ahead of
///     the yield in the board key handler, so it is never swallowed as
///     field text here.
///
/// Kept in the resolver (not inlined in `board_page`) so the board never
/// grows its own literal key list.
bool isGlobalShortcutKeyEvent(KeyEvent event) {
  if (!isKeyboardPress(event)) return false;
  final keyboard = HardwareKeyboard.instance;
  if (keyboard.isControlPressed || keyboard.isMetaPressed) return true;
  if (event.logicalKey == LogicalKeyboardKey.escape) return true;
  return _functionKeyLabels.containsKey(event.logicalKey);
}

/// Whether [event] matches the shortcut chord described by [keys] (the
/// `keys` string of a `SourceDto.shortcut`, e.g. `"Cmd+Shift+N"`).
///
/// Cmd, Command, Ctrl, Control, Meta, and Super all count as the single
/// "primary" modifier (Cmd on macOS / Ctrl elsewhere); Shift and
/// Alt/Option are matched exactly. The final non-modifier token is the
/// key label, compared case-insensitively against [keyboardLabelForEvent].
/// Lets the board activate any Shortcut-source tool from its declared
/// metadata without hard-coding a chord.
bool shortcutKeysMatchEvent(String keys, KeyEvent event) {
  if (!isKeyboardPress(event)) return false;
  final parts = keys
      .split('+')
      .map((part) => part.trim())
      .where((part) => part.isNotEmpty)
      .toList();
  if (parts.isEmpty) return false;

  var wantPrimary = false;
  var wantShift = false;
  var wantAlt = false;
  String? wantKey;
  for (final part in parts) {
    switch (part.toLowerCase()) {
      case 'cmd' || 'command' || 'ctrl' || 'control' || 'meta' || 'super':
        wantPrimary = true;
      case 'shift':
        wantShift = true;
      case 'alt' || 'option' || 'opt':
        wantAlt = true;
      default:
        wantKey = part;
    }
  }
  if (wantKey == null) return false;

  final keyboard = HardwareKeyboard.instance;
  final hasPrimary = keyboard.isControlPressed || keyboard.isMetaPressed;
  if (wantPrimary != hasPrimary) return false;
  if (wantShift != keyboard.isShiftPressed) return false;
  if (wantAlt != keyboard.isAltPressed) return false;

  final label = keyboardLabelForEvent(event);
  if (label == null) return false;
  return label.toLowerCase() == wantKey.toLowerCase();
}

bool isKeyboardActivationEvent(KeyEvent event) {
  if (!isKeyboardPress(event)) return false;
  final label = keyboardLabelForEvent(event);
  return label == 'Enter' || label == ' ';
}

bool isPrimaryKeyboardShortcut(KeyEvent event, String keyLabel) {
  if (!isKeyboardPress(event)) return false;
  final keyboard = HardwareKeyboard.instance;
  return keyboardLabelForEvent(event) == keyLabel &&
      (keyboard.isControlPressed || keyboard.isMetaPressed) &&
      !keyboard.isAltPressed;
}

bool isKeyboardContextMenuEvent(KeyEvent event) {
  if (!isKeyboardPress(event)) return false;
  if (event.logicalKey == LogicalKeyboardKey.contextMenu) return true;
  return keyboardLabelForEvent(event) == 'F10' &&
      HardwareKeyboard.instance.isShiftPressed;
}

/// Best-effort label translation for the keyboard FRB. Mirrors the labels
/// accepted by `upeg_core::key_from_label`.
String? keyboardLabelForEvent(KeyEvent event) {
  final logical = event.logicalKey;
  final character = event.character ?? '';
  if (logical == LogicalKeyboardKey.arrowUp) return 'ArrowUp';
  if (logical == LogicalKeyboardKey.arrowDown) return 'ArrowDown';
  if (logical == LogicalKeyboardKey.arrowLeft) return 'ArrowLeft';
  if (logical == LogicalKeyboardKey.arrowRight) return 'ArrowRight';
  if (logical == LogicalKeyboardKey.enter ||
      logical == LogicalKeyboardKey.numpadEnter) {
    return 'Enter';
  }
  if (logical == LogicalKeyboardKey.escape) return 'Escape';
  if (logical == LogicalKeyboardKey.tab) return 'Tab';
  if (logical == LogicalKeyboardKey.backspace) return 'Backspace';
  if (logical == LogicalKeyboardKey.home) return 'Home';
  if (logical == LogicalKeyboardKey.end) return 'End';
  if (logical == LogicalKeyboardKey.pageUp) return 'PageUp';
  if (logical == LogicalKeyboardKey.pageDown) return 'PageDown';
  if (logical == LogicalKeyboardKey.space) return ' ';
  final functionLabel = _functionKeyLabels[logical];
  if (functionLabel != null) return functionLabel;
  if (character.isNotEmpty) return character;
  final letterLabel = _logicalLetterLabels[logical];
  if (letterLabel != null) return letterLabel;
  return null;
}

KeyboardCommandDto? resolveKeyboardCommand(
  WidgetRef ref,
  KeyEvent event, {
  required KeyboardScopeDto scope,
  bool hasToolFocus = false,
}) {
  if (!isKeyboardPress(event)) return null;
  final label = keyboardLabelForEvent(event);
  if (label == null) return null;

  final keyboard = HardwareKeyboard.instance;
  final resolver = ref.read(keyboardCommandResolverProvider);
  return resolver(
    key: label,
    ctrl: keyboard.isControlPressed,
    meta: keyboard.isMetaPressed,
    shift: keyboard.isShiftPressed,
    alt: keyboard.isAltPressed,
    scope: scope,
    hasToolFocus: hasToolFocus,
  );
}
