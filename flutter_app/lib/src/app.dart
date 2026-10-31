/// Root `MaterialApp`.
///
/// Owns the `initApp` future and routes to either the splash screen, an
/// error scaffold, or the [BoardPage] depending on the result. Kept thin
/// so router/feature flag hooks land here in later phases.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/boot/logging.dart';
import 'package:upeg/src/boot/startup_profile.dart';
import 'package:upeg/src/features/controlled_embed/providers.dart';
import 'package:upeg/src/widgets/controlled_embed/session_host.dart';
import 'package:upeg/src/pages/board_page.dart';
import 'package:upeg/src/pages/popup_page.dart';
import 'package:upeg/src/pages/splash_page.dart';
import 'package:upeg/src/platform/app_exit_listener.dart';
import 'package:upeg/src/platform/deep_link_listener.dart';
import 'package:upeg/src/platform/global_hotkey.dart';
import 'package:upeg/src/platform/tray.dart';
import 'package:upeg/src/rust/api/boot.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/launch_intent_provider.dart';
import 'package:upeg/src/state/shared_state_versions.dart';
import 'package:upeg/src/state/theme_mode_provider.dart';
import 'package:upeg/src/state/toolkit_runtime_provider.dart';
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

final appInitProvider = FutureProvider<AppInitReport>(
  (ref) async {
    final run = ref.watch(appInitRunnerProvider);
    final log = ref.watch(appBootLogProvider);
    final installDesktopIntegrations = ref.watch(
      desktopIntegrationsInstallerProvider,
    );
    markStartupPhase('init_app_start');
    log('upeg: appInitProvider — calling initApp()');
    final report = await run();
    markStartupPhase('init_app_done');
    log('upeg: appInitProvider — initApp returned, version=${report.version}');
    // Restore the saved Board/tag selection before the first routed frame, so
    // the page never paints with a null Board. Best-effort with the same
    // 2-second cap as the integrations below: when the Boards load fails with
    // an Exception, Riverpod's default retry (10 attempts, up to ~38 s in
    // total) keeps `boardsProvider.future` pending, and boot must not wait
    // that long. BoardPage and PopupPage restore again on mount (idempotent),
    // which covers a failed or timed-out attempt. Runs before the cold-boot
    // launch intent is seeded below, so a deep link's Board wins.
    markStartupPhase('selection_restore_start');
    try {
      await ref
          .read(currentBoardKeyProvider.notifier)
          .restore()
          .timeout(const Duration(seconds: 2));
    } on Object catch (err) {
      log('upeg: appInitProvider — initial selection restore failed: $err');
    } finally {
      markStartupPhase('selection_restore_done');
    }
    // Catalog metadata is embedded in the shell, so this refresh never blocks
    // boot or makes an installed toolkit depend on the network.
    unawaited(ref.read(toolkitRuntimeProvider.notifier).syncCatalog());

    // Dispatch/deep-link prerequisites remain best-effort and time-bounded.
    // Independent tray/hotkey registration continues outside this gate.
    markStartupPhase('integrations_start');
    await installDesktopIntegrations(ref);
    markStartupPhase('integrations_done');
    log(
      'upeg: appInitProvider — routing prerequisites ready, painting BoardPage',
    );
    markStartupPhase('routing_ready');
    return report;
  },
  retry: (count, error) {
    // A second desktop process cannot acquire the live owner's lock. Retrying
    // this terminal result kept the splash up for the full Riverpod retry
    // window instead of showing the existing-instance guidance immediately.
    if (error is FrbError_AlreadyRunning) return null;
    return ProviderContainer.defaultRetry(count, error);
  },
);

Future<void> _installDesktopIntegrations(Ref ref) => installDesktopIntegrations(
  prepareControlledEmbed: ref.read(controlledEmbedNativeSupportedProvider)
      ? () => ref.read(controlledEmbedBridgeProvider).ready
      : null,
  installTray: () => UpegTray.install(ref),
  installGlobalHotkey: () => UpegGlobalHotkey.install(ref),
  seedLaunchIntent: () async {
    final dto = currentLaunchIntent();
    if (dto != null) {
      ref.read(launchIntentProvider.notifier).set(LaunchIntent.fromDto(dto));
    }
  },
  installDeepLinkListener: () => DeepLinkListener.install(ref),
);

