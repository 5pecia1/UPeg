/// Unit tests for the window mode Notifier.
///
/// `tray_manager` / `window_manager` / `app_links` cannot be tested
/// here — they require platform channels that are not available in
/// widget tests. Integration coverage lands in Phase 9.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/platform/window.dart';
import 'package:upeg/src/state/window_mode_provider.dart';

void main() {
  group('WindowModeNotifier', () {
    test('WindowMode_defaults_to_full', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);
      expect(container.read(windowModeProvider), WindowMode.full);
    });

    test('toggle_switches_from_full_to_popup', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);
      expect(container.read(windowModeProvider), WindowMode.full);

      final next = container.read(windowModeProvider.notifier).toggle();
      expect(next, WindowMode.popup);
      expect(container.read(windowModeProvider), WindowMode.popup);
    });

    test('toggle_switches_from_popup_to_full', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      container.read(windowModeProvider.notifier).set(WindowMode.popup);
      expect(container.read(windowModeProvider), WindowMode.popup);

      final next = container.read(windowModeProvider.notifier).toggle();
      expect(next, WindowMode.full);
    });

    test('set_updates_the_state_to_the_given_mode', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      container.read(windowModeProvider.notifier).set(WindowMode.popup);
      expect(container.read(windowModeProvider), WindowMode.popup);

      container.read(windowModeProvider.notifier).set(WindowMode.full);
      expect(container.read(windowModeProvider), WindowMode.full);
    });
  });

  group('initialWindowMode', () {
    test('returns_full_when_the_env_var_is_absent', () {
      expect(initialWindowMode(env: const {}), WindowMode.full);
    });

    test('returns_popup_when_UPEG_DESKTOP_POPUP_is_set', () {
      expect(
        initialWindowMode(env: const {'UPEG_DESKTOP_POPUP': '1'}),
        WindowMode.popup,
      );
    });

    test('returns_full_when_UPEG_DESKTOP_POPUP_is_an_empty_string', () {
      expect(
        initialWindowMode(env: const {'UPEG_DESKTOP_POPUP': ''}),
        WindowMode.full,
      );
    });
  });
}
