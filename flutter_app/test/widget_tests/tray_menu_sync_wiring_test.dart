/// Proves the WS4 Part B fix: `app.dart` mounts [TrayMenuSync] OUTSIDE
/// (above) the `appInitProvider ... .when(data:)` gate, so its
/// `ref.listen(pauseControllableProvider, ...)` subscription is live
/// before `UpegTray.install` does the first (necessarily-stale) read of
/// `pauseControllableProvider`.
///
/// Regression covered: `UpegTray.install` builds the initial tray menu
/// from `ref.read(pauseControllableProvider)` while `hostStateProvider`
/// (a `StreamProvider`) is still `AsyncLoading` — so the Pause item is
/// built `disabled: true` even on the embedded desktop, the one case
/// where Pause is valid. Nothing used to re-enable it once the real
/// `embedded` event landed a microtask later, because `TrayMenuSync`
/// existed but was never mounted anywhere.
///
/// Unlike `tray_menu_sync_test.dart` (which drives `TrayMenuSync` with
/// an injected recording `SetTrayContextMenuFn`), this test lets
/// `TrayMenuSync` use its REAL default seam (`trayManager.setContextMenu`)
/// and mocks the `tray_manager` platform channel directly — mirroring
/// `tray_icon_click_test.dart`'s pattern — so it can also drive the real
/// `UpegTray.install` and observe both `setContextMenu` calls (the
/// stale initial one from `install`, and the corrected one from
/// `TrayMenuSync`) through the same channel, in production order.
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/platform/tray.dart';
import 'package:upeg/src/rust/api/events.dart';
import 'package:upeg/src/state/host_state_provider.dart';

import '../test_helpers/i18n_test_catalog.dart';

const MethodChannel _trayChannel = MethodChannel('tray_manager');

bool _pauseItemDisabled(Map<Object?, Object?> setContextMenuArgs) {
  final menu = setContextMenuArgs['menu']! as Map<Object?, Object?>;
  final items = menu['items']! as List<Object?>;
  final pauseItem = items.cast<Map<Object?, Object?>>().firstWhere(
    (item) => item['key'] == TrayCommand.togglePause.menuItemKey,
  );
  return pauseItem['disabled']! as bool;
}

void main() {
  testWidgets(
    'TrayMenuSync는_UpegTray_install보다_먼저_구독해_embedded_전환에서_Pause를_재활성화한다',
    (tester) async {
      final controller = StreamController<HostStateEvent>();
      addTearDown(controller.close);
      final container = ProviderContainer(
        overrides: [
          ...i18nTestOverrides,
          hostStateStreamProvider.overrideWith((ref) => controller.stream),
        ],
      );
      addTearDown(container.dispose);

      final setContextMenuCalls = <Map<Object?, Object?>>[];
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(_trayChannel, (call) async {
            if (call.method == 'setContextMenu') {
              setContextMenuCalls.add(call.arguments as Map<Object?, Object?>);
            }
            return true;
          });
      addTearDown(() {
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .setMockMethodCallHandler(_trayChannel, null);
      });

      // Mirrors `app.dart`: TrayMenuSync mounted with its default (real
      // trayManager) seam, above/outside any appInit gate, BEFORE
      // `UpegTray.install` runs below.
      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(
            home: TrayMenuSync(child: SizedBox.shrink()),
          ),
        ),
      );
      await tester.pump();

      // Mounting alone must not re-set anything — `TrayMenuSync` only
      // reacts to CHANGES after it subscribes; the initial menu is
      // `UpegTray.install`'s job.
      expect(setContextMenuCalls, isEmpty);

      // `UpegTray.install` runs next, exactly like `_installDesktopIntegrations`
      // does once `appInitProvider`'s async body reaches it — strictly
      // after `TrayMenuSync` above has already mounted and subscribed.
      final installProvider = Provider<Future<void>>(
        (ref) => UpegTray.install(ref),
      );
      await container.read(installProvider);

      expect(
        setContextMenuCalls,
        hasLength(1),
        reason: 'UpegTray.install sets the initial context menu',
      );
      expect(
        _pauseItemDisabled(setContextMenuCalls[0]),
        isTrue,
        reason:
            'install() reads pauseControllableProvider before the '
            'host-state stream has emitted, so the initial menu still '
            'greys out Pause even on what will turn out to be the '
            'embedded desktop',
      );

      // The real event lands (a microtask+ after install ran) — this is
      // the transition the whole fix hinges on: TrayMenuSync must
      // already be subscribed to catch it.
      controller.add(const HostStateEvent.embedded(endpoint: 'unix:/tmp/e'));
      await tester.pump();
      await tester.pump();

      expect(
        setContextMenuCalls,
        hasLength(2),
        reason:
            'TrayMenuSync must re-set the context menu once '
            'pauseControllableProvider flips false -> true',
      );
      expect(
        _pauseItemDisabled(setContextMenuCalls[1]),
        isFalse,
        reason: 'Pause must be enabled on the embedded desktop',
      );
    },
  );
}
