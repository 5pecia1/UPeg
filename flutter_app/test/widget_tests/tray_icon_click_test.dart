/// Tray icon click routing.
///
/// Launcher immediacy: LEFT click runs the summon toggle (same code
/// path as the global hotkey), RIGHT click opens the context menu on
/// platforms where `tray_manager.popUpContextMenu` exists
/// (macOS/Windows). Linux AppIndicator emits no mouse events — the
/// attached menu opens natively, so this routing never fires there.
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/platform/tray.dart';

const MethodChannel _trayChannel = MethodChannel('tray_manager');
const MethodChannel _windowChannel = MethodChannel('window_manager');

void main() {
  group('Tray icon click', () {
    test('Tray_icon_click은_지원_platform에서_context_menu를_연다', () async {
      var calls = 0;

      await showTrayContextMenuFromIconClick(
        isSupported: () => true,
        popUpContextMenu: () async {
          calls += 1;
        },
      );

      expect(calls, 1);
    });

    test('Tray_icon_click은_미지원_platform에서_context_menu를_열지_않는다', () async {
      var calls = 0;

      await showTrayContextMenuFromIconClick(
        isSupported: () => false,
        popUpContextMenu: () async {
          calls += 1;
        },
      );

      expect(calls, 0);
    });

    testWidgets('Tray_왼쪽_click은_menu_대신_summon_toggle을_실행한다', (tester) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.windows;
      try {
        final trayCalls = <String>[];
        final windowCalls = <String>[];
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .setMockMethodCallHandler(_trayChannel, (call) async {
              trayCalls.add(call.method);
              return true;
            });
        // 보이는 창 시나리오: toggle 판정이 hide로 떨어져야 한다.
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .setMockMethodCallHandler(_windowChannel, (call) async {
              windowCalls.add(call.method);
              return switch (call.method) {
                'isVisible' => true,
                'isMinimized' => false,
                _ => null,
              };
            });

        final container = ProviderContainer();
        addTearDown(container.dispose);
        final installProvider = Provider<Future<void>>(
          (ref) => UpegTray.install(ref),
        );

        await container.read(installProvider);
        expect(trayCalls, containsAllInOrder(['setIcon', 'setContextMenu']));

        trayCalls.clear();
        await TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .handlePlatformMessage(
              _trayChannel.name,
              const StandardMethodCodec().encodeMethodCall(
                const MethodCall('onTrayIconMouseDown'),
              ),
              (_) {},
            );
        await tester.pump();

        expect(trayCalls, isNot(contains('popUpContextMenu')));
        expect(windowCalls, contains('hide'));
      } finally {
        debugDefaultTargetPlatformOverride = null;
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .setMockMethodCallHandler(_trayChannel, null);
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .setMockMethodCallHandler(_windowChannel, null);
      }
    });

    testWidgets('Tray_오른쪽_click은_설치된_listener에서_context_menu를_연다', (
      tester,
    ) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.windows;
      try {
        final trayCalls = <String>[];
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .setMockMethodCallHandler(_trayChannel, (call) async {
              trayCalls.add(call.method);
              return true;
            });

        final container = ProviderContainer();
        addTearDown(container.dispose);
        final installProvider = Provider<Future<void>>(
          (ref) => UpegTray.install(ref),
        );

        await container.read(installProvider);

        trayCalls.clear();
        await TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .handlePlatformMessage(
              _trayChannel.name,
              const StandardMethodCodec().encodeMethodCall(
                const MethodCall('onTrayIconRightMouseDown'),
              ),
              (_) {},
            );
        await tester.pump();

        expect(trayCalls, contains('popUpContextMenu'));
      } finally {
        debugDefaultTargetPlatformOverride = null;
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .setMockMethodCallHandler(_trayChannel, null);
      }
    });
  });
}
