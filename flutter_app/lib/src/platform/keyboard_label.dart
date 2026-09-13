/// Platform-specific keyboard modifier labels.
///
/// On macOS the convention is the `⌘` glyph; on Linux/Windows users
/// expect to see `Ctrl`. The modifier *check* in the key handler
/// already accepts either `meta` or `ctrl`, but the *label* now
/// follows convention.
///
/// Detection goes through `defaultTargetPlatform` for production,
/// but widget tests should override [`keyboardPlatformProvider`]
/// instead of mutating the foundation debug var (which trips
/// `_verifyInvariants` on every pump).
library;

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

/// Production source of the current platform for keyboard label
/// purposes. Reads [`defaultTargetPlatform`]. Tests override this
/// provider to pin a deterministic platform.
final keyboardPlatformProvider = Provider<TargetPlatform>(
  (ref) => defaultTargetPlatform,
);

/// `⌘` on macOS, `Ctrl` elsewhere — driven by [platform].
String primaryModifierLabel(TargetPlatform platform) {
  return switch (platform) {
    TargetPlatform.macOS => '⌘',
    _ => 'Ctrl',
  };
}

/// `⌘K` (mac) or `Ctrl K` (others). The space on the Ctrl variant
/// is intentional — `CtrlK` reads as a single token; `Ctrl K` keeps
/// modifier and key visually separate.
String shortcutLabel(TargetPlatform platform, String key) {
  return _withModifier(primaryModifierLabel(platform), key);
}

/// The literal Control key: `⌃` on macOS, `Ctrl` elsewhere. Distinct
/// from [primaryModifierLabel] — a `Ctrl+U`-style binding stays Ctrl
/// on macOS too (it is not a Cmd chord).
String controlModifierLabel(TargetPlatform platform) {
  return switch (platform) {
    TargetPlatform.macOS => '⌃',
    _ => 'Ctrl',
  };
}

/// `⌃U` (mac) or `Ctrl U` (others) — the Control-literal counterpart
/// of [shortcutLabel], following the same glyph/word spacing rule.
String controlShortcutLabel(TargetPlatform platform, String key) {
  return _withModifier(controlModifierLabel(platform), key);
}

/// Glyph modifiers (`⌘`, `⌃`) join their key directly; word modifiers
/// (`Ctrl`) keep a separating space.
String _withModifier(String modifier, String key) {
  final isGlyph = modifier.length == 1;
  return isGlyph ? '$modifier$key' : '$modifier $key';
}
