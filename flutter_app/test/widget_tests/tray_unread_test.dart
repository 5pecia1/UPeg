import 'dart:async';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/platform/tray.dart';
import 'package:upeg/src/state/notification_results_provider.dart';

void main() {
  testWidgets('unread result wins over startup icon restoration', (
    tester,
  ) async {
    const tray = MethodChannel('tray_manager');
    const motion = MethodChannel('upeg/accessibility');
    final messenger = tester.binding.defaultBinaryMessenger;
    final paths = <String>[];
    var failUnread = true;
    late final StreamController<bool> unread;
    unread = StreamController<bool>(onListen: () => unread.add(true));
    debugDefaultTargetPlatformOverride = TargetPlatform.windows;
    messenger.setMockMethodCallHandler(motion, (_) async => false);
    messenger.setMockMethodCallHandler(tray, (call) async {
      if (call.method == 'setIcon') {
        final path =
            (call.arguments as Map<Object?, Object?>)['iconPath']! as String;
        paths.add(path);
        if (path.endsWith('tray_icon_unread.ico') && failUnread) {
          failUnread = false;
          throw PlatformException(code: 'temporary');
        }
      }
      return true;
    });
    final container = ProviderContainer(
      overrides: [
        unreadNotificationResultsProvider.overrideWith((ref) => unread.stream),
      ],
    );
    try {
      await container.read(Provider<Future<void>>(UpegTray.install));
      await tester.pump();
      expect(paths.last, endsWith('windows/frame_0.ico'));
      await tester.pump();
      await tester.pump();
      expect(
        paths.where((path) => path.endsWith('tray_icon_unread.ico')),
        isEmpty,
      );
      for (var frame = 1; frame < 9; frame++) {
        await tester.pump(const Duration(milliseconds: 100));
        expect(paths.last, endsWith('windows/frame_$frame.ico'));
      }
      await tester.pump(const Duration(milliseconds: 100));
      await tester.pump();
      expect(paths.last, endsWith('assets/tray/tray_icon_unread.ico'));
      await tester.pump(const Duration(seconds: 2));
      await tester.pump();
      expect(
        paths.where((path) => path.endsWith('tray_icon_unread.ico')),
        hasLength(2),
      );
      await tester.pump(const Duration(seconds: 2));
      expect(paths.last, endsWith('assets/tray/tray_icon_unread.ico'));
      final idleCount = paths.length;
      await tester.pump(const Duration(seconds: 2));
      expect(paths.length, idleCount);
      unread.add(false);
      await tester.pump();
      await tester.pump();
      expect(paths.last, endsWith('assets/tray/tray_icon.ico'));
      unread.add(true);
      unread.add(false);
      await tester.pump();
      await tester.pump();
      expect(paths.last, endsWith('assets/tray/tray_icon.ico'));
    } finally {
      container.dispose();
      await tester.pump();
      unawaited(unread.close());
      debugDefaultTargetPlatformOverride = null;
      messenger.setMockMethodCallHandler(motion, null);
      messenger.setMockMethodCallHandler(tray, null);
    }
  });
}
