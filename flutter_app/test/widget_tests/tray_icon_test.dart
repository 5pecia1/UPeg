/// Tray icon asset wiring (B01/B02).
///
/// Production `UpegTray.install` previously called
/// `trayManager.setIcon('')`, which made the tray menu show up without
/// any pictogram. Batch S (S3) wires the `assets/tray_icon.png` asset
/// through an injectable [SetTrayIconFn] seam so widget tests can
/// observe the icon path without ever touching the platform channel.
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/platform/tray.dart';

void main() {
  group('Tray icon', () {
    test('tray_icon_path_points_to_a_non_empty_PNG_asset', () {
      // The constant lives next to its sole call site in `tray.dart`.
      // Web/mobile targets short-circuit `install`, so the asset path
      // is only meaningful when `isTraySupported` is true.
      expect(trayIconAssetPath, isNotEmpty);
      expect(trayIconAssetPath, endsWith('.png'));
      expect(trayIconAssetPath, equals('assets/tray_icon.png'));
    });

    test(
      'tray_install_calls_the_injected_setTrayIconFn_seam_with_the_PNG_path',
      () async {
        final calls = <String>[];
        await installTrayIcon(
          setIcon: (path) async {
            calls.add(path);
          },
        );
        expect(calls, [trayIconAssetPath]);
      },
    );
  });
}
