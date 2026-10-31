/// System tray wiring for the Flutter desktop surface.
///
/// Open Pegboard and Toggle popup / full stay local to the Dart window
/// layer. Tray icon LEFT click summons the current window mode
/// (`window_summon.dart`); RIGHT click explicitly opens the native context
/// menu where `tray_manager.popUpContextMenu` exists. On Linux the AppIndicator
/// backend emits no per-button mouse events at all — the context menu
/// attached via `setContextMenu` opens natively on click, so the
/// left/right split only takes effect on macOS/Windows. Keep popup
/// open, Pause/Resume and Quit route through Riverpod providers
/// (`pinnedProvider`, `togglePausedProvider`, `quitAppProvider`).
///
/// The icon is picked per platform by [trayIconSpecFor]: a 22×22 colour
/// PNG on Linux, a template image on macOS and an ICO on Windows.
///
/// Tray is not available on web and not always available on Linux
/// (depends on `libayatana-appindicator3` at runtime). `install`
/// guards both cases so the rest of the app keeps working.
library;

import 'dart:io' show Platform;
import 'dart:ui' show AppExitResponse;

import 'package:flutter/foundation.dart'
    show TargetPlatform, debugPrint, defaultTargetPlatform, kIsWeb;
import 'package:flutter/widgets.dart'
    show BuildContext, Widget, WidgetsBinding, WidgetsBindingObserver;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:tray_manager/tray_manager.dart';

import 'package:upeg/src/platform/app_lifecycle.dart';
import 'package:upeg/src/platform/tray_animation.dart';
import 'package:upeg/src/platform/tray_icon.dart';
import 'package:upeg/src/platform/window_summon.dart';
import 'package:upeg/src/state/pause_control_provider.dart';
import 'package:upeg/src/state/pause_provider.dart';
import 'package:upeg/src/state/pinned_provider.dart';
import 'package:upeg/src/state/window_mode_provider.dart';

export 'package:upeg/src/platform/tray_icon.dart';

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
/// `OpenDashboard` / `TogglePopup` are still dispatched at the call
/// site because they need extra Dart-side coordination (window mode
/// notifier, the summon in [openPegboardFromTray]). `Quit` goes through
/// [quitAppProvider] so the tray shares one quit pipeline with the
/// keyboard shortcut and the window close button. `TogglePin` is
/// dispatched here so the focus-loss observer reads a fresh
/// `pinnedProvider` value without going through the tray controller
/// singleton.
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
      // Same pipeline as the keyboard shortcut and the window close
      // button: whatever `quitAppProvider` does before exit also covers
      // the tray.
      unawaited(container.read(quitAppProvider)());
    case TrayCommand.openDashboard:
    case TrayCommand.togglePopup:
      // Handled at the UpegTray call site — they need the window mode
      // notifier and/or the summon seam.
      break;
  }
}

