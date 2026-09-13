/// App lifecycle hooks shared by GUI shortcuts, tray commands, and the
/// intercepted OS window-close (X) button.
library;

import 'package:flutter/services.dart' show MissingPluginException;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:window_manager/window_manager.dart' show windowManager;

import 'package:upeg/src/platform/window.dart' show isWindowManagerSupported;
import 'package:upeg/src/rust/api/boot.dart' as frb_boot;

/// Synchronous shutdown hook. Defaults to the FRB `shutdown()` shim
/// which releases the OS instance lock. Tests pass a recording closure.
typedef ShutdownFn = void Function();

/// Window close hook. Defaults to `windowManager.destroy()`. Tests pass
/// a recording closure so the platform channel never gets called.
typedef CloseWindowFn = Future<void> Function();

/// Default shutdown that delegates to the FRB lifecycle hook.
void defaultAppShutdown() => frb_boot.shutdown();

/// Default window close that delegates to `window_manager`. Lives at
/// top-level so the tray and shortcut test harnesses can inject a
/// recording double without ever touching the real platform channel.
///
/// `destroy()` — not `close()` — because the desktop shell installs
/// [installWindowCloseIntercept] (`setPreventClose(true)`) to route the
/// OS X button through the quit-confirm dialog. A confirmed quit must
/// bypass that intercept; `close()` would only re-fire `onWindowClose`.
Future<void> defaultCloseWindow() => windowManager.destroy();

/// Run the Quit handler: release the instance lock via FRB, then close
/// the OS window. Order matters: shutdown must run before the window
/// closes so the on-disk lock is gone before the process exits.
Future<void> runAppQuit({
  ShutdownFn shutdown = defaultAppShutdown,
  CloseWindowFn closeWindow = defaultCloseWindow,
}) async {
  shutdown();
  await closeWindow();
}

typedef QuitAppFn = Future<void> Function();

final quitAppProvider = Provider<QuitAppFn>((ref) => runAppQuit);

/// Asks the user to confirm the pending quit. `true` means "quit now".
typedef ConfirmQuitFn = Future<bool> Function();

/// Shared quit-request pipeline for keyboard `q` / Cmd+Q and the
/// intercepted OS close button: quit ONLY when [confirm] resolves true.
/// Pure orchestration — both effects are injected, so unit tests can
/// verify the decision without any platform channel.
Future<void> runQuitRequest({
  required ConfirmQuitFn confirm,
  required QuitAppFn quit,
}) async {
  if (!await confirm()) return;
  await quit();
}

/// Install the OS close intercept (`setPreventClose(true)`) so the X
/// button fires `WindowListener.onWindowClose` instead of killing the
/// process behind `shutdown()`'s back (leaking the instance lock and
/// discovery entries). Progressive enhancement: unsupported targets and
/// plugin-less widget-test runs no-op.
Future<void> installWindowCloseIntercept() async {
  if (!isWindowManagerSupported) return;
  try {
    await windowManager.setPreventClose(true);
  } on MissingPluginException {
    // No native plugin registered (widget tests) — the X button then
    // keeps its direct-close behavior, matching the pre-intercept state.
  }
}
