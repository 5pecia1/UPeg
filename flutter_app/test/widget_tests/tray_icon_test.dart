/// Tray icon wiring.
///
/// `UpegTray.install` hands one [TrayIconSpec] to `trayManager.setIcon`,
/// and which spec depends on the platform: a 22x22 colour PNG on Linux,
/// a template image on macOS and an ICO on Windows. The platform call
/// goes through an injectable [SetTrayIconFn] seam so these tests
/// observe the spec without touching the platform channel; the bundled
/// files are checked through the real asset bundle so an asset missing
/// from `pubspec.yaml` fails here instead of silently dropping the tray
/// at runtime.
library;

import 'dart:typed_data';

import 'package:flutter/foundation.dart'
    show TargetPlatform, debugDefaultTargetPlatformOverride;
import 'package:flutter/services.dart' show rootBundle;
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/platform/tray.dart';

const int _pngSignature = 0x89504E47;

/// Width and height from the IHDR chunk: 8 signature bytes, 4 length
/// bytes and the 4-byte chunk type come first, then two big-endian
/// uint32 values. [ByteData] reads big-endian by default.
({int width, int height}) _pngSize(ByteData png) {
  expect(png.getUint32(0), _pngSignature, reason: 'not a PNG file');
  return (width: png.getUint32(16), height: png.getUint32(20));
}

void main() {
  group('trayIconSpecFor', () {
    test('Linux_uses_the_22px_colour_PNG_at_the_unchanged_asset_path', () {
      expect(trayIconSpecFor(TargetPlatform.linux), (
        assetPath: 'assets/tray_icon.png',
        isTemplate: false,
        iconSize: 18,
      ));
    });

    test('macOS_uses_the_template_image_at_2x_drawn_at_22_points', () {
      expect(trayIconSpecFor(TargetPlatform.macOS), (
        assetPath: 'assets/tray/trayTemplate@2x.png',
        isTemplate: true,
        iconSize: 22,
      ));
    });

    test('Windows_uses_the_ICO_because_the_loader_rejects_PNG', () {
      expect(trayIconSpecFor(TargetPlatform.windows), (
        assetPath: 'assets/tray/tray_icon.ico',
        isTemplate: false,
        iconSize: 18,
      ));
    });

    test('targets_without_a_tray_fall_back_to_the_Linux_spec', () {
      final linux = trayIconSpecFor(TargetPlatform.linux);
      expect(trayIconSpecFor(TargetPlatform.android), linux);
      expect(trayIconSpecFor(TargetPlatform.fuchsia), linux);
      expect(trayIconSpecFor(TargetPlatform.iOS), linux);
    });
  });

  group('installTrayIcon', () {
    test('hands_the_spec_of_the_given_platform_to_the_setIcon_seam', () async {
      const expectedAssets = {
        TargetPlatform.linux: 'assets/tray_icon.png',
        TargetPlatform.macOS: 'assets/tray/trayTemplate@2x.png',
        TargetPlatform.windows: 'assets/tray/tray_icon.ico',
      };
      for (final MapEntry(key: platform, value: assetPath)
          in expectedAssets.entries) {
        final calls = <TrayIconSpec>[];
        await installTrayIcon(
          setIcon: (spec) async {
            calls.add(spec);
          },
          platform: platform,
        );
        expect(calls, [trayIconSpecFor(platform)], reason: '$platform');
        expect(calls.single.assetPath, assetPath, reason: '$platform');
      }
    });

    test('defaults_to_the_running_target_platform', () async {
      debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
      try {
        final calls = <TrayIconSpec>[];
        await installTrayIcon(
          setIcon: (spec) async {
            calls.add(spec);
          },
        );
        expect(calls, [trayIconSpecFor(TargetPlatform.macOS)]);
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });
  });

  group('bundled tray assets', () {
    testWidgets('every_platform_asset_is_declared_in_pubspec_and_loads', (
      tester,
    ) async {
      // `rootBundle` only serves what `flutter.assets` lists, so a
      // missing declaration surfaces as a load failure here.
      final assetPaths = {
        for (final platform in TargetPlatform.values)
          trayIconSpecFor(platform).assetPath,
      };
      for (final assetPath in assetPaths) {
        final data = await tester.runAsync(() => rootBundle.load(assetPath));
        expect(data, isNotNull, reason: assetPath);
        expect(data!.lengthInBytes, greaterThan(0), reason: assetPath);
      }
    });

    testWidgets('the_Linux_PNG_is_22x22_pixels', (tester) async {
      final spec = trayIconSpecFor(TargetPlatform.linux);
      final png = await tester.runAsync(() => rootBundle.load(spec.assetPath));

      expect(_pngSize(png!), (width: 22, height: 22));
    });

    testWidgets('the_macOS_template_is_2x_the_icon_size_in_points', (
      tester,
    ) async {
      final spec = trayIconSpecFor(TargetPlatform.macOS);
      final png = await tester.runAsync(() => rootBundle.load(spec.assetPath));

      expect(_pngSize(png!), (
        width: spec.iconSize * 2,
        height: spec.iconSize * 2,
      ));
    });

    testWidgets('the_Windows_asset_is_a_real_ICO_not_a_renamed_PNG', (
      tester,
    ) async {
      final spec = trayIconSpecFor(TargetPlatform.windows);
      final ico = await tester.runAsync(() => rootBundle.load(spec.assetPath));

      // ICONDIR: reserved = 0, type = 1 (icon), then the image count,
      // all little-endian uint16.
      expect(ico!.getUint16(0, Endian.little), 0, reason: 'reserved');
      expect(ico.getUint16(2, Endian.little), 1, reason: 'type: icon');
      expect(ico.getUint16(4, Endian.little), greaterThan(0), reason: 'count');
    });
  });
}
