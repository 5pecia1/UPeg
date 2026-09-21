/// F03 — Right-click on a [Pin] shows an "unpin" context menu that
/// dispatches through the central pegboard mutation path.
///
/// The retired Dioxus surface bound TogglePin to a context-menu
/// click on the pin tile. Flutter preserves that affordance through
/// the [Pin] secondary-tap handler and the canvas-level unpin seam
/// (inventory row F03).
library;

import 'package:flutter/material.dart';
import 'package:flutter/gestures.dart' show kSecondaryButton;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/pegboard_mutations_provider.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/pin.dart';

import '../test_helpers/fake_binding_catalog.dart';
import '../test_helpers/i18n_test_catalog.dart';

const _placement = PlacementDto(
  toolId: 'num.hex_to_decimal',
  x: 0,
  y: 0,
  w: 1,
  h: 1,
);

/// A placement carrying a user span override — the precondition for the
/// "reset size" menu item to appear.
const _resizedPlacement = PlacementDto(
  toolId: 'num.hex_to_decimal',
  x: 0,
  y: 0,
  w: 2,
  h: 1,
  spanCols: 2,
  spanRows: 1,
);

Widget _harness({
  PinMutator? unpin,
  ClearPinSpanMutator? clearSpan,
  String? boardKey,
  PinTapCallback? onTap,
  PinTapCallback? onOpenModal,
  PlacementDto placement = _placement,
}) {
  return ProviderScope(
    overrides: [
      ...i18nTestOverrides,
      fakeBindingCatalogOverride,
      if (unpin != null) unpinToolMutatorProvider.overrideWithValue(unpin),
      if (clearSpan != null)
        clearPinSpanMutatorProvider.overrideWithValue(clearSpan),
      if (boardKey != null)
        currentBoardKeyProvider.overrideWith(
          () => _SeededCurrentBoard(boardKey),
        ),
    ],
    child: MaterialApp(
      theme: UpegTheme.darkTheme(),
      home: Scaffold(
        body: SizedBox(
          width: 220,
          height: 120,
          child: Pin(
            placement: placement,
            toolLabel: 'hex → dec',
            onTap: onTap,
            onOpenModal: onOpenModal,
          ),
        ),
      ),
    ),
  );
}

class _SeededCurrentBoard extends CurrentBoardNotifier {
  _SeededCurrentBoard(this._seed);
  final String _seed;
  @override
  BoardKey? build() {
    super.build();
    return BoardKey.parse(_seed);
  }
}

