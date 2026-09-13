/// Root `MaterialApp`.
///
/// Owns the `initApp` future and routes to either the splash screen, an
/// error scaffold, or the [BoardPage] depending on the result. Kept thin
/// so router/feature flag hooks land here in later phases.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/boot/logging.dart';
import 'package:upeg/src/features/controlled_embed/providers.dart';
import 'package:upeg/src/widgets/controlled_embed/session_host.dart';
import 'package:upeg/src/pages/board_page.dart';
import 'package:upeg/src/pages/popup_page.dart';
import 'package:upeg/src/pages/splash_page.dart';
import 'package:upeg/src/platform/deep_link_listener.dart';
import 'package:upeg/src/platform/global_hotkey.dart';
import 'package:upeg/src/platform/tray.dart';
import 'package:upeg/src/rust/api/boot.dart';
import 'package:upeg/src/state/launch_intent_provider.dart';
import 'package:upeg/src/state/shared_state_versions.dart';
import 'package:upeg/src/state/theme_mode_provider.dart';
import 'package:upeg/src/state/tweaks_provider.dart';
import 'package:upeg/src/state/window_mode_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/launch_intent_applier.dart';
import 'package:upeg/src/widgets/popup_auto_hide_observer.dart';
import 'package:upeg/src/widgets/window_mode_applier.dart';

/// Indirection so widget tests can override the boot call without loading
/// the native dylib.
typedef AppInitRunner = Future<AppInitReport> Function();

final appInitRunnerProvider = Provider<AppInitRunner>((ref) => initApp);

typedef DesktopIntegrationsInstaller = Future<void> Function(Ref ref);

final appBootLogProvider = Provider<BootLog>((ref) => defaultBootLog);

final desktopIntegrationsInstallerProvider =
    Provider<DesktopIntegrationsInstaller>(
      (ref) => _installDesktopIntegrations,
    );

final appInitProvider = FutureProvider<AppInitReport>((ref) async {
  final run = ref.watch(appInitRunnerProvider);
  final log = ref.watch(appBootLogProvider);
  final installDesktopIntegrations = ref.watch(
    desktopIntegrationsInstallerProvider,
  );
  log('upeg: appInitProvider — calling initApp()');
  final report = await run();
  log('upeg: appInitProvider — initApp returned, version=${report.version}');

  // Post-init wiring is best-effort: tray/window/deep-link failures must
  // NOT keep the splash spinner on screen forever. Each subsystem is
  // wrapped in a per-call timeout + try/catch so a hang or a missing
  // platform binding degrades to "no integration" rather than to
  // "blank window forever".
  await installDesktopIntegrations(ref);
  log('upeg: appInitProvider — desktop integrations done, painting BoardPage');
  return report;
});

Future<void> _installDesktopIntegrations(Ref ref) async {
  if (ref.read(controlledEmbedNativeSupportedProvider)) {
    await _bestEffort(
      'ControlledEmbed WebView provider',
      () => ref.read(controlledEmbedBridgeProvider).ready,
    );
  }
  // Geometry application moved to `WindowModeApplier` (C03) — the
  // observer at `MaterialApp.home` watches `windowModeProvider` and
  // applies geometry on every change (including the initial mount),
  // so we no longer drive `applyWindowMode` from here.
  await _bestEffort('UpegTray.install', () => UpegTray.install(ref));
  // Launcher immediacy: system-wide summon hotkey (Ctrl+Alt+Space by
  // default — see `global_hotkey.dart`). Best-effort like the rest:
  // a Wayland session or a conflicting registration degrades to the
  // tray / deep-link summon paths.
  await _bestEffort(
    'UpegGlobalHotkey.install',
    () => UpegGlobalHotkey.install(ref),
  );
  await _bestEffort(
    'DeepLinkListener.install',
    () => DeepLinkListener.install(ref),
  );
  // Seed the cold-boot launch intent (PRD §5.9 / Batch E D02). When
  // argv carried a `upeg://open?...` URL, `LaunchIntentApplier` on
  // the BoardPage will see this on its first frame and dispatch the
  // tool. `currentLaunchIntent()` returns `null` when argv had no
  // deep link, so this is a no-op otherwise.
  await _bestEffort('seedLaunchIntent', () async {
    final dto = currentLaunchIntent();
    if (dto != null) {
      ref.read(launchIntentProvider.notifier).set(LaunchIntent.fromDto(dto));
    }
  });
}

