/// Focus scroll-into-view: moving keyboard focus (arrows / hjkl) onto a
/// pin outside the viewport must scroll the canvas just enough to reveal
/// it — and never move at all when the pin is already fully visible.
///
/// The offset math lives in the pure [scrollOffsetToReveal] helper
/// (unit-tested directly); the widget tests drive the grid through
/// [focusedPinProvider] the same way BoardPage keyboard navigation does.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/focused_pin_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/board_canvas.dart';

import '../test_helpers/i18n_test_catalog.dart';

const Key _gridKey = Key('board-canvas-grid');
const Key _verticalScrollKey = Key('board-canvas-vertical-scroll');

const String _nearToolId = 'fixture.near';
const String _farToolId = 'fixture.far';
const int _farCol = 5;
const int _farRow = 12;

const LayoutSnapshotDto _snapshot = LayoutSnapshotDto(
  boardKey: 'dev',
  boardCols: 6,
  placements: [
    PlacementDto(toolId: _nearToolId, x: 0, y: 0, w: 1, h: 1),
    PlacementDto(toolId: _farToolId, x: _farCol, y: _farRow, w: 1, h: 1),
  ],
);

double _cellLeft(int col) =>
    UpegSizing.pegboardPadding +
    col * (UpegSizing.pinCellWidth + UpegSizing.pinGap);

double _cellTop(int row) =>
    UpegSizing.pegboardPadding +
    row * (boardCanvasPinCellHeight + UpegSizing.pinGap);

Widget _harness() {
  return ProviderScope(
    overrides: [
      ...i18nTestOverrides,
      toolsLoaderProvider.overrideWith(
        (ref) =>
            () => const <ToolDto>[],
      ),
    ],
    child: MaterialApp(
      home: Scaffold(
        body: debugBoardCanvasGrid(snapshot: _snapshot, onPinTap: (_) {}),
      ),
    ),
  );
}

ScrollController _horizontalController(WidgetTester tester) =>
    tester.widget<SingleChildScrollView>(find.byKey(_gridKey)).controller!;

ScrollController _verticalController(WidgetTester tester) => tester
    .widget<SingleChildScrollView>(find.byKey(_verticalScrollKey))
    .controller!;

void _focus(WidgetTester tester, String toolId) {
  ProviderScope.containerOf(
    tester.element(find.byKey(_gridKey)),
    listen: false,
  ).read(focusedPinProvider.notifier).focus(ToolId.parse(toolId));
}

void main() {
  group('scrollOffsetToReveal', () {
    test('an_already_fully_visible_range_does_not_change_the_offset', () {
      expect(
        scrollOffsetToReveal(
          currentOffset: 100,
          viewportExtent: 600,
          targetStart: 200,
          targetEnd: 350,
        ),
        100,
      );
    });

    test('a_range_clipped_right_or_below_moves_minimally_to_show_its_end', () {
      // Viewport [0, 600), target [700, 850) → move so 850 is flush with
      // the viewport end: minimal offset 250, not target-start 700.
      expect(
        scrollOffsetToReveal(
          currentOffset: 0,
          viewportExtent: 600,
          targetStart: 700,
          targetEnd: 850,
        ),
        250,
      );
    });

    test('a_partially_clipped_range_is_revealed_with_minimal_movement', () {
      // Viewport [0, 600), target [500, 650) → only 50px of movement.
      expect(
        scrollOffsetToReveal(
          currentOffset: 0,
          viewportExtent: 600,
          targetStart: 500,
          targetEnd: 650,
        ),
        50,
      );
    });

    test('a_range_clipped_left_or_above_moves_to_show_its_start', () {
      expect(
        scrollOffsetToReveal(
          currentOffset: 300,
          viewportExtent: 600,
          targetStart: 120,
          targetEnd: 270,
        ),
        120,
      );
    });

    test('a_range_larger_than_the_viewport_aligns_its_start_edge', () {
      expect(
        scrollOffsetToReveal(
          currentOffset: 0,
          viewportExtent: 300,
          targetStart: 400,
          targetEnd: 900,
        ),
        400,
      );
    });
  });

  group('BoardCanvas focus scroll-into-view', () {
    testWidgets(
      'focusing_a_pin_outside_the_viewport_reveals_it_with_minimal_movement_on_both_axes',
      (tester) async {
        await tester.pumpWidget(_harness());
        await tester.pump();

        _focus(tester, _farToolId);
        await tester.pump();

        final viewport = tester.getSize(find.byKey(_gridKey));
        final expectedH =
            _cellLeft(_farCol) + UpegSizing.pinCellWidth - viewport.width;
        final expectedV =
            _cellTop(_farRow) + boardCanvasPinCellHeight - viewport.height;
        expect(_horizontalController(tester).offset, expectedH);
        expect(_verticalController(tester).offset, expectedV);
      },
    );

    testWidgets('focusing_an_already_visible_pin_does_not_scroll', (
      tester,
    ) async {
      await tester.pumpWidget(_harness());
      await tester.pump();

      _focus(tester, _nearToolId);
      await tester.pump();

      expect(_horizontalController(tester).offset, 0);
      expect(_verticalController(tester).offset, 0);
    });

    testWidgets('a_pin_already_focused_when_the_grid_mounts_is_also_revealed', (
      tester,
    ) async {
      // The focus-restore path right after a board/palette switch: even
      // when the canvas mounts with focus already set, the pin must end
      // up inside the viewport.
      final container = ProviderContainer(
        overrides: [
          ...i18nTestOverrides,
          toolsLoaderProvider.overrideWith(
            (ref) =>
                () => const <ToolDto>[],
          ),
        ],
      );
      addTearDown(container.dispose);
      container
          .read(focusedPinProvider.notifier)
          .focus(ToolId.parse(_farToolId));

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: MaterialApp(
            home: Scaffold(
              body: debugBoardCanvasGrid(snapshot: _snapshot, onPinTap: (_) {}),
            ),
          ),
        ),
      );
      await tester.pump();

      expect(_horizontalController(tester).offset, greaterThan(0));
      expect(_verticalController(tester).offset, greaterThan(0));
    });
  });
}
