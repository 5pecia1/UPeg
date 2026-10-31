/// Bundled tray images and the platform icon driver.
library;

import 'package:flutter/foundation.dart'
    show TargetPlatform, defaultTargetPlatform;
import 'package:tray_manager/tray_manager.dart';

/// What `tray_manager.setIcon` needs for one platform: the bundled
/// asset, whether macOS should treat it as a template image (recoloured
/// by the menu bar), and the macOS icon size in points. [iconSize] is
/// ignored outside macOS.
typedef TrayIconSpec = ({String assetPath, bool isTemplate, int iconSize});

/// Linux (and every target without a tray, so the mapping stays total):
/// 22×22 hinted colour PNG. AppIndicator never recolours the image and
/// the status-notifier host picks the display size; KDE and XFCE trays
/// typically use 22 px, which this version fits 1:1.
const TrayIconSpec _linuxTrayIcon = (
  assetPath: 'assets/tray_icon.png',
  isTemplate: false,
  iconSize: 18,
);

/// macOS: black + alpha template image at 2x (44 px) drawn at 22 pt, so
/// the menu bar recolours it for light and dark appearances.
const TrayIconSpec _macosTrayIcon = (
  assetPath: 'assets/tray/trayTemplate@2x.png',
  isTemplate: true,
  iconSize: 22,
);

/// Windows: `tray_manager` loads the file with `LoadImage(IMAGE_ICON)`,
/// which needs an ICO, not a PNG.
const TrayIconSpec _windowsTrayIcon = (
  assetPath: 'assets/tray/tray_icon.ico',
  isTemplate: false,
  iconSize: 18,
);

/// The tray icon for [platform]. Lives at the top level so the unit
/// test can assert the production-side values without booting the full
/// tray pipeline.
TrayIconSpec trayIconSpecFor(TargetPlatform platform) => switch (platform) {
  TargetPlatform.macOS => _macosTrayIcon,
  TargetPlatform.windows => _windowsTrayIcon,
  TargetPlatform.linux ||
  TargetPlatform.android ||
  TargetPlatform.fuchsia ||
  TargetPlatform.iOS => _linuxTrayIcon,
};

/// Injection seam for the platform `setIcon` call. Production wraps
/// `trayManager.setIcon`; tests pass a recording closure so the
/// platform channel never fires.
typedef SetTrayIconFn = Future<void> Function(TrayIconSpec spec);

Future<void> setPlatformTrayIcon(TrayIconSpec spec) => trayManager.setIcon(
  spec.assetPath,
  isTemplate: spec.isTemplate,
  iconSize: spec.iconSize,
);

/// Install the tray icon image. Split out from the tray installer so
/// the icon contract is unit-testable in isolation — the test passes a
/// recording [SetTrayIconFn] and asserts the spec equals
/// [trayIconSpecFor] for the given [platform] (default: the running
/// target).
Future<void> installTrayIcon({
  SetTrayIconFn setIcon = setPlatformTrayIcon,
  TargetPlatform? platform,
}) async {
  await setIcon(trayIconSpecFor(platform ?? defaultTargetPlatform));
}
