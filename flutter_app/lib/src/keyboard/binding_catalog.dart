/// Dart access to the shared keyboard binding catalog.
///
/// The catalog (`upeg_core::binding_catalog` via the
/// `keyboardBindingCatalog` FRB) is the single source for "which keys
/// do what" display surfaces: the `?` cheatsheet overlay and the pin
/// context-menu key hints. Widgets read it through
/// [keyboardBindingCatalogProvider] so widget tests can override it
/// with a fixture instead of linking the native dylib.
///
/// Rendering stays typed end-to-end: [CatalogKeyDto] maps to key-cap
/// glyphs here, and the OS-conventional modifier labels come from
/// `platform/keyboard_label.dart`.
library;

import 'package:flutter/foundation.dart' show TargetPlatform;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/platform/keyboard_label.dart';
import 'package:upeg/src/rust/api/keyboard.dart';

/// The shared binding catalog. Overridden in widget tests with a
/// fixture catalog (`test_helpers/fake_binding_catalog.dart`).
final keyboardBindingCatalogProvider = Provider<List<ScopeBindingsDto>>(
  (ref) => keyboardBindingCatalog(),
);

/// Key-cap glyph for one catalog key. Arrows/backspace use their
/// conventional glyphs; word keys stay lowercase to match the `esc`
/// kbd-chip idiom the palette established.
String catalogKeyCapLabel(CatalogKeyDto key) {
  return switch (key) {
    CatalogKeyDto_Up() => '↑',
    CatalogKeyDto_Down() => '↓',
    CatalogKeyDto_Left() => '←',
    CatalogKeyDto_Right() => '→',
    CatalogKeyDto_Enter() => '↵',
    CatalogKeyDto_Esc() => 'esc',
    CatalogKeyDto_Tab() => 'tab',
    CatalogKeyDto_BackTab() => '⇧tab',
    CatalogKeyDto_PageUp() => 'pgup',
    CatalogKeyDto_PageDown() => 'pgdn',
    CatalogKeyDto_Home() => 'home',
    CatalogKeyDto_End() => 'end',
    CatalogKeyDto_Backspace() => '⌫',
    CatalogKeyDto_Space() => 'space',
    CatalogKeyDto_Char(:final ch) => ch,
    CatalogKeyDto_F(:final number) => 'F$number',
  };
}

/// Display label for one chord, applying the OS-conventional modifier
/// labels (`⌘K` / `Ctrl K`, `⌃U` / `Ctrl U`).
String catalogChordLabel(TargetPlatform platform, CatalogChordDto chord) {
  final cap = catalogKeyCapLabel(chord.key);
  return switch (chord.modifier) {
    ChordModifierDto.none => cap,
    ChordModifierDto.primary => shortcutLabel(platform, cap),
    ChordModifierDto.control => controlShortcutLabel(platform, cap),
  };
}

/// Minimum run length worth compressing into a `first–last` range cap.
const int _keyCapRangeMinRun = 3;

/// Key-cap labels for an entry's chords, compressing consecutive
/// plain single-character runs (the `1`…`9` board slots) into a single
/// `1–9` cap. Generic: any run of [_keyCapRangeMinRun]+ consecutive
/// unmodified single characters compresses, nothing is hard-coded to a
/// specific binding.
List<String> catalogEntryKeyCaps(
  TargetPlatform platform,
  List<CatalogBindingDto> bindings,
) {
  final labels = <String>[];
  var i = 0;
  while (i < bindings.length) {
    final run = _plainCharRunLength(bindings, i);
    if (run >= _keyCapRangeMinRun) {
      final first = _plainChar(bindings[i].chord);
      final last = _plainChar(bindings[i + run - 1].chord);
      labels.add('$first–$last');
      i += run;
      continue;
    }
    labels.add(catalogChordLabel(platform, bindings[i].chord));
    i += 1;
  }
  return labels;
}

/// Length of the consecutive-plain-char run starting at [start]
/// (single characters, no modifier, each one code unit above the
/// previous).
int _plainCharRunLength(List<CatalogBindingDto> bindings, int start) {
  final first = _plainChar(bindings[start].chord);
  if (first == null) return 1;
  var length = 1;
  var previous = first.codeUnitAt(0);
  for (var i = start + 1; i < bindings.length; i++) {
    final next = _plainChar(bindings[i].chord);
    if (next == null || next.codeUnitAt(0) != previous + 1) break;
    previous = next.codeUnitAt(0);
    length += 1;
  }
  return length;
}

String? _plainChar(CatalogChordDto chord) {
  if (chord.modifier != ChordModifierDto.none) return null;
  final key = chord.key;
  if (key is! CatalogKeyDto_Char || key.ch.length != 1) return null;
  return key.ch;
}

/// First display chord bound to a command matching [matches] in
/// [scope], or null when the catalog has no such binding. Used by the
/// pin context menu to annotate entries with their keyboard mirror
/// (`o` / `c` / `p` / `e`) without hard-coding any key.
String? chordCapForCommand(
  List<ScopeBindingsDto> catalog,
  TargetPlatform platform, {
  required KeyboardScopeDto scope,
  required bool Function(KeyboardCommandDto command) matches,
}) {
  for (final scopeBindings in catalog) {
    if (scopeBindings.scope != scope) continue;
    for (final entry in scopeBindings.entries) {
      for (final binding in entry.bindings) {
        if (matches(binding.command)) {
          return catalogChordLabel(platform, binding.chord);
        }
      }
    }
  }
  return null;
}
