/// Unit tests for the shared quit-request pipeline behind keyboard `q`
/// and the intercepted OS close (X) button. The pipeline is pure
/// orchestration — confirm and quit effects are injected — so the
/// interception decision is verifiable without any platform channel.
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/platform/app_lifecycle.dart';

void main() {
  group('runQuitRequest', () {
    test('확인_전에는_종료하지_않는다', () async {
      var quitCalls = 0;
      await runQuitRequest(
        confirm: () async => false,
        quit: () async {
          quitCalls += 1;
        },
      );
      expect(quitCalls, 0);
    });

    test('확인하면_quit이_정확히_한번_호출된다', () async {
      var quitCalls = 0;
      await runQuitRequest(
        confirm: () async => true,
        quit: () async {
          quitCalls += 1;
        },
      );
      expect(quitCalls, 1);
    });

    test('확인된_X_닫기는_shutdown을_거친_뒤_window를_닫는다', () async {
      final calls = <String>[];
      await runQuitRequest(
        confirm: () async => true,
        quit: () => runAppQuit(
          shutdown: () => calls.add('shutdown'),
          closeWindow: () async => calls.add('destroy'),
        ),
      );
      // Lock/discovery cleanup (shutdown) must precede the window
      // teardown — the X button can never bypass it.
      expect(calls, ['shutdown', 'destroy']);
    });

    test('confirm이_거부되면_shutdown도_window_닫기도_없다', () async {
      final calls = <String>[];
      await runQuitRequest(
        confirm: () async => false,
        quit: () => runAppQuit(
          shutdown: () => calls.add('shutdown'),
          closeWindow: () async => calls.add('destroy'),
        ),
      );
      expect(calls, isEmpty);
    });
  });
}
