/// Global (system-wide) summon hotkey — launcher immediacy.
///
/// One binding, defined in exactly one place ([kSummonHotkeyKey] +
/// [kSummonHotkeyModifiers]) so a later settings surface only has to
/// replace these constants with a stored value. Pressing the hotkey
/// runs the shared summon toggle (`window_summon.dart`): window shown
/// → hide; hidden/minimized → popup mode + show + focus.
///
/// Implementation choice (no-reinvention): `hotkey_manager`
/// (leanflutter, MIT) over wiring the workspace's `global-hotkey`
/// Rust crate through upeg-frb. The crate must be pumped by the
/// platform's main event loop (winit/tao style), which the
/// FRB-embedded cdylib does not own; the Flutter plugin hooks the
/// engine's own loop instead. Support matrix:
///   * Windows — `RegisterHotKey`, works.
///   * macOS   — Carbon `RegisterEventHotKey`, works.
///   * Linux   — X11 via `keybinder-3.0`, works on X11 sessions;
///     Wayland has no global-hotkey protocol the plugin supports, so
///     registration fails soft there (tray/deep-link summon still
///     works). Building the Linux runner needs `keybinder-3.0` dev
///     headers.
library;

import 'dart:io' show Platform;

import 'package:flutter/foundation.dart' show debugPrint, kIsWeb;
import 'package:flutter/services.dart' show PhysicalKeyboardKey;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:hotkey_manager/hotkey_manager.dart';

import 'package:upeg/src/platform/window_summon.dart';

/// Default summon binding: Ctrl+Alt+Space. Space is mnemonic for
/// "launcher" (Spotlight/PowerToys Run convention); Ctrl+Alt keeps it
/// off the OS-reserved Meta/Super layer.
const PhysicalKeyboardKey kSummonHotkeyKey = PhysicalKeyboardKey.space;

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

/// Build the summon [HotKey] from the canonical constants. System
/// scope: the whole point is summoning upeg while another app has
/// focus.
HotKey buildSummonHotKey() => HotKey(
  identifier: kSummonHotkeyIdentifier,
  key: kSummonHotkeyKey,
  modifiers: kSummonHotkeyModifiers,
  scope: HotKeyScope.system,
);

/// Whether the current build/target supports `hotkey_manager`. Same
/// desktop-trio gate as tray/window; Wayland failures surface at
/// runtime as a soft registration error, not here.
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
  /// stack registrations; a failed registration (e.g. Wayland, or the
  /// combo being taken by another app) logs and degrades to the
  /// tray/deep-link paths.
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