/// Prepare dispatch and ordered deep-link delivery before routing to the Board.
/// Independent tray/hotkey registration continues while the Board is usable.
@visibleForTesting
Future<void> installDesktopIntegrations({
  required Future<void> Function()? prepareControlledEmbed,
  required Future<void> Function() installTray,
  required Future<void> Function() installGlobalHotkey,
  required Future<void> Function() seedLaunchIntent,
  required Future<void> Function() installDeepLinkListener,
  BootLog log = defaultBootLog,
}) async {
  // Tray and summon-hotkey registration are independent of the Board and
  // deep-link routing. Keep their platform-channel waits off the boot gate.
  unawaited(
    runBestEffortIntegration('UpegTray.install', installTray, log: log),
  );
  // The plugin cannot confirm the native key grab, so completion means
  // "requested" (see `global_hotkey.dart`).
  unawaited(
    runBestEffortIntegration(
      'UpegGlobalHotkey.install',
      installGlobalHotkey,
      successLabel: 'requested',
      log: log,
    ),
  );
  if (prepareControlledEmbed != null) {
    await runBestEffortIntegration(
      'ControlledEmbed WebView provider',
      prepareControlledEmbed,
      log: log,
    );
  }
  // Geometry application moved to `WindowModeApplier` (C03) — the
  // observer at `MaterialApp.home` watches `windowModeProvider` and
  // applies geometry on every change (including the initial mount),
  // so we no longer drive `applyWindowMode` from here.
  // Seed the cold-boot launch intent (PRD §5.9 / Batch E D02). When
  // argv carried a `upeg://open?...` URL, `LaunchIntentApplier` on
  // the BoardPage will see this on its first frame and dispatch the
  // tool. `currentLaunchIntent()` returns `null` when argv had no
  // deep link, so this is a no-op otherwise. It runs before the link
  // listener is installed: a link another launch forwarded while this one
  // was booting is newer than argv, and the listener only delivers it
  // after it is installed, so that link always replaces this seed.
  await runBestEffortIntegration(
    'seedLaunchIntent',
    seedLaunchIntent,
    log: log,
  );
  await runBestEffortIntegration(
    'DeepLinkListener.install',
    installDeepLinkListener,
    log: log,
  );
}

/// Run a desktop-integration call with a wall-clock cap (2 seconds by
/// default). A stalled dispatch/deep-link prerequisite must not keep the
/// splash up indefinitely. Background registrations use the same cap for
/// reporting; a timeout does not cancel the underlying native operation.
///
/// Logs exactly one outcome: `<name> <successLabel>` when [fn] finished in
/// time, `<name> timed out …` when the cap hit first (the success label is
/// NOT logged then), or `<name> failed: …` plus the stack when it threw.
/// [successLabel] is 'ok' unless the call cannot confirm its effect, e.g.
/// the global hotkey, whose plugin never reports a failed key grab.
@visibleForTesting
Future<void> runBestEffortIntegration(
  String name,
  Future<void> Function() fn, {
  String successLabel = 'ok',
  Duration timeout = const Duration(seconds: 2),
  BootLog log = defaultBootLog,
}) async {
  try {
    final completedInTime = await fn()
        .then((_) => true)
        .timeout(timeout, onTimeout: () => false);
    log(
      completedInTime
          ? 'upeg: $name $successLabel'
          : 'upeg: $name timed out after ${timeout.inMilliseconds}ms; '
                'continuing without it',
    );
  } on Object catch (err, stack) {
    log('upeg: $name failed: $err');
    log(stack.toString());
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
    // Take the shared-state version baseline before anything else reads
    // shared state (initApp, tweaks, boards, selection), so a change another
    // surface makes while this app boots is not absorbed into the baseline
    // (`shared_state_versions.dart`; `write_tx` in `upeg-sources` bumps the
    // revision so polling cannot miss a board or layout edit). `listen`
    // creates the probe once and keeps it alive without rebuilding here.
    ref.listen(sharedStateSyncProvider, (_, _) {});
    final init = ref.watch(appInitProvider);
    // Only the periodic timer waits for a successful initApp.
    if (init.hasValue) ref.watch(sharedStatePollingProvider);
    final themeMode = ref.watch(themeModeProvider);
    final accent = ref.watch(accentProvider);
    return AppExitListener(
      child: MaterialApp(
        title: 'upeg',
        // Both palettes are built via the same `forAccent` factory so
        // K03 (accent flip) re-paints the live MaterialApp without a
        // restart — the previous hardcoded `darkTheme/lightTheme` pair
        // was the bug. `accentProvider` derives the typed `Accent` from
        // `tweaksProvider`.
        theme: UpegTheme.forAccent(accent, brightness: Brightness.light),
        darkTheme: UpegTheme.forAccent(accent, brightness: Brightness.dark),
        // The first frame paints the saved theme, which `tweaksProvider` loads
        // synchronously; the default is the Rust one (Light, the SSOT).
        // `themeModeProvider` watches `tweaksProvider` so a Tweaks edit
        // re-renders with the new mode without an app restart.
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
                error: (err, _) => Scaffold(
                  body: Center(child: Text(_bootFailureMessage(err))),
                ),
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
      ),
    );
  }
}
