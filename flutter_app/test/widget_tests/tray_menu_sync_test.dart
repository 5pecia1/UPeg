/// Tests for [TrayMenuSync] (WS4 Task B2).
///
/// Mirrors `window_mode_applier_test.dart`'s shape: an injected seam
/// (`SetTrayContextMenuFn` here, `ApplyWindowMode` there) records calls
/// instead of touching the real platform channel, and the underlying
/// state source (`hostStateStreamProvider` here, a directly-driven
/// notifier there) is overridden so the FRB call never fires. The
/// load-bearing behavior: when `pauseControllableProvider` flips
/// (because a new `HostStateEvent` arrived), the observer re-sets the
/// tray context menu with the Pause item's `disabled` flag following
/// suit — so the tray menu never lags a stale controllability after a
/// host-state change.
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:tray_manager/tray_manager.dart';

import 'package:upeg/src/platform/tray.dart';
import 'package:upeg/src/rust/api/events.dart';
import 'package:upeg/src/state/host_state_provider.dart';

void main() {
  group('TrayMenuSync', () {
    testWidgets(
      'TrayMenuSync_resets_the_context_menu_when_pauseControllable_changes',
      (tester) async {
        final controller = StreamController<HostStateEvent>();
        addTearDown(controller.close);
        final resets = <Menu>[];
        final container = ProviderContainer(
          overrides: [
            hostStateStreamProvider.overrideWith((ref) => controller.stream),
          ],
        );
        addTearDown(container.dispose);

        await tester.pumpWidget(
          UncontrolledProviderScope(
            container: container,
            child: MaterialApp(
              home: TrayMenuSync(
                setContextMenu: (menu) async {
                  resets.add(menu);
                },
                child: const SizedBox.shrink(),
              ),
            ),
          ),
        );
        await tester.pump();

        // No event has arrived yet (host state defaults to
        // not-controllable) — no re-set on initial mount either way;
        // the initial menu is applied by `UpegTray.install` directly.
        expect(resets, isEmpty);

        // Embedded (this process hosts in-process) flips controllable
        // false → true.
        controller.add(const HostStateEvent.embedded(endpoint: 'unix:/tmp/e'));
        await tester.pump();
        await tester.pump();

        expect(resets, hasLength(1));
        final firstItem = (resets[0].items ?? const <MenuItem>[]).firstWhere(
          (item) => item.key == TrayCommand.togglePause.menuItemKey,
        );
        expect(firstItem.disabled, isFalse);

        // Attached (a foreign daemon) flips controllable true → false.
        controller.add(const HostStateEvent.attached(endpoint: 'unix:/tmp/d'));
        await tester.pump();
        await tester.pump();

        expect(resets, hasLength(2));
        final secondItem = (resets[1].items ?? const <MenuItem>[]).firstWhere(
          (item) => item.key == TrayCommand.togglePause.menuItemKey,
        );
        expect(
          secondItem.disabled,
          isTrue,
          reason:
              'menu re-set after the flip must carry the new (disabled) state',
        );
      },
    );

    testWidgets('TrayMenuSync_isolates_platform_call_exceptions', (
      tester,
    ) async {
      final originalDebugPrint = debugPrint;
      final logs = <String>[];
      debugPrint = (String? message, {int? wrapWidth}) {
        if (message != null) logs.add(message);
      };

      final controller = StreamController<HostStateEvent>();
      addTearDown(controller.close);
      final container = ProviderContainer(
        overrides: [
          hostStateStreamProvider.overrideWith((ref) => controller.stream),
        ],
      );
      addTearDown(container.dispose);

      try {
        await tester.pumpWidget(
          UncontrolledProviderScope(
            container: container,
            child: MaterialApp(
              home: TrayMenuSync(
                setContextMenu: (_) async {
                  throw StateError('missing platform method');
                },
                child: const Text('board'),
              ),
            ),
          ),
        );
        await tester.pump();

        controller.add(const HostStateEvent.embedded(endpoint: 'unix:/tmp/e'));
        await tester.pump();
        await tester.pump();
      } finally {
        debugPrint = originalDebugPrint;
      }

      expect(find.text('board'), findsOneWidget);
      expect(tester.takeException(), isNull);
      expect(logs, contains(contains('upeg: setContextMenu failed')));
    });
  });
}
