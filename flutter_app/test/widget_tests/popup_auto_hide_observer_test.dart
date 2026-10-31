/// Widget tests for [PopupAutoHideObserver] — the root-mounted
/// observer that translates `window_manager`'s `onWindowBlur` callback
/// into `windowManager.hide()` when (and only when) the popup window
/// is unpinned AND the active window mode is popup, one
/// [kPopupAutoHideGrace] after the blur and only if the window is still
/// unfocused then.
///
/// The [WindowHider] abstract class is the test seam: production code
/// uses [WindowManagerHider] (delegating to `windowManager.hide()` and
/// `windowManager.isFocused()`), tests pass a [_RecordingHider] so the
/// platform channel is never touched. Blur, focus and hide events reach the
/// observer the way the native plugin sends them, through the
/// `window_manager` channel; `debugTriggerBlur` covers the plain blur cases.
library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/state/pinned_provider.dart';
import 'package:upeg/src/state/window_mode_provider.dart';
import 'package:upeg/src/widgets/popup_auto_hide_observer.dart';

class _RecordingHider extends WindowHider {
  _RecordingHider({this.focused = false});

  /// What the window reports when the observer asks after the grace period.
  final bool focused;
  int hideCalls = 0;
  int focusChecks = 0;

  @override
  Future<void> hide() async {
    hideCalls++;
  }

  @override
  Future<bool> isFocused() async {
    focusChecks++;
    return focused;
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

/// Delivers [eventName] ('blur', 'focus', 'hide', 'show') to the
/// `window_manager` listeners the way the native plugin does.
Future<void> _emitWindowEvent(String eventName) {
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  return messenger.handlePlatformMessage(
    'window_manager',
    const StandardMethodCodec().encodeMethodCall(
      MethodCall('onEvent', {'eventName': eventName}),
    ),
    (_) {},
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
        await tester.pump(kPopupAutoHideGrace);

        expect(hider.hideCalls, 1);
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
        await tester.pump(kPopupAutoHideGrace);

        expect(hider.hideCalls, 0);
        expect(hider.focusChecks, 0, reason: 'a pinned popup arms no timer');
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
        await tester.pump(kPopupAutoHideGrace);

        expect(hider.hideCalls, 0);
        expect(hider.focusChecks, 0, reason: 'full mode arms no timer');
      },
    );

    testWidgets(
      'popupautohideobserver_waits_out_the_grace_period_before_hiding',
      (tester) async {
        // The hotkey toggle relies on this: its key-down callback runs after
        // the grab-induced blur and must still find the popup shown.
        final hider = _RecordingHider();
        await _pumpObserver(
          tester,
          mode: WindowMode.popup,
          pinned: false,
          hider: hider,
        );

        await _emitWindowEvent('blur');
        await tester.pump(
          kPopupAutoHideGrace - const Duration(milliseconds: 1),
        );

        expect(hider.hideCalls, 0);

        await tester.pump(const Duration(milliseconds: 1));

        expect(hider.hideCalls, 1);
      },
    );

    testWidgets(
      'popupautohideobserver_does_not_hide_when_focus_returns_within_the_grace_period',
      (tester) async {
        final hider = _RecordingHider();
        await _pumpObserver(
          tester,
          mode: WindowMode.popup,
          pinned: false,
          hider: hider,
        );

        await _emitWindowEvent('blur');
        await tester.pump(kPopupAutoHideGrace ~/ 2);
        await _emitWindowEvent('focus');
        await tester.pump(kPopupAutoHideGrace * 2);

        expect(hider.hideCalls, 0);
        expect(hider.focusChecks, 0, reason: 'focus cancels the timer');
      },
    );

    testWidgets(
      'popupautohideobserver_does_not_hide_when_the_window_is_focused_at_the_check',
      (tester) async {
        // No focus event arrived, but the window reports focus when the
        // grace period ends: it must stay.
        final hider = _RecordingHider(focused: true);
        await _pumpObserver(
          tester,
          mode: WindowMode.popup,
          pinned: false,
          hider: hider,
        );

        await _emitWindowEvent('blur');
        await tester.pump(kPopupAutoHideGrace);

        expect(hider.focusChecks, 1);
        expect(hider.hideCalls, 0);
      },
    );

    testWidgets(
      'popupautohideobserver_a_window_hide_event_cancels_the_pending_hide',
      (tester) async {
        // Two quick hotkey presses: the first press's grab blur arms the
        // timer and its toggle hides the popup; the second press shows the
        // popup again while its own key grab still reports no focus. The
        // first timer must not outlive the hide and close the shown popup.
        final hider = _RecordingHider();
        await _pumpObserver(
          tester,
          mode: WindowMode.popup,
          pinned: false,
          hider: hider,
        );

        await _emitWindowEvent('blur');
        await _emitWindowEvent('hide');
        await _emitWindowEvent('show');
        await tester.pump(kPopupAutoHideGrace * 2);

        expect(hider.focusChecks, 0, reason: 'the hide event cancels it');
        expect(hider.hideCalls, 0);
      },
    );

    testWidgets(
      'popupautohideobserver_does_not_hide_a_popup_pinned_during_the_grace_period',
      (tester) async {
        final hider = _RecordingHider();
        await _pumpObserver(
          tester,
          mode: WindowMode.popup,
          pinned: false,
          hider: hider,
        );

        await _emitWindowEvent('blur');
        ProviderScope.containerOf(
          tester.element(find.byType(PopupAutoHideObserver)),
        ).read(pinnedProvider.notifier).set(true);
        await tester.pump(kPopupAutoHideGrace);

        expect(hider.hideCalls, 0);
        expect(hider.focusChecks, 0, reason: 'pinned is checked again first');
      },
    );

    testWidgets(
      'popupautohideobserver_dispose_cancels_every_pending_hide_even_after_a_repeated_blur',
      (tester) async {
        final hider = _RecordingHider();
        await _pumpObserver(
          tester,
          mode: WindowMode.popup,
          pinned: false,
          hider: hider,
        );

        await _emitWindowEvent('blur');
        await _emitWindowEvent('blur');
        await tester.pumpWidget(const SizedBox.shrink());
        await tester.pump(kPopupAutoHideGrace * 2);

        expect(hider.focusChecks, 0);
        expect(hider.hideCalls, 0);
      },
    );
  });
}
