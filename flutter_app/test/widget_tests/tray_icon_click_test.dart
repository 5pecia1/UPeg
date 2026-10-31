/// Tray icon click routing.
///
/// LEFT click summons the current mode; RIGHT click opens the context menu on
/// platforms where `tray_manager.popUpContextMenu` exists
/// (macOS/Windows). Linux AppIndicator emits no mouse events — the
/// attached menu opens natively, so this routing never fires there.
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/platform/tray.dart';
import 'package:upeg/src/platform/app_lifecycle.dart';
import 'package:upeg/src/state/window_mode_provider.dart';

const MethodChannel _trayChannel = MethodChannel('tray_manager');
const MethodChannel _windowChannel = MethodChannel('window_manager');

void main() {
  group('Tray icon click', () {
    test(
      'tray_icon_click_opens_the_context_menu_on_supported_platforms',
      () async {
        var calls = 0;

        await showTrayContextMenuFromIconClick(
          isSupported: () => true,
          popUpContextMenu: () async {
            calls += 1;
          },
        );

        expect(calls, 1);
      },
    );

    test(
      'tray_icon_click_does_not_open_the_context_menu_on_unsupported_platforms',
      () async {
        var calls = 0;

        await showTrayContextMenuFromIconClick(
          isSupported: () => false,
          popUpContextMenu: () async {
            calls += 1;
          },
        );

        expect(calls, 0);
      },
    );

    testWidgets('tray_left_click_preserves_mode_without_hiding_or_quitting', (
      tester,
    ) async {
      tester.platformDispatcher.accessibilityFeaturesTestValue =
          const FakeAccessibilityFeatures(disableAnimations: true);
      addTearDown(
        tester.platformDispatcher.clearAccessibilityFeaturesTestValue,
      );
      debugDefaultTargetPlatformOverride = TargetPlatform.windows;
      try {
        final trayCalls = <String>[];
        final windowCalls = <String>[];
        var visible = true;
        var minimized = false;
        var quitCalls = 0;
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .setMockMethodCallHandler(_trayChannel, (call) async {
              trayCalls.add(call.method);
              return true;
            });
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .setMockMethodCallHandler(_windowChannel, (call) async {
              windowCalls.add(call.method);
              return switch (call.method) {
                'isVisible' => visible,
                'isMinimized' => minimized,
                _ => null,
              };
            });

        final container = ProviderContainer(
          overrides: [
            windowModeProvider.overrideWith(
              () => WindowModeNotifier(initial: WindowMode.full),
            ),
            quitAppProvider.overrideWithValue(() async {
              quitCalls += 1;
            }),
          ],
        );
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
        expect(windowCalls, containsAllInOrder(['show', 'focus']));
        expect(windowCalls, isNot(contains('hide')));
        expect(container.read(windowModeProvider), WindowMode.full);
        expect(quitCalls, 0);

        visible = false;
        windowCalls.clear();
        await TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .handlePlatformMessage(
              _trayChannel.name,
              const StandardMethodCodec().encodeMethodCall(
                const MethodCall('onTrayIconMouseDown'),
              ),
              (_) {},
            );
        await tester.pump();

        expect(windowCalls, containsAllInOrder(['show', 'focus']));
        expect(windowCalls, isNot(contains('hide')));
        expect(container.read(windowModeProvider), WindowMode.full);
        expect(quitCalls, 0);

        container.read(windowModeProvider.notifier).set(WindowMode.popup);
        visible = true;
        minimized = true;
        windowCalls.clear();
        await TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .handlePlatformMessage(
              _trayChannel.name,
              const StandardMethodCodec().encodeMethodCall(
                const MethodCall('onTrayIconMouseDown'),
              ),
              (_) {},
            );
        await tester.pump();

        expect(windowCalls, containsAllInOrder(['restore', 'show', 'focus']));
        expect(windowCalls, isNot(contains('hide')));
        expect(container.read(windowModeProvider), WindowMode.popup);
        expect(quitCalls, 0);
      } finally {
        debugDefaultTargetPlatformOverride = null;
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .setMockMethodCallHandler(_trayChannel, null);
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .setMockMethodCallHandler(_windowChannel, null);
      }
    });

    testWidgets(
      'tray_right_click_opens_the_context_menu_from_the_installed_listener',
      (tester) async {
        tester.platformDispatcher.accessibilityFeaturesTestValue =
            const FakeAccessibilityFeatures(disableAnimations: true);
        addTearDown(
          tester.platformDispatcher.clearAccessibilityFeaturesTestValue,
        );
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
          await TestDefaultBinaryMessengerBinding
              .instance
              .defaultBinaryMessenger
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
      },
    );

    testWidgets('should summon and quit during an enabled startup cue', (
      tester,
    ) async {
      const motionChannel = MethodChannel('upeg/accessibility');
      final messenger = tester.binding.defaultBinaryMessenger;
      final iconPaths = <String>[];
      final windowCalls = <String>[];
      final quitCalls = <String>[];
      debugDefaultTargetPlatformOverride = TargetPlatform.windows;
      messenger.setMockMethodCallHandler(motionChannel, (_) async => false);
      messenger.setMockMethodCallHandler(_trayChannel, (call) async {
        if (call.method == 'setIcon') {
          final args = call.arguments as Map<Object?, Object?>;
          iconPaths.add(args['iconPath']! as String);
        }
        return true;
      });
      messenger.setMockMethodCallHandler(_windowChannel, (call) async {
        windowCalls.add(call.method);
        return switch (call.method) {
          'isVisible' || 'isMinimized' => false,
          _ => null,
        };
      });
      final container = ProviderContainer(
        overrides: [
          quitAppProvider.overrideWithValue(
            () => runAppQuit(
              shutdown: () => quitCalls.add('shutdown'),
              closeWindow: () async => quitCalls.add('close'),
            ),
          ),
        ],
      );
      try {
        final installProvider = Provider<Future<void>>(UpegTray.install);
        await container.read(installProvider);
        await tester.pump();
        expect(iconPaths, hasLength(2));
        expect(iconPaths.last, endsWith('windows/frame_0.ico'));

        await messenger.handlePlatformMessage(
          _trayChannel.name,
          const StandardMethodCodec().encodeMethodCall(
            const MethodCall('onTrayIconMouseDown'),
          ),
          (_) {},
        );
        await tester.pump();
        expect(windowCalls, containsAllInOrder(['show', 'focus']));
        expect(quitCalls, isEmpty);

        dispatchTrayCommand(container, TrayCommand.quit);
        await tester.pump();

        expect(quitCalls, ['shutdown', 'close']);
        expect(iconPaths, hasLength(3));
        expect(iconPaths.last, endsWith('assets/tray/tray_icon.ico'));
        await tester.pump(const Duration(seconds: 1));
        expect(iconPaths, hasLength(3));
      } finally {
        container.dispose();
        await tester.pump();
        debugDefaultTargetPlatformOverride = null;
        messenger
          ..setMockMethodCallHandler(motionChannel, null)
          ..setMockMethodCallHandler(_trayChannel, null)
          ..setMockMethodCallHandler(_windowChannel, null);
      }
    });
  });
}
