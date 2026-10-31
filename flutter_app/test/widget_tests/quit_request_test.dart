/// Unit tests for the shared quit-request pipeline behind keyboard `q`
/// and the intercepted OS close (X) button. The pipeline is pure
/// orchestration — confirm, draft-flush, and quit effects are injected — so
/// the interception decision is verifiable without any platform channel.
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

  group('runQuitWithDraftFlush', () {
    test('flushes_pending_drafts_before_the_quit_effects_run', () async {
      final calls = <String>[];
      await runQuitWithDraftFlush(
        flushDrafts: () => calls.add('flush'),
        quit: () => runAppQuit(
          shutdown: () => calls.add('shutdown'),
          closeWindow: () async => calls.add('destroy'),
        ),
      );
      // Drafts are written before the instance lock is released.
      expect(calls, ['flush', 'shutdown', 'destroy']);
    });

    test('a_denied_confirm_flushes_nothing', () async {
      final calls = <String>[];
      await runQuitRequest(
        confirm: () async => false,
        quit: () => runQuitWithDraftFlush(
          flushDrafts: () => calls.add('flush'),
          quit: () async => calls.add('quit'),
        ),
      );
      expect(calls, isEmpty);
    });
  });
}
