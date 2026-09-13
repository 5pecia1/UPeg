/// System tray wiring for the Flutter desktop surface.
///
/// Quit / Open Dashboard / Toggle Popup stay local to the Dart window
/// layer. Tray icon LEFT click runs the launcher summon toggle
/// (`window_summon.dart` — same behavior as the global hotkey); RIGHT
/// click explicitly opens the native context menu on platforms where
/// `tray_manager.popUpContextMenu` exists. On Linux the AppIndicator
/// backend emits no per-button mouse events at all — the context menu
/// attached via `setContextMenu` opens natively on click, so the
/// left/right split only takes effect on macOS/Windows. TogglePin and
/// Pause/Resume route through Riverpod providers backed by the FRB
/// APIs.
///
/// Tray is not available on web and not always available on Linux
/// (depends on `libayatana-appindicator3` at runtime). `install`
/// guards both cases so the rest of the app keeps working.
library;

import 'dart:io' show Platform;

import 'package:flutter/foundation.dart'
    show TargetPlatform, debugPrint, defaultTargetPlatform, kIsWeb;
import 'package:flutter/widgets.dart' show BuildContext, Widget;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:tray_manager/tray_manager.dart';

import 'package:upeg/src/platform/app_lifecycle.dart';
import 'package:upeg/src/platform/window_summon.dart';
import 'package:upeg/src/state/pause_control_provider.dart';
import 'package:upeg/src/state/pause_provider.dart';
import 'package:upeg/src/state/pinned_provider.dart';
import 'package:upeg/src/state/window_mode_provider.dart';

/// Typed tray menu command. Each variant carries the `tray_manager`
/// menu key as its only field so the production switch is free of
/// `String` discriminators and the mapping is exhaustive — adding a
/// new menu item is a single enum entry plus a single `case`.
enum TrayCommand {
  quit('quit'),
  openDashboard('open_dashboard'),
  togglePopup('toggle_popup'),
  togglePin('toggle_pin'),
  togglePause('toggle_pause');

  const TrayCommand(this.menuItemKey);

  /// The `MenuItem.key` string passed to `tray_manager` when the menu
  /// is built and reported back through `onTrayMenuItemClick`.
  final String menuItemKey;

  /// Lookup helper: returns `null` for an unknown menu key so callers
  /// can short-circuit without a default-arm fallback.
  static TrayCommand? fromKey(String key) {
    for (final cmd in TrayCommand.values) {
      if (cmd.menuItemKey == key) return cmd;
    }
    return null;
  }
}

/// Routes a [TrayCommand] to its side-effect provider. Lives at top
/// level so widget tests can drive it with a vanilla `ProviderContainer`
/// instead of building the full `UpegTray` (which would require platform
/// channels). The production switch in `onTrayMenuItemClick` is a thin
/// adapter that calls this with the live `Ref`.
///
/// `OpenDashboard` / `TogglePopup` / `Quit` are still dispatched at the
/// call site because they need extra Dart-side coordination (window
/// mode notifier, the [runTrayQuit] flow). `TogglePin` is dispatched
/// here so the focus-loss observer reads a fresh `pinnedProvider`
/// value without going through the tray controller singleton.
void dispatchTrayCommand(ProviderContainer container, TrayCommand cmd) {
  switch (cmd) {
    case TrayCommand.togglePause:
      // Pause is a process-local `AtomicBool` (PRD §5.9) — it cannot
      // reach a host living in a separate daemon process, so the
      // mutator only fires when this process is the in-process
      // (Embedded) host. `pauseControllableProvider` is the single
      // source of truth the menu-item `disabled` flag also reads.
      if (container.read(pauseControllableProvider)) {
        container.read(togglePausedProvider)();
      }
    case TrayCommand.togglePin:
      // Writes the pinned flag; `PopupAutoHideObserver` reads it on
      // every blur event to gate auto-hide.
      container.read(pinnedProvider.notifier).toggle();
    case TrayCommand.quit:
    case TrayCommand.openDashboard:
    case TrayCommand.togglePopup:
      // Handled at the UpegTray call site — they need ref to the
      // window mode notifier and/or the runTrayQuit flow.
      break;
  }
}

/// Tray-facing quit alias. The implementation lives in
/// `app_lifecycle.dart` so GUI shortcuts do not depend on tray wiring.
Future<void> runTrayQuit({
  ShutdownFn shutdown = defaultAppShutdown,
  CloseWindowFn closeWindow = defaultCloseWindow,
}) => runAppQuit(shutdown: shutdown, closeWindow: closeWindow);

/// Filesystem-style asset path for the tray icon PNG. Lives at the
/// top level so the unit test can assert the production-side value
/// without booting the full tray pipeline.
const String trayIconAssetPath = 'assets/tray_icon.png';

/// Injection seam for the platform `setIcon` call. Production wraps
/// `trayManager.setIcon`; tests pass a recording closure so the
/// platform channel never fires.
typedef SetTrayIconFn = Future<void> Function(String iconPath);

Future<void> _defaultSetTrayIcon(String iconPath) =>
    trayManager.setIcon(iconPath);

