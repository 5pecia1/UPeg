/// Widget tests for [PopupAutoHideObserver] — the root-mounted
/// observer that translates `window_manager`'s `onWindowBlur` callback
/// into `windowManager.hide()` when (and only when) the popup window
/// is unpinned AND the active window mode is popup.
///
/// The [WindowHider] abstract class is the test seam: production code
/// uses [WindowManagerHider] (delegating to `windowManager.hide()`),
/// tests pass a [_RecordingHider] so the platform channel is never
/// touched. This keeps the test free of `tray_manager` /
/// `window_manager` mocking gymnastics.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/state/pinned_provider.dart';
import 'package:upeg/src/state/window_mode_provider.dart';
import 'package:upeg/src/widgets/popup_auto_hide_observer.dart';

class _RecordingHider extends WindowHider {
  int calls = 0;

  @override
  Future<void> hide() async {
    calls++;
  }
}

class _SeededWindowModeNotifier extends WindowModeNotifier {
  _SeededWindowModeNotifier(this._seed);

  final WindowMode _seed;

  @override
  WindowMode build() => _seed;
}

class _SeededPinnedNotifier extends PinnedNotifier {
  _SeededPinnedNotifier(this._seed);

  final bool _seed;

  @override
  bool build() => _seed;
}

Future<void> _pumpObserver(
  WidgetTester tester, {
  required WindowMode mode,
  required bool pinned,
  required WindowHider hider,
}) async {
  await tester.pumpWidget(
    ProviderScope(
      overrides: [
        windowModeProvider.overrideWith(() => _SeededWindowModeNotifier(mode)),
        pinnedProvider.overrideWith(() => _SeededPinnedNotifier(pinned)),
      ],
      child: MaterialApp(
        home: PopupAutoHideObserver(
          hider: hider,
          child: const SizedBox.shrink(),
        ),
      ),
    ),
  );
}

void main() {
  group('PopupAutoHideObserver', () {
    testWidgets(
      'popupautohideobserver_calls_hide_on_blur_in_popup_mode_when_unpinned',
      (tester) async {
        final hider = _RecordingHider();
        await _pumpObserver(
          tester,
          mode: WindowMode.popup,
          pinned: false,
          hider: hider,
        );

        final observer = tester.widget<PopupAutoHideObserver>(
          find.byType(PopupAutoHideObserver),
        );
        observer.debugTriggerBlur();
        await tester.pumpAndSettle();

        expect(hider.calls, 1);
      },
    );

    testWidgets(
      'popupautohideobserver_does_not_call_hide_on_blur_when_pinned',
      (tester) async {
        final hider = _RecordingHider();
        await _pumpObserver(
          tester,
          mode: WindowMode.popup,
          pinned: true,
          hider: hider,
        );

        final observer = tester.widget<PopupAutoHideObserver>(
          find.byType(PopupAutoHideObserver),
        );
        observer.debugTriggerBlur();
        await tester.pumpAndSettle();

        expect(hider.calls, 0);
      },
    );

    testWidgets(
      'popupautohideobserver_does_not_call_hide_on_blur_in_full_mode',
      (tester) async {
        final hider = _RecordingHider();
        await _pumpObserver(
          tester,
          mode: WindowMode.full,
          pinned: false,
          hider: hider,
        );

        final observer = tester.widget<PopupAutoHideObserver>(
          find.byType(PopupAutoHideObserver),
        );
        observer.debugTriggerBlur();
        await tester.pumpAndSettle();

        expect(hider.calls, 0);
      },
    );
  });
}
