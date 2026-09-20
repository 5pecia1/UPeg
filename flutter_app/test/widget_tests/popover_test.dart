/// Widget tests for [showUpegPopover].
///
/// The popover implements the "ghost button" right-click menu (a look
/// carried over from the retired Dioxus surface) — load-bearing
/// behavior is:
///   1) selecting an item returns its value through the showUpegPopover Future,
///   2) tapping outside the surface dismisses with `null` (barrier dismissal).
library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/popover.dart';

import '../test_helpers/fake_keyboard_resolver.dart';

import '../test_helpers/i18n_test_catalog.dart';

Widget _hostHarness(Future<void> Function(BuildContext context) opener) {
  return ProviderScope(
    overrides: [...i18nTestOverrides, fakeKeyboardResolverOverride],
    child: MaterialApp(
      theme: UpegTheme.darkTheme(),
      home: Builder(
        builder: (context) => Scaffold(
          body: Center(
            child: TextButton(
              key: const Key('open-popover-btn'),
              onPressed: () async {
                await opener(context);
              },
              child: const Text('open'),
            ),
          ),
        ),
      ),
    ),
  );
}

void main() {
  group('showUpegPopover', () {
    testWidgets('the_popover_returns_the_selected_items_value', (tester) async {
      String? picked;
      await tester.pumpWidget(
        _hostHarness((ctx) async {
          picked = await showUpegPopover<String>(
            context: ctx,
            globalPosition: const Offset(50, 50),
            items: const <UpegPopoverItem<String>>[
              UpegPopoverItem(
                value: 'rename',
                label: 'rename',
                key: Key('item-rename'),
              ),
              UpegPopoverItem(
                value: 'delete',
                label: 'delete',
                key: Key('item-delete'),
              ),
            ],
          );
        }),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('open-popover-btn')));
      await tester.pump();

      await tester.tap(find.byKey(const Key('item-delete')));
      await tester.pump();

      expect(picked, 'delete');
    });

    testWidgets('the_popover_returns_null_on_a_barrier_tap', (tester) async {
      String? picked = 'sentinel';
      await tester.pumpWidget(
        _hostHarness((ctx) async {
          picked = await showUpegPopover<String>(
            context: ctx,
            globalPosition: const Offset(50, 50),
            items: const <UpegPopoverItem<String>>[
              UpegPopoverItem(
                value: 'rename',
                label: 'rename',
                key: Key('item-rename'),
              ),
            ],
          );
        }),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('open-popover-btn')));
      await tester.pump();

      // Tap somewhere outside the surface (top-left corner) to fire the
      // PopupRoute barrier dismiss. One pump is enough because the route
      // has no reverse transition.
      await tester.tapAt(const Offset(2, 2));
      await tester.pump();

      expect(picked, isNull);
    });

    testWidgets('the_popover_navigates_and_selects_via_the_keyboard', (
      tester,
    ) async {
      String? picked;
      await tester.pumpWidget(
        _hostHarness((ctx) async {
          picked = await showUpegPopover<String>(
            context: ctx,
            globalPosition: const Offset(50, 50),
            items: const <UpegPopoverItem<String>>[
              UpegPopoverItem(
                value: 'rename',
                label: 'rename',
                key: Key('item-rename'),
              ),
              UpegPopoverItem(
                value: 'delete',
                label: 'delete',
                key: Key('item-delete'),
              ),
            ],
          );
        }),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('open-popover-btn')));
      await tester.pumpAndSettle();

      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();

      expect(picked, 'delete');
    });
  });
}
