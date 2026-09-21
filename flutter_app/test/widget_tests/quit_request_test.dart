/// Unit tests for the shared quit-request pipeline behind keyboard `q`
/// and the intercepted OS close (X) button. The pipeline is pure
/// orchestration — confirm and quit effects are injected — so the
/// interception decision is verifiable without any platform channel.
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/platform/app_lifecycle.dart';

void main() {
  group('runQuitRequest', () {
    test('does_not_quit_before_confirmation', () async {
      var quitCalls = 0;
      await runQuitRequest(
        confirm: () async => false,
        quit: () async {
          quitCalls += 1;
        },
      );
      expect(quitCalls, 0);
    });

    test('quit_is_called_exactly_once_when_confirmed', () async {
      var quitCalls = 0;
      await runQuitRequest(
        confirm: () async => true,
        quit: () async {
          quitCalls += 1;
        },
      );
      expect(quitCalls, 1);
    });

    test(
      'a_confirmed_X_close_goes_through_shutdown_then_closes_the_window',
      () async {
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
      },
    );

    test(
      'when_confirm_is_denied_there_is_no_shutdown_or_window_close',
      () async {
        final calls = <String>[];
        await runQuitRequest(
          confirm: () async => false,
          quit: () => runAppQuit(
            shutdown: () => calls.add('shutdown'),
            closeWindow: () async => calls.add('destroy'),
          ),
        );
        expect(calls, isEmpty);
      },
    );
  });
}
