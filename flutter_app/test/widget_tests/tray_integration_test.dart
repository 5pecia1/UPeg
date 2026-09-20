/// Integration tests for the tray menu dispatch. Each tray menu click
/// must route through a Riverpod provider (`togglePausedProvider`,
/// `pinnedProvider`) so widget tests can intercept the FRB call
/// without touching the platform channel.
///
/// The Quit handler is covered separately in `tray_quit_test.dart` via
/// the [runTrayQuit] injection points.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/platform/tray.dart';
import 'package:upeg/src/rust/api/pause.dart' as frb;
import 'package:upeg/src/state/pause_control_provider.dart';
import 'package:upeg/src/state/pinned_provider.dart';
import 'package:upeg/src/state/pause_provider.dart';

void main() {
  group('UpegTray.onTrayMenuItemClick', () {
    test('Tray_TogglePause_calls_the_togglePaused_provider_when_embedded', () {
      // Pause is a process-local `AtomicBool` (WS4 PRD §5.9) — the
      // dispatch only fires when THIS process hosts in-process
      // (`pauseControllableProvider == true`). Override it directly so
      // this test stays independent of the real host-state stream.
      var calls = 0;
      final container = ProviderContainer(
        overrides: [
          pauseControllableProvider.overrideWithValue(true),
          togglePausedProvider.overrideWithValue(() {
            calls += 1;
            return const frb.PausedStateSnapshotDto(
              state: frb.PausedStateDto.paused,
            );
          }),
        ],
      );
      addTearDown(container.dispose);

      dispatchTrayCommand(container, TrayCommand.togglePause);

      expect(calls, 1);
    });

    test('dispatchTrayCommand_togglePause_is_a_no_op_when_not_embedded', () {
      // Split-brain guard: when this process is attached to a separate
      // daemon (or has no host), pause cannot reach that foreign
      // process, so the dispatch must not call the mutator at all.
      var calls = 0;
      final container = ProviderContainer(
        overrides: [
          pauseControllableProvider.overrideWithValue(false),
          togglePausedProvider.overrideWithValue(() {
            calls += 1;
            return const frb.PausedStateSnapshotDto(
              state: frb.PausedStateDto.paused,
            );
          }),
        ],
      );
      addTearDown(container.dispose);

      dispatchTrayCommand(container, TrayCommand.togglePause);

      expect(calls, 0);
    });

    test('the_Tray_TogglePin_callback_toggles_pinnedProvider', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      expect(container.read(pinnedProvider), isFalse);

      dispatchTrayCommand(container, TrayCommand.togglePin);
      expect(container.read(pinnedProvider), isTrue);

      dispatchTrayCommand(container, TrayCommand.togglePin);
      expect(container.read(pinnedProvider), isFalse);
    });
  });
}
