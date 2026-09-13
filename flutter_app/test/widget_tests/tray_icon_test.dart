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
    test('Tray_icon_path는_빈_문자열이_아닌_PNG_자산을_가리킨다', () {
      // The constant lives next to its sole call site in `tray.dart`.
      // Web/mobile targets short-circuit `install`, so the asset path
      // is only meaningful when `isTraySupported` is true.
      expect(trayIconAssetPath, isNotEmpty);
      expect(trayIconAssetPath, endsWith('.png'));
      expect(trayIconAssetPath, equals('assets/tray_icon.png'));
    });

    test('Tray_install은_주입된_setTrayIconFn_seam을_PNG_경로로_호출한다', () async {
      final calls = <String>[];
      await installTrayIcon(
        setIcon: (path) async {
          calls.add(path);
        },
      );
      expect(calls, [trayIconAssetPath]);
    });
  });
}
