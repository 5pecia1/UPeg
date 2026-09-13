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
    test('primaryModifierLabel은_macOS에서_command_glyph를_반환한다', () {
      expect(primaryModifierLabel(TargetPlatform.macOS), '⌘');
    });

    test('primaryModifierLabel은_linux에서_Ctrl을_반환한다', () {
      expect(primaryModifierLabel(TargetPlatform.linux), 'Ctrl');
    });

    test('primaryModifierLabel은_windows에서_Ctrl을_반환한다', () {
      expect(primaryModifierLabel(TargetPlatform.windows), 'Ctrl');
    });

    test('shortcutLabel은_macOS에서_modifier와_key를_붙여서_반환한다', () {
      expect(shortcutLabel(TargetPlatform.macOS, 'K'), '⌘K');
      expect(shortcutLabel(TargetPlatform.macOS, '↵'), '⌘↵');
    });

    test('shortcutLabel은_linux에서_modifier와_key를_공백으로_분리한다', () {
      expect(shortcutLabel(TargetPlatform.linux, 'K'), 'Ctrl K');
      expect(shortcutLabel(TargetPlatform.linux, '↵'), 'Ctrl ↵');
    });
  });
}
