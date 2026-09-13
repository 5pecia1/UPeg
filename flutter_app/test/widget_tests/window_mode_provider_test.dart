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
    test('WindowMode_초기값은_full이다', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);
      expect(container.read(windowModeProvider), WindowMode.full);
    });

    test('toggle는_full_상태에서_popup으로_바꾼다', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);
      expect(container.read(windowModeProvider), WindowMode.full);

      final next = container.read(windowModeProvider.notifier).toggle();
      expect(next, WindowMode.popup);
      expect(container.read(windowModeProvider), WindowMode.popup);
    });

    test('toggle는_popup_상태에서_full로_바꾼다', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      container.read(windowModeProvider.notifier).set(WindowMode.popup);
      expect(container.read(windowModeProvider), WindowMode.popup);

      final next = container.read(windowModeProvider.notifier).toggle();
      expect(next, WindowMode.full);
    });

    test('set은_지정된_모드로_상태를_갱신한다', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      container.read(windowModeProvider.notifier).set(WindowMode.popup);
      expect(container.read(windowModeProvider), WindowMode.popup);

      container.read(windowModeProvider.notifier).set(WindowMode.full);
      expect(container.read(windowModeProvider), WindowMode.full);
    });
  });

  group('initialWindowMode', () {
    test('환경변수가_없으면_full을_반환한다', () {
      expect(initialWindowMode(env: const {}), WindowMode.full);
    });

    test('UPEG_DESKTOP_POPUP가_설정되면_popup을_반환한다', () {
      expect(
        initialWindowMode(env: const {'UPEG_DESKTOP_POPUP': '1'}),
        WindowMode.popup,
      );
    });

    test('UPEG_DESKTOP_POPUP가_빈_문자열이면_full을_반환한다', () {
      expect(
        initialWindowMode(env: const {'UPEG_DESKTOP_POPUP': ''}),
        WindowMode.full,
      );
    });
  });
}