/// Run a desktop-integration call with a 2-second wall-clock cap.
/// Failures and timeouts log a debug line and return — the rest of the
/// app must keep going. The most common hang is `tray_manager` on a
/// macOS host where the platform channel is silently unresponsive;
/// without this guard the splash spinner would never clear.
Future<void> _bestEffort(String name, Future<void> Function() fn) async {
  try {
    await fn().timeout(
      const Duration(seconds: 2),
      onTimeout: () {
        debugPrint('upeg: $name timed out after 2s; continuing without it');
      },
    );
    debugPrint('upeg: $name ok');
  } on Object catch (err, stack) {
    debugPrint('upeg: $name failed: $err');
    debugPrint(stack.toString());
  }
}

/// Route to the correct landing page based on the initial window mode.
/// PRD §5.9: popup → compact search-first PopupPage; full → BoardPage.
/// Phase 7 minimum — Phase 10 will add the tray-driven full↔popup live
/// transition.
Widget _routeForMode(WindowMode mode, AppInitReport report) {
  return switch (mode) {
    WindowMode.popup => const PopupPage(),
    WindowMode.full => BoardPage(report: report),
  };
}

const String _genericBootFailurePrefix = 'boot failed';
const String _alreadyRunningBootTitle = 'upeg is already running';

String _bootFailureMessage(Object err) {
  return switch (err) {
    FrbError_AlreadyRunning(pid: final pid) =>
      '$_alreadyRunningBootTitle. PID $pid already holds the desktop lock. '
          'Use the existing window or quit that process before starting upeg again.',
    FrbError_HostUnavailable() =>
      '$_genericBootFailurePrefix: host unavailable',
    FrbError_Validation(field: final field, reason: final reason) =>
      '$_genericBootFailurePrefix: $field $reason',
    FrbError_Io(message: final message) =>
      '$_genericBootFailurePrefix: $message',
    FrbError_Internal(message: final message) =>
      '$_genericBootFailurePrefix: $message',
    _ => '$_genericBootFailurePrefix: $err',
  };
}

class UpegApp extends ConsumerWidget {
  const UpegApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    ref.watch(sharedStatePollingProvider);
    final init = ref.watch(appInitProvider);
    final themeMode = ref.watch(themeModeProvider);
    final accent = ref.watch(accentProvider);
    return MaterialApp(
      title: 'upeg',
      // Both palettes are built via the same `forAccent` factory so
      // K03 (accent flip) re-paints the live MaterialApp without a
      // restart — the previous hardcoded `darkTheme/lightTheme` pair
      // was the bug. `accentProvider` derives the typed `Accent` from
      // `tweaksProvider`.
      theme: UpegTheme.forAccent(accent, brightness: Brightness.light),
      darkTheme: UpegTheme.forAccent(accent, brightness: Brightness.dark),
      // Dark-first default: dark on first run. `themeModeProvider` watches
      // `tweaksProvider` so a Tweaks edit re-renders with the new mode
      // without an app restart.
      themeMode: themeMode,
      debugShowCheckedModeBanner: false,
      builder: (context, child) =>
          ControlledEmbedSessionHost(child: child ?? const SizedBox.shrink()),
      // TrayMenuSync must be mounted OUTSIDE `init.when(data:)` — it has
      // to subscribe to `pauseControllableProvider` on the very first
      // synchronous build, before `appInitProvider`'s async body reaches
      // `UpegTray.install` (which does the initial, necessarily-stale
      // `ref.read`). Mounting it under `data:` would wire it up only
      // after the host-state stream's single event already landed,
      // silently reproducing the "Pause never re-enables" regression.
      home: TrayMenuSync(
        child: WindowModeApplier(
          child: PopupAutoHideObserver(
            child: init.when(
              loading: () => const SplashPage(),
              error: (err, _) =>
                  Scaffold(body: Center(child: Text(_bootFailureMessage(err)))),
              // LaunchIntentApplier wraps BOTH routed surfaces: a deep link
              // (`upeg://open?...`) that arrives while in popup mode must still
              // force WindowMode.full and dispatch, so the observer has to be
              // mounted above the popup↔board route switch, not inside BoardPage.
              data: (report) => LaunchIntentApplier(
                child: _routeForMode(ref.watch(windowModeProvider), report),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
