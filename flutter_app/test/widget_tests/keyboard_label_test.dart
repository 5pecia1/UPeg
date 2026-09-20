/// Unit tests for the platform-aware keyboard label helper.
///
/// The helper picks `⌘` on macOS and `Ctrl` elsewhere. Tests pass
/// the [`TargetPlatform`] directly so we don't have to touch the
/// foundation debug-var (which trips `_verifyInvariants` on every
/// widget-test pump). Widget tests that need to pin a platform
/// override [`keyboardPlatformProvider`] instead.
library;

import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/platform/keyboard_label.dart';

void main() {
  group('keyboard_label (CC)', () {
    test('primarymodifierlabel_returns_the_command_glyph_on_macos', () {
      expect(primaryModifierLabel(TargetPlatform.macOS), '⌘');
    });

    test('primarymodifierlabel_returns_ctrl_on_linux', () {
      expect(primaryModifierLabel(TargetPlatform.linux), 'Ctrl');
    });

    test('primarymodifierlabel_returns_ctrl_on_windows', () {
      expect(primaryModifierLabel(TargetPlatform.windows), 'Ctrl');
    });

    test('shortcutlabel_joins_modifier_and_key_on_macos', () {
      expect(shortcutLabel(TargetPlatform.macOS, 'K'), '⌘K');
      expect(shortcutLabel(TargetPlatform.macOS, '↵'), '⌘↵');
    });

    test('shortcutlabel_separates_modifier_and_key_with_a_space_on_linux', () {
      expect(shortcutLabel(TargetPlatform.linux, 'K'), 'Ctrl K');
      expect(shortcutLabel(TargetPlatform.linux, '↵'), 'Ctrl ↵');
    });
  });
}
