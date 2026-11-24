import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/platform/desktop_update.dart';

void main() {
  test(
    'should return only a newer stable release from the official path',
    () async {
      Future<String?> check(String url, [String current = '0.7.11']) =>
          DesktopUpdateService(
            resolveLatest: () async => Uri.parse(url),
          ).check(current);
      expect(
        await check('https://github.com/5pecia1/UPeg/releases/tag/v0.8.0'),
        '0.8.0',
      );
      expect(
        await check('https://github.com/5pecia1/UPeg/releases/tag/v0.7.11'),
        isNull,
      );
      expect(
        await check('https://github.com/5pecia1/UPeg/releases/tag/v0.7.9'),
        isNull,
      );
      expect(
        await check('https://github.com/5pecia1/UPeg/releases/tag/v0.8.0-rc.1'),
        isNull,
      );
      expect(
        await check('https://evil.example/5pecia1/UPeg/releases/tag/v0.8.0'),
        isNull,
      );
      expect(
        await check('https://github.com/5pecia1/UPeg/releases/tag/v0.08.0'),
        isNull,
      );
      expect(
        await check(
          'https://github.com/5pecia1/UPeg/releases/tag/v0.8.0',
          'dev',
        ),
        isNull,
      );
    },
  );

  test('should treat network errors as unavailable', () async {
    final service = DesktopUpdateService(
      resolveLatest: () async => throw Exception('offline'),
    );
    expect(await service.check('1.0.0'), isNull);
  });

  test('should open only the approved release page on Windows', () async {
    Uri? opened;
    final service = DesktopUpdateService(
      platform: TargetPlatform.windows,
      openUrl: (uri) async {
        opened = uri;
        return true;
      },
    );
    expect(await service.perform('1.2.3'), DesktopUpdateAction.openedDownloads);
    expect(
      opened.toString(),
      'https://github.com/5pecia1/UPeg/releases/tag/v1.2.3',
    );
    expect(() => service.perform('../evil'), throwsArgumentError);
  });

  test(
    'should hand approved version to native updater and surface failure',
    () async {
      final service = DesktopUpdateService(
        platform: TargetPlatform.macOS,
        nativeUpdate: (version) async {
          expect(version, '1.2.3');
          return 'installed';
        },
      );
      expect(await service.perform('1.2.3'), DesktopUpdateAction.installed);
      final failed = DesktopUpdateService(
        platform: TargetPlatform.linux,
        nativeUpdate: (_) async => throw PlatformException(code: 'failed'),
      );
      expect(() => failed.perform('1.2.3'), throwsA(isA<PlatformException>()));
    },
  );

  test(
    'should fall back to release page when native integration is absent',
    () async {
      final service = DesktopUpdateService(
        platform: TargetPlatform.linux,
        nativeUpdate: (_) async => throw MissingPluginException(),
        openUrl: (_) async => true,
      );
      expect(
        await service.perform('1.2.3'),
        DesktopUpdateAction.openedDownloads,
      );
    },
  );
}
