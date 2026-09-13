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
    test('보이는_창은_hide로_판정한다', () {
      const snapshot = WindowSnapshot(isVisible: true, isMinimized: false);
      expect(decideSummonToggle(snapshot), SummonToggleDecision.hideWindow);
    });

    test('숨겨진_창은_popup_summon으로_판정한다', () {
      const snapshot = WindowSnapshot(isVisible: false, isMinimized: false);
      expect(decideSummonToggle(snapshot), SummonToggleDecision.summonPopup);
    });

    test('최소화된_창은_visible이어도_popup_summon으로_판정한다', () {
      const snapshot = WindowSnapshot(isVisible: true, isMinimized: true);
      expect(decideSummonToggle(snapshot), SummonToggleDecision.summonPopup);
    });
  });

  group('summonPlan', () {
    test('최소화_상태면_restore_show_focus_순서로_전이한다', () {
      expect(summonPlan(isMinimized: true), [
        SummonStep.restore,
        SummonStep.show,
        SummonStep.focus,
      ]);
    });

    test('최소화가_아니면_show_focus만_수행한다', () {
      expect(summonPlan(isMinimized: false), [
        SummonStep.show,
        SummonStep.focus,
      ]);
    });
  });

  group('summonWindow', () {
    test('숨겨진_창을_show하고_focus한다', () async {
      final driver = _RecordingSummonDriver(visible: false, minimized: false);

      await summonWindow(driver: driver);

      expect(driver.calls, ['isVisible', 'isMinimized', 'show', 'focus']);
    });

    test('최소화된_창은_restore부터_수행한다', () async {
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
    test('보이는_창은_숨기고_window_mode는_바꾸지_않는다', () async {
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

    test('숨겨진_창은_popup_mode로_전환하고_summon한다', () async {
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

    test('최소화된_창은_restore를_거쳐_popup으로_summon한다', () async {
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
