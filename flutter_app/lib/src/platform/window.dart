/// Window geometry + decoration toggle for the popup ↔ full mode
/// change.
///
/// Web and non-supported platforms no-op so the same call sites work
/// on all targets.
library;

import 'dart:io' show Platform;

import 'package:flutter/foundation.dart' show kIsWeb, visibleForTesting;
import 'package:flutter/services.dart' show MissingPluginException;
import 'package:flutter/widgets.dart' show Size;
import 'package:window_manager/window_manager.dart';

import 'package:upeg/src/state/window_mode_provider.dart';

/// PRD §5.9 dimensions: 400×500 frameless popup vs 1280×800 decorated
/// dashboard. 1280×800 comfortably fits six 168px pin columns (1008px)
/// plus chrome padding. Kept as private named constants so both
/// dimensions are auditable in one place.
const Size _popupSize = Size(400, 500);
const Size _fullSize = Size(1280, 800);

/// Expose window size constants for [`window_platform_test.dart`].
@visibleForTesting
Size get popupSizeForTesting => _popupSize;

@visibleForTesting
Size get fullSizeForTesting => _fullSize;

/// `UPEG_DESKTOP_POPUP=1` env var → start in popup mode.
const String _envDesktopPopup = 'UPEG_DESKTOP_POPUP';

@visibleForTesting
abstract class WindowModeDriver {
  Future<void> setSize(Size size);
  Future<void> setHasShadow(bool hasShadow);
  Future<void> setTitleBarStyle(TitleBarStyle titleBarStyle);
  Future<void> show();
  Future<void> focus();
}

class _WindowManagerDriver implements WindowModeDriver {
  const _WindowManagerDriver();

  @override
  Future<void> focus() => windowManager.focus();

  @override
  Future<void> setHasShadow(bool hasShadow) =>
      windowManager.setHasShadow(hasShadow);

  @override
  Future<void> setSize(Size size) => windowManager.setSize(size);

  @override
  Future<void> setTitleBarStyle(TitleBarStyle titleBarStyle) =>
      windowManager.setTitleBarStyle(titleBarStyle);

  @override
  Future<void> show() => windowManager.show();
}

/// Whether the current build/target supports `window_manager`. Tests
/// and the web build short-circuit through this gate so the platform
/// channels never get touched.
bool get isWindowManagerSupported {
  if (kIsWeb) return false;
  try {
    return Platform.isLinux || Platform.isMacOS || Platform.isWindows;
  } on UnsupportedError {
    return false;
  }
}

/// Initialize the `window_manager` plugin after the Flutter binding is ready.
Future<void> ensureWindowManagerInitialized() async {
  if (!isWindowManagerSupported) return;
  await windowManager.ensureInitialized();
}

/// Initial window mode at app start, derived from the
/// `UPEG_DESKTOP_POPUP` env var. Pure function — safe to call from
/// tests.
WindowMode initialWindowMode({Map<String, String>? env}) {
  final source = env ?? _readEnv();
  if (source[_envDesktopPopup] != null &&
      source[_envDesktopPopup]!.isNotEmpty) {
    return WindowMode.popup;
  }
  return kDefaultWindowMode;
}

Map<String, String> _readEnv() {
  if (kIsWeb) return const <String, String>{};
  try {
    return Platform.environment;
  } on UnsupportedError {
    return const <String, String>{};
  }
}

/// Apply geometry + decoration for the given [mode]. No-op on
/// unsupported platforms.
Future<void> applyWindowMode(WindowMode mode) async {
  if (!isWindowManagerSupported) return;
  await applyWindowModeWith(mode, const _WindowManagerDriver());
}

@visibleForTesting
Future<void> applyWindowModeWith(
  WindowMode mode,
  WindowModeDriver driver,
) async {
  final size = switch (mode) {
    WindowMode.popup => _popupSize,
    WindowMode.full => _fullSize,
  };
  final decorated = switch (mode) {
    WindowMode.popup => false,
    WindowMode.full => true,
  };
  await driver.setSize(size);
  await _setHasShadowIfSupported(driver, decorated);
  await driver.setTitleBarStyle(
    decorated ? TitleBarStyle.normal : TitleBarStyle.hidden,
  );
  await driver.show();
  await driver.focus();
}

Future<void> _setHasShadowIfSupported(
  WindowModeDriver driver,
  bool hasShadow,
) async {
  try {
    await driver.setHasShadow(hasShadow);
  } on MissingPluginException {
    // Linux window_manager currently lacks setHasShadow on some plugin
    // builds. Shadow is decorative, so keep geometry/show/focus alive.
  }
}
