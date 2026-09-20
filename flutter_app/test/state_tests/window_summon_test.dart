/// Launcher summon toggle — pure judgment + state transition tests.
///
/// The global hotkey itself cannot run in headless CI, so the toggle
/// decision ([decideSummonToggle]), the summon plan ([summonPlan]) and
/// the driver-facing executors are exercised through a recording
/// [WindowSummonDriver] — no platform channel is ever touched.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/platform/window_summon.dart';
import 'package:upeg/src/state/window_mode_provider.dart';

class _RecordingSummonDriver extends WindowSummonDriver {
  _RecordingSummonDriver({required this.visible, required this.minimized});

  final bool visible;
  final bool minimized;
  final calls = <String>[];

  @override
  Future<bool> isVisible() async {
    calls.add('isVisible');
    return visible;
  }

  @override
  Future<bool> isMinimized() async {
    calls.add('isMinimized');
    return minimized;
  }

  @override
  Future<void> restore() async {
    calls.add('restore');
  }

  @override
  Future<void> show() async {
    calls.add('show');
  }

  @override
  Future<void> focus() async {
    calls.add('focus');
  }

  @override
  Future<void> hide() async {
    calls.add('hide');
  }
}

void main() {
  group('decideSummonToggle', () {
    test('visible_window_decides_hide', () {
      const snapshot = WindowSnapshot(isVisible: true, isMinimized: false);
      expect(decideSummonToggle(snapshot), SummonToggleDecision.hideWindow);
    });

    test('hidden_window_decides_popup_summon', () {
      const snapshot = WindowSnapshot(isVisible: false, isMinimized: false);
      expect(decideSummonToggle(snapshot), SummonToggleDecision.summonPopup);
    });

    test('minimized_window_decides_popup_summon_even_when_visible', () {
      const snapshot = WindowSnapshot(isVisible: true, isMinimized: true);
      expect(decideSummonToggle(snapshot), SummonToggleDecision.summonPopup);
    });
  });

  group('summonPlan', () {
    test('minimized_transitions_through_restore_show_focus_order', () {
      expect(summonPlan(isMinimized: true), [
        SummonStep.restore,
        SummonStep.show,
        SummonStep.focus,
      ]);
    });

    test('not_minimized_runs_only_show_focus', () {
      expect(summonPlan(isMinimized: false), [
        SummonStep.show,
        SummonStep.focus,
      ]);
    });
  });

  group('summonWindow', () {
    test('shows_and_focuses_hidden_window', () async {
      final driver = _RecordingSummonDriver(visible: false, minimized: false);

      await summonWindow(driver: driver);

      expect(driver.calls, ['isVisible', 'isMinimized', 'show', 'focus']);
    });

    test('minimized_window_restores_first', () async {
      final driver = _RecordingSummonDriver(visible: false, minimized: true);

      await summonWindow(driver: driver);

      expect(driver.calls, [
        'isVisible',
        'isMinimized',
        'restore',
        'show',
        'focus',
      ]);
    });
  });

  group('runSummonToggle', () {
    test('visible_window_hides_without_changing_window_mode', () async {
      final container = ProviderContainer(
        overrides: [
          windowModeProvider.overrideWith(
            () => WindowModeNotifier(initial: WindowMode.full),
          ),
        ],
      );
      addTearDown(container.dispose);
      final driver = _RecordingSummonDriver(visible: true, minimized: false);

      await runSummonToggle(container, driver: driver);

      expect(driver.calls, contains('hide'));
      expect(driver.calls, isNot(contains('show')));
      expect(container.read(windowModeProvider), WindowMode.full);
    });

    test('hidden_window_switches_to_popup_mode_and_summons', () async {
      final container = ProviderContainer(
        overrides: [
          windowModeProvider.overrideWith(
            () => WindowModeNotifier(initial: WindowMode.full),
          ),
        ],
      );
      addTearDown(container.dispose);
      final driver = _RecordingSummonDriver(visible: false, minimized: false);

      await runSummonToggle(container, driver: driver);

      expect(container.read(windowModeProvider), WindowMode.popup);
      expect(driver.calls, containsAllInOrder(['show', 'focus']));
      expect(driver.calls, isNot(contains('hide')));
    });

    test('minimized_window_summons_to_popup_via_restore', () async {
      final container = ProviderContainer(
        overrides: [
          windowModeProvider.overrideWith(
            () => WindowModeNotifier(initial: WindowMode.popup),
          ),
        ],
      );
      addTearDown(container.dispose);
      final driver = _RecordingSummonDriver(visible: true, minimized: true);

      await runSummonToggle(container, driver: driver);

      expect(container.read(windowModeProvider), WindowMode.popup);
      expect(driver.calls, containsAllInOrder(['restore', 'show', 'focus']));
    });
  });
}
