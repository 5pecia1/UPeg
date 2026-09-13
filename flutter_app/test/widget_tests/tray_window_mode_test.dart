/// Tray menu ↔ window mode transitions (inventory rows B03 / B04).
///
/// B03 ("Open dashboard → WindowMode::Full") and B04 ("Toggle popup ↔
/// full") are wired in `platform/tray.dart` via `UpegTray._setMode` and
/// `_toggleMode`. The production switch in `onTrayMenuItemClick`
/// performs the state write AND the platform `applyWindowMode` call;
/// both rows were ⚠️ "no integration test" because the latter requires
/// `window_manager`. We lock down the typed half — the
/// `WindowModeNotifier` transitions — by driving the public
/// `set`/`toggle` API the tray dispatch uses.
///
/// These tests intentionally do NOT cross the platform-channel boundary;
/// the geometry application path is locked down separately in C03's
/// `WindowModeApplier` test seam.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/state/window_mode_provider.dart';

void main() {
  group('Tray menu ↔ WindowMode transitions', () {
    test('Tray_OpenDashboard은_WindowMode를_full로_바꾼다', () {
      // Mirrors `tray.dart::_setMode(WindowMode.full)` — the only path
      // the tray's `Open dashboard` menu item drives. Start from popup
      // (the non-default state) so the transition is observable.
      final container = ProviderContainer(
        overrides: [
          windowModeProvider.overrideWith(
            () => WindowModeNotifier(initial: WindowMode.popup),
          ),
        ],
      );
      addTearDown(container.dispose);
      expect(container.read(windowModeProvider), WindowMode.popup);

      container.read(windowModeProvider.notifier).set(WindowMode.full);

      expect(
        container.read(windowModeProvider),
        WindowMode.full,
        reason: 'Open dashboard must force WindowMode.full',
      );
    });

    test('Tray_TogglePopup은_full에서_popup으로_전환한다', () {
      // Mirrors `tray.dart::_toggleMode` — `WindowModeNotifier.toggle`
      // is the typed flip the tray's `Toggle popup ↔ full` item uses.
      final container = ProviderContainer(
        overrides: [
          windowModeProvider.overrideWith(
            () => WindowModeNotifier(initial: WindowMode.full),
          ),
        ],
      );
      addTearDown(container.dispose);
      expect(container.read(windowModeProvider), WindowMode.full);

      final next = container.read(windowModeProvider.notifier).toggle();

      expect(next, WindowMode.popup);
      expect(
        container.read(windowModeProvider),
        WindowMode.popup,
        reason: 'Toggle popup ↔ full from Full must land in popup',
      );
    });

    test('Tray_TogglePopup은_popup에서_full로_되돌린다', () {
      // Round-trip lock: toggling twice from full lands back on full.
      // Pins the symmetry of the `toggle` switch so a future refactor
      // can't accidentally short-circuit one of the arms.
      final container = ProviderContainer(
        overrides: [
          windowModeProvider.overrideWith(
            () => WindowModeNotifier(initial: WindowMode.full),
          ),
        ],
      );
      addTearDown(container.dispose);

      container.read(windowModeProvider.notifier).toggle();
      final next = container.read(windowModeProvider.notifier).toggle();

      expect(next, WindowMode.full);
      expect(container.read(windowModeProvider), WindowMode.full);
    });
  });
}