/// Tray "Open Pegboard": force the full window mode, then bring the
/// window to the front. The mode write alone is not enough —
/// `WindowModeApplier` dedupes equal modes, so a hidden or minimized
/// window that is already `full` would stay hidden. [summon] is the
/// unconditional show + focus (+ restore) primitive; tests inject a
/// recording closure so `window_manager` is never touched.
Future<void> openPegboardFromTray(
  ProviderContainer container, {
  Future<void> Function() summon = summonWindow,
}) async {
  container.read(windowModeProvider.notifier).set(WindowMode.full);
  await summon();
}

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
///
/// [keepPopupOpen] is the check mark of the "Keep popup open" item —
/// the live `pinnedProvider` value. Defaults to `false`, the provider's
/// initial value.
Menu buildTrayMenu({
  bool pauseControllable = true,
  bool keepPopupOpen = false,
}) => Menu(
  items: [
    MenuItem(
      key: TrayCommand.openDashboard.menuItemKey,
      label: 'Open Pegboard',
    ),
    MenuItem(
      key: TrayCommand.togglePopup.menuItemKey,
      label: 'Toggle popup / full',
    ),
    MenuItem.checkbox(
      key: TrayCommand.togglePin.menuItemKey,
      label: 'Keep popup open',
      checked: keepPopupOpen,
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
/// or [pinnedProvider] changes, so the Pause item's `disabled` flag and
/// the "Keep popup open" check mark never lag a stale host-state or pin
/// state after `UpegTray.install`'s initial `buildTrayMenu` call. Each
/// re-set rebuilds the whole menu from the latest value of both
/// providers. Guarded by [isTraySupported] so it is a no-op on
/// platforms without a tray (web, mobile, or an unsupported Linux
/// desktop session) — mirrors `WindowModeApplier`'s injected-seam +
/// platform-gate shape, but (unlike that observer) does NOT re-apply at
/// mount: the initial menu is already set by `UpegTray.install` with the
/// current values, so this only reacts to subsequent changes.
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
    void resync(bool? previous, bool next) {
      if (previous == next) return;
      if (!isTraySupported) return;
      // ignore: discarded_futures — re-sync is fire-and-forget.
      _resyncBestEffort(
        pauseControllable: ref.read(pauseControllableProvider),
        keepPopupOpen: ref.read(pinnedProvider),
      );
    }

    ref
      ..listen<bool>(pauseControllableProvider, resync)
      ..listen<bool>(pinnedProvider, resync);
    return child;
  }

  Future<void> _resyncBestEffort({
    required bool pauseControllable,
    required bool keepPopupOpen,
  }) async {
    try {
      await setContextMenu(
        buildTrayMenu(
          pauseControllable: pauseControllable,
          keepPopupOpen: keepPopupOpen,
        ),
      );
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
class UpegTray with TrayListener, WidgetsBindingObserver {
  UpegTray._(this._ref);

  final Ref _ref;
  static UpegTray? _installed;
  static bool _installing = false;
  bool _disposed = false;

  /// Install the tray. Idempotent: subsequent calls are no-ops so
  /// hot-reload doesn't stack listeners.
  static Future<void> install(Ref ref) async {
    if (!isTraySupported) {
      debugPrint('upeg: tray not supported on this platform; skipping install');
      return;
    }
    if (_installed != null || _installing) return;
    _installing = true;
    final tray = UpegTray._(ref);
    ref.onDispose(tray._dispose);
    try {
      // The icon asset is picked per platform ([trayIconSpecFor]); the
      // wire-up lives in [installTrayIcon] so the icon contract stays
      // unit-testable in isolation. The assets ship via pubspec's
      // `flutter.assets` list.
      await installTrayIcon();
      if (tray._disposed) return;
      await trayManager.setContextMenu(
        buildTrayMenu(
          pauseControllable: ref.read(pauseControllableProvider),
          keepPopupOpen: ref.read(pinnedProvider),
        ),
      );
      if (tray._disposed) return;
      trayManager.addListener(tray);
      WidgetsBinding.instance.addObserver(tray);
      _installed = tray;
      unawaited(playTrayStartupAnimation());
    } on Exception catch (err) {
      debugPrint('upeg: tray install failed: $err');
    } finally {
      _installing = false;
    }
  }

  void _dispose() {
    _disposed = true;
    trayManager.removeListener(this);
    WidgetsBinding.instance.removeObserver(this);
    if (identical(_installed, this)) _installed = null;
    unawaited(stopTrayAnimation());
  }

  @override
  void didChangeAccessibilityFeatures() {
    if (WidgetsBinding
        .instance
        .platformDispatcher
        .accessibilityFeatures
        .disableAnimations) {
      unawaited(stopTrayAnimation());
    }
  }

  @override
  Future<AppExitResponse> didRequestAppExit() async {
    await stopTrayAnimation();
    return AppExitResponse.exit;
  }

  /// Left click always restores, shows, and focuses the current mode.
  /// The global hotkey retains its separate show/hide toggle. Only
  /// macOS/Windows deliver this event — Linux AppIndicator opens the
  /// attached menu natively and never reaches here.
  @override
  void onTrayIconMouseDown() {
    unawaited(summonWindow());
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
      case TrayCommand.openDashboard:
        unawaited(openPegboardFromTray(_ref.container));
      case TrayCommand.togglePopup:
        _toggleMode();
      case TrayCommand.quit:
      case TrayCommand.togglePin:
      case TrayCommand.togglePause:
        dispatchTrayCommand(_ref.container, cmd);
    }
  }

  void _toggleMode() {
    // Geometry application is owned by `WindowModeApplier` (C03) —
    // the observer listens on `windowModeProvider` and calls
    // `applyWindowMode` on every change. The tray dispatch only flips
    // the typed state; the platform call follows automatically.
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
