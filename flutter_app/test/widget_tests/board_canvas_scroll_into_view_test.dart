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
    test('이미_완전히_보이는_구간은_offset을_바꾸지_않는다', () {
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

    test('우측_또는_아래로_잘린_구간은_끝만_보이도록_최소_이동한다', () {
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

    test('부분적으로_잘린_구간도_최소_이동으로_노출한다', () {
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

    test('왼쪽_또는_위로_잘린_구간은_시작이_보이도록_이동한다', () {
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

    test('뷰포트보다_큰_구간은_시작_모서리를_정렬한다', () {
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
    testWidgets('뷰포트_밖_pin에_focus하면_양_축_모두_최소_이동으로_노출한다', (tester) async {
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
    });

    testWidgets('이미_보이는_pin에_focus하면_스크롤하지_않는다', (tester) async {
      await tester.pumpWidget(_harness());
      await tester.pump();

      _focus(tester, _nearToolId);
      await tester.pump();

      expect(_horizontalController(tester).offset, 0);
      expect(_verticalController(tester).offset, 0);
    });

    testWidgets('grid가_mount될_때_이미_focus된_pin도_노출한다', (tester) async {
      // Board/palette 전환 직후의 focus 복원 경로: focus가 세팅된 상태로
      // canvas가 새로 mount되어도 pin이 뷰포트 안으로 들어와야 한다.
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
