/// Global (system-wide) summon hotkey — launcher immediacy.
///
/// One binding, defined in exactly one place ([summonHotkeyKeyFor] +
/// [kSummonHotkeyModifiers]) so a later settings surface only has to
/// replace these with a stored value. Pressing the hotkey runs the shared
/// summon toggle (`window_summon.dart`): window shown → hide;
/// hidden/minimized → popup mode + show + focus.
///
/// On X11 the hotkey's own passive key grab makes GDK report focus loss
/// (FocusOut NotifyGrab) while the key is held, so focus cannot be read
/// inside the handler; the toggle is therefore not focus-aware: a shown
/// window that other windows cover is hidden like any other, not raised. A
/// focus check on macOS and Windows has not been tried on real machines, so
/// the toggle is the same on every platform. The same grab-induced blur
/// reaches `PopupAutoHideObserver`, which is why that observer hides only
/// after a short grace period.
///
/// Default binding: Ctrl+Alt+U on Linux, Ctrl+Alt+Space on macOS and
/// Windows. Linux cannot use Space: `hotkey_manager` turns the physical key
/// into a GTK keysym by reverse lookup in Flutter's `kGtkToLogicalKey`, and
/// the only entry that maps to `space` is KP_Space (0xFF80). The plugin
/// therefore asks keybinder to grab `<Primary><Alt>KP_Space`, a key a
/// regular keyboard does not produce, and Ctrl+Alt+Space never fires.
/// Letter keys have no entry in that table, so the lookup falls back to the
/// logical key id, which for `u` is 0x75 — the canonical keysym. The
/// combination has not been checked against IDE, input-method or desktop
/// shortcuts.
///
/// Implementation choice (no-reinvention): `hotkey_manager`
/// (leanflutter, MIT) over wiring the workspace's `global-hotkey`
/// Rust crate through upeg-frb. The crate must be pumped by the
/// platform's main event loop (winit/tao style), which the
/// FRB-embedded cdylib does not own; the Flutter plugin hooks the
/// engine's own loop instead. Support matrix:
///   * Windows — `RegisterHotKey`, works.
///   * macOS   — Carbon `RegisterEventHotKey`, works.
///   * Linux   — X11 via `keybinder-3.0`, supported on X11 sessions with
///     the U default above. The plugin discards the result of
///     `keybinder_bind`, so a failed grab (a combination another
///     application holds, or no X11 at all) is never reported:
///     registration looks successful either way. Wayland has no
///     global-hotkey protocol the plugin supports; there, bind a
///     desktop-environment shortcut to the app executable instead —
///     launching it again forwards to the running instance, which
///     summons the window. Building the Linux runner needs
///     `keybinder-3.0` dev headers.
library;

import 'dart:io' show Platform;

import 'package:flutter/foundation.dart'
    show TargetPlatform, debugPrint, defaultTargetPlatform, kIsWeb;
import 'package:flutter/services.dart' show PhysicalKeyboardKey;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:hotkey_manager/hotkey_manager.dart';

import 'package:upeg/src/platform/window_summon.dart';

/// Default summon key for [platform]: U on Linux (Space cannot be grabbed
/// there, see the library comment), Space everywhere else. Space is
/// mnemonic for "launcher" (Spotlight/PowerToys Run convention), and with
/// Ctrl+Alt it stays off the OS-reserved Meta/Super layer.
PhysicalKeyboardKey summonHotkeyKeyFor(TargetPlatform platform) =>
    switch (platform) {
      TargetPlatform.linux => PhysicalKeyboardKey.keyU,
      _ => PhysicalKeyboardKey.space,
    };

/// Modifier set for the summon binding. Order is cosmetic (the plugin
/// treats it as a set).
const List<HotKeyModifier> kSummonHotkeyModifiers = [
  HotKeyModifier.control,
  HotKeyModifier.alt,
];

/// Stable identifier for the summon binding — keeps register /
/// unregister idempotent across hot restarts (the plugin defaults to
/// a random UUID per `HotKey` instance otherwise).
const String kSummonHotkeyIdentifier = 'upeg.summon';

/// Build the summon [HotKey] for [platform] (default: the running
/// platform) from the canonical constants. System scope: the whole point
/// is summoning upeg while another app has focus.
HotKey buildSummonHotKey({TargetPlatform? platform}) => HotKey(
  identifier: kSummonHotkeyIdentifier,
  key: summonHotkeyKeyFor(platform ?? defaultTargetPlatform),
  modifiers: kSummonHotkeyModifiers,
  scope: HotKeyScope.system,
);

/// Whether the current build/target supports `hotkey_manager`. Same
/// desktop-trio gate as tray/window; Wayland is not detected here (the
/// plugin reports no failure there either, see the library comment).
bool get isGlobalHotkeySupported {
  if (kIsWeb) return false;
  try {
    return Platform.isLinux || Platform.isMacOS || Platform.isWindows;
  } on UnsupportedError {
    return false;
  }
}

/// Registration seam. Production wraps `hotKeyManager`; tests pass
/// recording closures so the platform channel never fires.
typedef RegisterHotKeyFn =
    Future<void> Function(HotKey hotKey, {HotKeyHandler? keyDownHandler});
typedef UnregisterAllHotKeysFn = Future<void> Function();

Future<void> _defaultRegister(HotKey hotKey, {HotKeyHandler? keyDownHandler}) =>
    hotKeyManager.register(hotKey, keyDownHandler: keyDownHandler);

Future<void> _defaultUnregisterAll() => hotKeyManager.unregisterAll();

/// Global hotkey installer — mirrors `UpegTray.install` /
/// `DeepLinkListener.install` (idempotent, best-effort, called from
/// `app.dart`'s desktop-integration phase).
class UpegGlobalHotkey {
  UpegGlobalHotkey._();

  static bool _installed = false;

  /// Install the summon hotkey. Idempotent so hot reload doesn't
  /// stack registrations. A registration that throws logs and degrades to
  /// the tray/deep-link paths; a grab that only fails natively (Wayland, or
  /// the combo being taken by another app) is not reported by the plugin,
  /// so a clean return means "requested", not "bound".
  static Future<void> install(
    Ref ref, {
    RegisterHotKeyFn register = _defaultRegister,
    UnregisterAllHotKeysFn unregisterAll = _defaultUnregisterAll,
  }) async {
    if (!isGlobalHotkeySupported) {
      debugPrint('upeg: global hotkey not supported on this platform');
      return;
    }
    if (_installed) return;
    // Hot-restart hygiene per the plugin README: native registrations
    // survive a Dart restart, so clear them before re-registering.
    await unregisterAll();
    // Snapshot the container once at install time; the handler must
    // not re-resolve state through the (potentially disposed) Ref.
    final container = ref.container;
    await register(
      buildSummonHotKey(),
      keyDownHandler: (_) {
        // Fire-and-forget: the hotkey callback is synchronous.
        runSummonToggle(container).ignore();
      },
    );
    _installed = true;
  }
}
