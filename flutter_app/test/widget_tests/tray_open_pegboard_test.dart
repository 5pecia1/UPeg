/// Tray "Open Pegboard".
///
/// The old "Open dashboard" item only wrote `WindowMode.full`. That does
/// nothing for a window that is already `full` but hidden or minimized:
/// `WindowModeApplier` dedupes equal modes, so nothing shows the window.
/// `openPegboardFromTray` therefore also runs the unconditional summon.
/// The summon is injected, so no test here touches `window_manager`.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/platform/tray.dart';
import 'package:upeg/src/state/window_mode_provider.dart';

ProviderContainer _containerStartingIn(WindowMode mode) {
  final container = ProviderContainer(
    overrides: [
      windowModeProvider.overrideWith(() => WindowModeNotifier(initial: mode)),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

void main() {
  group('openPegboardFromTray', () {
    test('from_popup_it_forces_full_and_then_summons_once', () async {
      final container = _containerStartingIn(WindowMode.popup);
      final modesSeenBySummon = <WindowMode>[];

      await openPegboardFromTray(
        container,
        summon: () async {
          modesSeenBySummon.add(container.read(windowModeProvider));
        },
      );

      // One summon, and by the time it ran the mode was already full.
      expect(modesSeenBySummon, [WindowMode.full]);
      expect(container.read(windowModeProvider), WindowMode.full);
    });

    test('a_hidden_window_that_is_already_full_is_still_summoned', () async {
      // The no-op case of the old item: the mode write changes nothing,
      // so the summon is the only thing that can bring the window back.
      final container = _containerStartingIn(WindowMode.full);
      var summonCalls = 0;

      await openPegboardFromTray(
        container,
        summon: () async {
          summonCalls += 1;
        },
      );

      expect(summonCalls, 1);
      expect(container.read(windowModeProvider), WindowMode.full);
    });
  });
}