/// Injection seam for `trayManager.popUpContextMenu`. The Flutter
/// plugin does not automatically show the menu from icon mouse events
/// on macOS/Windows (the `tray-icon` crate the retired Dioxus surface
/// used did), so the listener calls this explicitly.
typedef PopUpTrayContextMenuFn = Future<void> Function();

Future<void> _defaultPopUpTrayContextMenu() => trayManager.popUpContextMenu();

/// Support gate for programmatic context-menu popup. Linux already
/// attaches the context menu to AppIndicator via `setContextMenu`, but
/// `popUpContextMenu` itself is not implemented there.
typedef TrayContextMenuSupportFn = bool Function();

bool _defaultTrayContextMenuSupported() => canPopUpTrayContextMenu;

bool get canPopUpTrayContextMenu {
  if (kIsWeb) return false;
  return switch (defaultTargetPlatform) {
    TargetPlatform.macOS || TargetPlatform.windows => true,
    TargetPlatform.android ||
    TargetPlatform.fuchsia ||
    TargetPlatform.iOS ||
    TargetPlatform.linux => false,
  };
}

/// Open the tray context menu from a tray-icon click when the platform
/// requires an explicit popup call. Test seams keep the behavior
/// platform-channel-free in unit tests.
Future<void> showTrayContextMenuFromIconClick({
  TrayContextMenuSupportFn isSupported = _defaultTrayContextMenuSupported,
  PopUpTrayContextMenuFn popUpContextMenu = _defaultPopUpTrayContextMenu,
}) async {
  if (!isSupported()) return;
  await popUpContextMenu();
}

/// Install the tray icon image. Split out from [UpegTray.install] so
/// the icon-path contract is unit-testable in isolation — the test
/// passes a recording [SetTrayIconFn] and asserts the path equals
/// [trayIconAssetPath].
Future<void> installTrayIcon({
  SetTrayIconFn setIcon = _defaultSetTrayIcon,
}) async {
  await setIcon(trayIconAssetPath);
}

/// Build the tray menu shown by [UpegTray.install].
///
/// Kept as a top-level seam so unit tests can assert command keys,
/// visible labels, and group separators without touching the
/// `tray_manager` platform channel.
///
/// [pauseControllable] greys the Pause item (`disabled: true`) when
/// this process is not the in-process (Embedded) host — pause is a
/// process-local `AtomicBool` (PRD §5.9) and cannot reach a foreign
/// daemon. Defaults to `true` so existing call sites keep prior
/// behavior until they thread the live `pauseControllableProvider`
/// value through.
Menu buildTrayMenu({bool pauseControllable = true}) => Menu(
  items: [
    MenuItem(
      key: TrayCommand.openDashboard.menuItemKey,
      label: 'Open dashboard',
    ),
    MenuItem(
      key: TrayCommand.togglePopup.menuItemKey,
      label: 'Toggle popup / full',
    ),
    MenuItem(
      key: TrayCommand.togglePin.menuItemKey,
      label: 'Toggle pin (keep popup open)',
    ),
    MenuItem.separator(),
    MenuItem(
      key: TrayCommand.togglePause.menuItemKey,
      label: 'Pause / Resume host',
      disabled: !pauseControllable,
    ),
    MenuItem.separator(),
    MenuItem(key: TrayCommand.quit.menuItemKey, label: 'Quit'),
  ],
);

/// Injection seam for `trayManager.setContextMenu`. Production wraps
/// the platform call directly; tests pass a recording closure so the
/// platform channel never fires.
typedef SetTrayContextMenuFn = Future<void> Function(Menu menu);

Future<void> _defaultSetTrayContextMenu(Menu menu) =>
    trayManager.setContextMenu(menu);

/// Re-sets the tray context menu whenever [pauseControllableProvider]
/// flips, so the Pause item's `disabled` flag never lags a stale
/// host-state after `UpegTray.install`'s initial `buildTrayMenu` call.
/// Guarded by [isTraySupported] so it is a no-op on platforms without
/// a tray (web, mobile, or an unsupported Linux desktop session) —
/// mirrors `WindowModeApplier`'s injected-seam + platform-gate shape,
/// but (unlike that observer) does NOT re-apply at mount: the initial
/// menu is already set by `UpegTray.install` with the current
/// controllability, so this only reacts to subsequent changes.
///
/// Mounted in `app.dart`'s `UpegApp.build()`, wrapping `WindowModeApplier`
/// OUTSIDE (above) the `appInitProvider ... .when(data:)` gate. That
/// placement is load-bearing: it makes this widget's `build` — and
/// therefore its `ref.listen` subscription — run on the very first
/// synchronous frame, before `appInitProvider`'s async body reaches
/// `UpegTray.install`. `UpegTray.install` does the first-ever
/// `ref.read(pauseControllableProvider)`, which is what starts the
/// `hostStateProvider` stream subscription; mounting this widget any
/// later (e.g. under `data:`) would race that subscription and could
/// miss the single PoC event entirely, reproducing the "Pause never
/// re-enables on the embedded desktop" regression.
///
/// Known limitation (WS4): the host-state stream is currently a PoC
/// single-event replay (see `host_state_provider.dart`), so in
/// practice this observer sees at most one transition (e.g.
/// loading → embedded) until Phase 7 wires a real broadcaster. That is
/// sufficient to re-enable Pause on the embedded desktop; it just
/// won't reflect further host-state changes mid-session. The dispatch
/// gate in [dispatchTrayCommand] is what actually prevents the split-brain
/// toggle regardless of whether the menu visually re-greys.
class TrayMenuSync extends ConsumerWidget {
  const TrayMenuSync({
    super.key,
    required this.child,
    this.setContextMenu = _defaultSetTrayContextMenu,
  });