void main() {
  group('Pin context menu (F03)', () {
    testWidgets('a_pin_right_click_shows_the_unpin_menu_item', (tester) async {
      await tester.pumpWidget(_harness(boardKey: 'dev'));
      await tester.pump();

      // A right-click (secondary tap) raises the context menu.
      await tester.tapAt(
        tester.getCenter(find.byType(Pin)),
        buttons: kSecondaryButton,
      );
      await tester.pumpAndSettle();

      expect(find.text('unpin'), findsOneWidget);
    });

    testWidgets('tapping_the_pin_unpin_menu_item_calls_unpintool', (
      tester,
    ) async {
      BoardKey? observedBoard;
      ToolId? observedTool;
      void recorder(BoardKey boardKey, ToolId toolId) {
        observedBoard = boardKey;
        observedTool = toolId;
      }

      await tester.pumpWidget(_harness(unpin: recorder, boardKey: 'dev'));
      await tester.pump();

      await tester.tapAt(
        tester.getCenter(find.byType(Pin)),
        buttons: kSecondaryButton,
      );
      await tester.pumpAndSettle();
      await tester.tap(find.text('unpin'));
      await tester.pumpAndSettle();

      expect(observedBoard, BoardKey.parse('dev'));
      expect(observedTool, ToolId.parse('num.hex_to_decimal'));
    });

    testWidgets('a_pin_right_click_opens_the_expanded_modal', (tester) async {
      PlacementDto? opened;
      var tapped = false;
      await tester.pumpWidget(
        _harness(
          boardKey: 'dev',
          onTap: (_) => tapped = true,
          onOpenModal: (placement) => opened = placement,
        ),
      );
      await tester.pump();

      // Right-click (secondary tap) → context menu → "open" → invokes the
      // expanded-modal handler.
      await tester.tapAt(
        tester.getCenter(find.byType(Pin)),
        buttons: kSecondaryButton,
      );
      await tester.pumpAndSettle();
      expect(find.text('open'), findsOneWidget);
      await tester.tap(find.text('open'));
      await tester.pumpAndSettle();

      expect(opened?.toolId, 'num.hex_to_decimal');
      // The right-click path never fires Run (onTap).
      expect(tapped, isFalse);
    });

    testWidgets('a_pin_with_a_span_override_shows_the_reset_size_menu_item', (
      tester,
    ) async {
      await tester.pumpWidget(
        _harness(boardKey: 'dev', placement: _resizedPlacement),
      );
      await tester.pump();

      await tester.tapAt(
        tester.getCenter(find.byType(Pin)),
        buttons: kSecondaryButton,
      );
      await tester.pumpAndSettle();

      expect(find.byKey(const Key('pin-context-reset-size')), findsOneWidget);
      expect(find.text(i18nEn('pin.menu.reset_size')), findsOneWidget);
    });

    testWidgets(
      'a_pin_without_a_span_override_hides_the_reset_size_menu_item',
      (tester) async {
        await tester.pumpWidget(_harness(boardKey: 'dev'));
        await tester.pump();

        await tester.tapAt(
          tester.getCenter(find.byType(Pin)),
          buttons: kSecondaryButton,
        );
        await tester.pumpAndSettle();

        expect(find.byKey(const Key('pin-context-reset-size')), findsNothing);
      },
    );

    testWidgets('tapping_the_reset_size_menu_item_calls_clearpinspan', (
      tester,
    ) async {
      BoardKey? observedBoard;
      ToolId? observedTool;
      void recorder(BoardKey boardKey, ToolId toolId) {
        observedBoard = boardKey;
        observedTool = toolId;
      }

      await tester.pumpWidget(
        _harness(
          boardKey: 'dev',
          placement: _resizedPlacement,
          clearSpan: recorder,
        ),
      );
      await tester.pump();

      await tester.tapAt(
        tester.getCenter(find.byType(Pin)),
        buttons: kSecondaryButton,
      );
      await tester.pumpAndSettle();
      await tester.tap(find.text(i18nEn('pin.menu.reset_size')));
      await tester.pumpAndSettle();

      expect(observedBoard, BoardKey.parse('dev'));
      expect(observedTool, ToolId.parse('num.hex_to_decimal'));
    });

    testWidgets('context_menu_items_annotate_their_bound_key_labels', (
      tester,
    ) async {
      // The key labels come from the binding catalog, not hard-coding
      // (open→o, edit color→c, reset size→e, unpin→p).
      await tester.pumpWidget(
        _harness(
          boardKey: 'dev',
          placement: _resizedPlacement,
          onOpenModal: (_) {},
        ),
      );
      await tester.pump();

      await tester.tapAt(
        tester.getCenter(find.byType(Pin)),
        buttons: kSecondaryButton,
      );
      await tester.pumpAndSettle();

      Finder capInItem(String itemKey, String cap) => find.descendant(
        of: find.byKey(Key(itemKey)),
        matching: find.text(cap),
      );
      expect(capInItem('pin-context-open', 'o'), findsOneWidget);
      expect(capInItem('pin-context-edit-color', 'c'), findsOneWidget);
      expect(capInItem('pin-context-reset-size', 'e'), findsOneWidget);
      // The unpin item has no widget key — inside the menu the `p` cap
      // can only be unpin's.
      expect(find.text('p'), findsOneWidget);
    });

    testWidgets('a_pin_tap_runs_without_opening_the_modal', (tester) async {
      PlacementDto? opened;
      var tapped = false;
      await tester.pumpWidget(
        _harness(
          boardKey: 'dev',
          onTap: (_) => tapped = true,
          onOpenModal: (placement) => opened = placement,
        ),
      );
      await tester.pump();

      await tester.tap(find.byType(Pin));
      await tester.pumpAndSettle();

      // Inline-first: a tap only runs; it never opens the modal.
      expect(tapped, isTrue);
      expect(opened, isNull);
      expect(find.text('open'), findsNothing);
    });
  });
}
