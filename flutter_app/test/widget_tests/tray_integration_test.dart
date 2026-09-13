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
    test('Tray_TogglePause는_embedded에서_togglePaused_프로바이더를_호출한다', () {
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

    test('dispatchTrayCommand_togglePause는_비embedded에서_no_op이다', () {
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

    test('Tray_TogglePin_콜백은_pinnedProvider를_토글한다', () {
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
