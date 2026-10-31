/// Unit tests for the tray Quit item. The tray has no quit flow of its
/// own: Quit goes through `quitAppProvider`, the pipeline shared with
/// the keyboard shortcut and the window close button. `runAppQuit` is
/// that pipeline's default implementation; it must call FRB `shutdown()`
/// AND close the OS window — two platform-coupled side effects that have
/// to fire together, shutdown first. Inject recording shims so the call
/// order and call set are verifiable without touching `window_manager`
/// / FRB platform channels.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/platform/app_lifecycle.dart';
import 'package:upeg/src/platform/tray.dart';

void main() {
  group('dispatchTrayCommand quit', () {
    test('Tray_Quit_runs_the_quitAppProvider_pipeline_exactly_once', () {
      var quitCalls = 0;
      final container = ProviderContainer(
        overrides: [
          quitAppProvider.overrideWithValue(() async {
            quitCalls += 1;
          }),
        ],
      );
      addTearDown(container.dispose);

      dispatchTrayCommand(container, TrayCommand.quit);

      expect(quitCalls, 1);
    });
  });

  group('runAppQuit', () {
    test('Quit_calls_shutdown_and_windowClose_in_order', () async {
      final calls = <String>[];
      await runAppQuit(
        shutdown: () => calls.add('shutdown'),
        closeWindow: () async => calls.add('windowClose'),
      );
      expect(calls, ['shutdown', 'windowClose']);
    });

    test('Quit_has_already_called_shutdown_when_windowClose_fails', () async {
      final calls = <String>[];
      await expectLater(
        runAppQuit(
          shutdown: () => calls.add('shutdown'),
          closeWindow: () async {
            calls.add('windowClose');
            throw Exception('window close blew up');
          },
        ),
        throwsException,
      );
      // shutdown must run before close; even when close throws, the
      // FRB-side instance lock must already be released.
      expect(calls, ['shutdown', 'windowClose']);
    });
  });
}