  /// Subtree this observer wraps. Kept as a vanilla [Widget] so it can
  /// mount above any route, alongside `WindowModeApplier`.
  final Widget child;

  /// Injected platform call — defaults to
  /// `trayManager.setContextMenu`. Tests pass a recording closure.
  final SetTrayContextMenuFn setContextMenu;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    ref.listen<bool>(pauseControllableProvider, (previous, next) {
      if (previous == next) return;
      if (!isTraySupported) return;
      // ignore: discarded_futures — re-sync is fire-and-forget.
      _resyncBestEffort(next);
    });
    return child;
  }

  Future<void> _resyncBestEffort(bool pauseControllable) async {
    try {
      await setContextMenu(buildTrayMenu(pauseControllable: pauseControllable));
    } on Object catch (err, stack) {
      debugPrint('upeg: setContextMenu failed: $err');
      debugPrint(stack.toString());
    }
  }
}

/// Whether the current build/target supports `tray_manager`. Web and
/// mobile (Android/iOS) targets short-circuit so platform channels
/// never get called.
bool get isTraySupported {
  if (kIsWeb) return false;
  try {
    return Platform.isLinux || Platform.isMacOS || Platform.isWindows;
  } on UnsupportedError {
    return false;
  }
}

/// Tray controller — keeps the [Ref] for callback dispatch and bridges
/// `TrayListener` events into Riverpod state updates.
class UpegTray with TrayListener {
  UpegTray._(this._ref);

  final Ref _ref;
  static UpegTray? _installed;

  /// Install the tray. Idempotent: subsequent calls are no-ops so
  /// hot-reload doesn't stack listeners.
  static Future<void> install(Ref ref) async {
    if (!isTraySupported) {
      debugPrint('upeg: tray not supported on this platform; skipping install');
      return;
    }
    if (_installed != null) return;
    final tray = UpegTray._(ref);
    try {
      // The tray icon is a small 16×16 PNG asset; the wire-up lives in
      // [installTrayIcon] (S3) so the icon-path contract stays
      // unit-testable in isolation. `assets/tray_icon.png` ships via
      // pubspec's `flutter.assets` list.
      await installTrayIcon();
      await trayManager.setContextMenu(
        buildTrayMenu(pauseControllable: ref.read(pauseControllableProvider)),
      );
      trayManager.addListener(tray);
      _installed = tray;
    } on Exception catch (err) {
      debugPrint('upeg: tray install failed: $err');
    }
  }

  /// Left click = launcher summon toggle (shown → hide, hidden →
  /// popup + summon). Shares `runSummonToggle` with the global hotkey
  /// so both entry points stay behaviorally identical. Only
  /// macOS/Windows deliver this event — Linux AppIndicator opens the
  /// attached menu natively and never reaches here.
  @override
  void onTrayIconMouseDown() {
    unawaited(runSummonToggle(_ref.container));
  }

  @override
  void onTrayIconRightMouseDown() {
    _showContextMenuFromIconClick();
  }

  @override
  void onTrayMenuItemClick(MenuItem menuItem) {
    final key = menuItem.key;
    if (key == null) {
      debugPrint('upeg: tray menu item without key');
      return;
    }
    final cmd = TrayCommand.fromKey(key);
    if (cmd == null) {
      debugPrint('upeg: tray unknown menu key: $key');
      return;
    }
    switch (cmd) {
      case TrayCommand.quit:
        unawaited(runTrayQuit());
      case TrayCommand.openDashboard:
        _setMode(WindowMode.full);
      case TrayCommand.togglePopup:
        _toggleMode();
      case TrayCommand.togglePin:
      case TrayCommand.togglePause:
        dispatchTrayCommand(_ref.container, cmd);
    }
  }

  void _setMode(WindowMode mode) {
    // Geometry application is owned by `WindowModeApplier` (C03) —
    // the observer listens on `windowModeProvider` and calls
    // `applyWindowMode` on every change. The tray dispatch only flips
    // the typed state; the platform call follows automatically.
    _ref.read(windowModeProvider.notifier).set(mode);
  }

  void _toggleMode() {
    _ref.read(windowModeProvider.notifier).toggle();
  }

  void _showContextMenuFromIconClick() {
    unawaited(showTrayContextMenuFromIconClick());
  }
}

void unawaited(Future<void> future) {
  // Local mirror of `dart:async`'s `unawaited` — already provided by
  // `package:flutter_riverpod` transitively but kept explicit so the
  // intent is documented at every call site.
  future.ignore();
}
