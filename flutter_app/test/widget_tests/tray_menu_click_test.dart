/// Tray menu clicks reaching their handlers through the installed
/// `UpegTray` listener — the glue `onTrayMenuItemClick` adds on top of
/// the handlers that are tested one by one.
///
/// Both platform channels are mocked. The outgoing `tray_manager`
/// `setContextMenu` call is captured because it carries the menu item
/// ids; the native "menu item clicked" event is then injected the way
/// the platform plugin sends it. `window_manager` records the summon's
/// calls. `UpegTray.install` is idempotent per isolate, so everything
/// that needs the installed listener lives in one test.
library;

import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/platform/app_lifecycle.dart';
import 'package:upeg/src/platform/tray.dart';
import 'package:upeg/src/state/window_mode_provider.dart';

const MethodChannel _trayChannel = MethodChannel('tray_manager');
const MethodChannel _windowChannel = MethodChannel('window_manager');

void main() {
  testWidgets(
    'Open_Pegboard_shows_a_hidden_full_window_and_Quit_runs_the_quit_pipeline',
    (tester) async {
      tester.platformDispatcher.accessibilityFeaturesTestValue =
          const FakeAccessibilityFeatures(disableAnimations: true);
      addTearDown(
        tester.platformDispatcher.clearAccessibilityFeaturesTestValue,
      );
      final messenger =
          TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
      final menuItemIds = <String, int>{};
      final windowCalls = <String>[];
      var quitCalls = 0;

      messenger.setMockMethodCallHandler(_trayChannel, (call) async {
        if (call.method == 'setContextMenu') {
          final args = call.arguments as Map<Object?, Object?>;
          final menu = args['menu']! as Map<Object?, Object?>;
          final items = (menu['items']! as List<Object?>)
              .cast<Map<Object?, Object?>>();
          for (final item in items) {
            final key = item['key'] as String?;
            if (key != null) menuItemIds[key] = item['id']! as int;
          }
        }
        return true;
      });
      // A hidden window: the case the old "Open dashboard" could not fix
      // when the mode already was full.
      messenger.setMockMethodCallHandler(_windowChannel, (call) async {
        windowCalls.add(call.method);
        return switch (call.method) {
          'isVisible' => false,
          'isMinimized' => false,
          _ => null,
        };
      });
      addTearDown(() {
        messenger
          ..setMockMethodCallHandler(_trayChannel, null)
          ..setMockMethodCallHandler(_windowChannel, null);
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

      Future<void> click(TrayCommand command) =>
          messenger.handlePlatformMessage(
            _trayChannel.name,
            const StandardMethodCodec().encodeMethodCall(
              MethodCall('onTrayMenuItemClick', {
                'id': menuItemIds[command.menuItemKey],
              }),
            ),
            (_) {},
          );

      await click(TrayCommand.openDashboard);
      await tester.pump();

      expect(windowCalls, containsAllInOrder(['show', 'focus']));
      expect(container.read(windowModeProvider), WindowMode.full);
      expect(quitCalls, 0, reason: 'Open Pegboard must not quit');

      await click(TrayCommand.quit);
      await tester.pump();

      expect(quitCalls, 1);
    },
  );
}
