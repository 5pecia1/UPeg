/// Unit tests for the Quit tray handler. The production handler must
/// call FRB `shutdown()` AND close the OS window — those are two
/// platform-coupled side effects that have to fire together. Inject
/// recording shims so the call order and call set are verifiable
/// without touching `window_manager` / FRB platform channels.
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/platform/tray.dart';

void main() {
  group('runTrayQuit', () {
    test('Tray_Quit는_shutdown과_windowClose를_순서대로_호출한다', () async {
      final calls = <String>[];
      await runTrayQuit(
        shutdown: () => calls.add('shutdown'),
        closeWindow: () async => calls.add('windowClose'),
      );
      expect(calls, ['shutdown', 'windowClose']);
    });

    test('Tray_Quit는_windowClose_실패에도_shutdown은_이미_호출되어_있다', () async {
      final calls = <String>[];
      await expectLater(
        runTrayQuit(
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
