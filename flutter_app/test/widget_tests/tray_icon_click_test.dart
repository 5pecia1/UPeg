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

    testWidgets('tray_left_click_runs_summon_toggle_instead_of_the_menu', (
      tester,
    ) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.windows;
      try {
        final trayCalls = <String>[];
        final windowCalls = <String>[];
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .setMockMethodCallHandler(_trayChannel, (call) async {
              trayCalls.add(call.method);
              return true;
            });
        // Visible-window scenario: the toggle verdict must come out
        // hide.
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

    testWidgets(
      'tray_right_click_opens_the_context_menu_from_the_installed_listener',
      (tester) async {
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
  });
}
