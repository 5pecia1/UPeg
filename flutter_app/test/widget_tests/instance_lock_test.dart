/// Dart-side tests for the OS instance-lock seam (inventory rows A01 / A02).
///
/// The FRB layer owns the lock acquisition + stale-cleanup logic in
/// `upeg-frb/src/platform/instance_lock.rs`; those native paths already
/// have Rust unit coverage. What this file locks down is that Dart code
/// actually CALLS the FRB seam at the right moments — namely:
///   1. App boot: `initApp` is the only entry to the lock, and the
///      Flutter side calls it via `appInitRunnerProvider`.
///   2. Tray Quit: `runTrayQuit` MUST invoke `shutdown` before the
///      window closes so the lock file disappears even when the
///      process is torn down by the user via the tray.
///
/// Both seams are already overridable (`appInitRunnerProvider` for boot
/// and the `shutdown:` parameter of `runTrayQuit` for quit); these tests
/// pin the contract so future refactors can't silently drop the calls.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/app.dart';
import 'package:upeg/src/pages/splash_page.dart';
import 'package:upeg/src/platform/tray.dart';

import '../test_helpers/i18n_test_catalog.dart';

void main() {
  group('App boot ↔ instance lock seam', () {
    testWidgets('app_boot_delegates_the_instance_lock_path_to_frb', (
      tester,
    ) async {
      // The `appInitRunnerProvider` IS the Dart-side instance-lock
      // boundary — `initApp()` (the default runner) is the only public
      // call that acquires the OS lock. If a future refactor drops the
      // override seam, app boot can no longer be intercepted and the
      // dylib gets loaded in widget tests. Lock both: the override is
      // honoured AND the override gets called exactly once during boot.
      var initCalls = 0;
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            ...i18nTestOverrides,
            appInitRunnerProvider.overrideWithValue(() async {
              initCalls += 1;
              // Never resolve — the splash stays on screen so the test
              // doesn't have to fabricate a full `AppInitReport`.
              throw _NeverResolves();
            }),
          ],
          child: const UpegApp(),
        ),
      );
      // One pump is enough — appInitProvider builds eagerly when
      // `UpegApp.build` watches it.
      await tester.pump();

      expect(
        initCalls,
        1,
        reason:
            'initApp (the lock acquisition path) must be called exactly once',
      );
      // Sanity: the splash is still on screen because the runner is hung.
      expect(find.byType(SplashPage), findsOneWidget);
    });

    test('tray_quit_invokes_shutdown_to_release_the_instance_lock', () async {
      // A01/A02 — the OS lock file lives at `~/.upeg/desktop.lock` and is
      // released by FRB `shutdown()`. The tray Quit path MUST invoke
      // `shutdown` even when the user-facing window close throws, so the
      // lock is always gone before the process is allowed to exit. The
      // ordering contract (shutdown first, then close) is enforced in
      // `tray_quit_test.dart`; here we lock the FRB-delegation half so
      // the seam can't be silently swapped for a Dart-side no-op.
      var shutdownCalls = 0;
      await runTrayQuit(
        shutdown: () => shutdownCalls += 1,
        closeWindow: () async {},
      );
      expect(
        shutdownCalls,
        1,
        reason: 'shutdown() (the FRB lock-release seam) must fire on tray Quit',
      );
    });
  });
}

/// Sentinel exception thrown by the test runner override so the boot
/// future fails fast without an extra Completer.never plumbing. We
/// don't pump past the first frame so the splash stays mounted.
class _NeverResolves implements Exception {
  const _NeverResolves();
}
