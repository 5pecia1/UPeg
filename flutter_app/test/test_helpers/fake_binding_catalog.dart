/// Shared fixture for the keyboard binding catalog.
///
/// Widget tests never link the native dylib, so widgets that read
/// `keyboardBindingCatalogProvider` (cheatsheet overlay, pin context
/// menu key hints) need the provider overridden. The fixture mirrors a
/// representative slice of `upeg_core::binding_catalog()` — enough to
/// assert section rendering, key-cap compression, and the context-menu
/// key lookups — while the full-catalog/resolver parity stays pinned by
/// the Rust-side tests in `upeg-core/src/keyboard_catalog.rs`.
library;

import 'package:flutter_riverpod/misc.dart' show Override;

import 'package:upeg/src/keyboard/binding_catalog.dart';
import 'package:upeg/src/rust/api/keyboard.dart';

final Override fakeBindingCatalogOverride = keyboardBindingCatalogProvider
    .overrideWithValue(fakeBindingCatalog);

CatalogBindingDto _plainChar(String ch, KeyboardCommandDto command) =>
    CatalogBindingDto(
      chord: CatalogChordDto(
        modifier: ChordModifierDto.none,
        key: CatalogKeyDto.char(ch: ch),
      ),
      command: command,
    );

CatalogBindingDto _plain(CatalogKeyDto key, KeyboardCommandDto command) =>
    CatalogBindingDto(
      chord: CatalogChordDto(modifier: ChordModifierDto.none, key: key),
      command: command,
    );

final List<ScopeBindingsDto> fakeBindingCatalog = [
  ScopeBindingsDto(
    scope: KeyboardScopeDto.board,
    labelKey: 'keys.scope.board',
    entries: [
      BindingEntryDto(
        labelKey: 'keys.cmd.run',
        requiresToolFocus: false,
        bindings: [
          _plain(const CatalogKeyDto.enter(), const KeyboardCommandDto.run()),
          _plain(const CatalogKeyDto.space(), const KeyboardCommandDto.run()),
        ],
      ),
      BindingEntryDto(
        labelKey: 'keys.cmd.open',
        requiresToolFocus: false,
        bindings: [_plainChar('o', const KeyboardCommandDto.open())],
      ),
      BindingEntryDto(
        labelKey: 'keys.cmd.search',
        requiresToolFocus: false,
        bindings: [
          _plainChar('/', const KeyboardCommandDto.search()),
          const CatalogBindingDto(
            chord: CatalogChordDto(
              modifier: ChordModifierDto.primary,
              key: CatalogKeyDto.char(ch: 'k'),
            ),
            command: KeyboardCommandDto.search(),
          ),
        ],
      ),
      BindingEntryDto(
        labelKey: 'keys.cmd.switch_board',
        requiresToolFocus: false,
        bindings: [
          for (var slot = 1; slot <= 9; slot++)
            _plainChar('$slot', KeyboardCommandDto.switchBoard(slot: slot)),
        ],
      ),
      BindingEntryDto(
        labelKey: 'keys.cmd.toggle_pin',
        requiresToolFocus: true,
        bindings: [_plainChar('p', const KeyboardCommandDto.togglePin())],
      ),
      BindingEntryDto(
        labelKey: 'keys.cmd.edit_pin_color',
        requiresToolFocus: true,
        bindings: [_plainChar('c', const KeyboardCommandDto.editPinColor())],
      ),
      BindingEntryDto(
        labelKey: 'keys.cmd.start_resize',
        requiresToolFocus: true,
        bindings: [_plainChar('e', const KeyboardCommandDto.startResize())],
      ),
      BindingEntryDto(
        labelKey: 'keys.cmd.cheatsheet',
        requiresToolFocus: false,
        bindings: [_plainChar('?', const KeyboardCommandDto.showCheatsheet())],
      ),
    ],
  ),
  ScopeBindingsDto(
    scope: KeyboardScopeDto.resize,
    labelKey: 'keys.scope.resize',
    entries: [
      BindingEntryDto(
        labelKey: 'keys.cmd.reset_span',
        requiresToolFocus: false,
        bindings: [_plainChar('0', const KeyboardCommandDto.resetSpan())],
      ),
      BindingEntryDto(
        labelKey: 'keys.cmd.cancel',
        requiresToolFocus: false,
        bindings: [
          _plain(const CatalogKeyDto.esc(), const KeyboardCommandDto.cancel()),
        ],
      ),
    ],
  ),
];
