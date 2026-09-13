/// Unit tests for the pegboard peg-hole dot geometry ("구멍 = 좌표계").
///
/// [PegboardDotsPainter]'s dot pitch must be DERIVED from the same cell
/// constants [_AbsoluteGrid] uses to place pins (`UpegSizing.pinCellWidth`
/// / `boardCanvasPinCellHeight` + `UpegSizing.pinGap`) — never an
/// independent magic number — so a hole always lands on the
/// gap-centred intersection between two adjacent cell slots. These
/// tests pin that arithmetic down numerically, pure (no widget tree).
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/board_canvas.dart';

import '../test_helpers/i18n_test_catalog.dart';

Widget _harness({required LayoutSnapshotDto snapshot}) {
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
        body: debugBoardCanvasGrid(snapshot: snapshot, onPinTap: (_) {}),
      ),
    ),
  );
}

void main() {
  group('PegboardDotsPainter pitch', () {
    test('pitchX는_셀_너비와_gap의_합에서_파생된다', () {
      expect(
        PegboardDotsPainter.pitchX,
        UpegSizing.pinCellWidth + UpegSizing.pinGap,
      );
    });

    test('pitchY는_셀_높이와_gap의_합에서_파생된다', () {
      expect(
        PegboardDotsPainter.pitchY,
        boardCanvasPinCellHeight + UpegSizing.pinGap,
      );
    });

    test('pitchX와_pitchY는_셀이_정사각형이_아니므로_서로_다르다', () {
      // Regression guard: a single shared `_spacing` constant (the old
      // implementation) would silently make these equal and misalign
      // the dot grid against the (non-square) cell grid.
      expect(PegboardDotsPainter.pitchX, isNot(PegboardDotsPainter.pitchY));
    });
  });

  group('PegboardDotsPainter 구멍 위치 공식', () {
    test('holeX(0)은_pitchX의_절반_gap만큼_왼쪽에_위치한다', () {
      expect(PegboardDotsPainter.holeX(0), -UpegSizing.pinGap / 2);
    });

    test('holeY(0)은_pitchY의_절반_gap만큼_위쪽에_위치한다', () {
      expect(PegboardDotsPainter.holeY(0), -UpegSizing.pinGap / 2);
    });

    test('holeX(k)는_k_pitchX에서_반_gap을_뺀_값이다', () {
      for (final k in [1, 2, 3, 6]) {
        expect(
          PegboardDotsPainter.holeX(k),
          k * PegboardDotsPainter.pitchX - UpegSizing.pinGap / 2,
        );
      }
    });

    test('holeY(k)는_k_pitchY에서_반_gap을_뺀_값이다', () {
      for (final k in [1, 2, 3, 6]) {
        expect(
          PegboardDotsPainter.holeY(k),
          k * PegboardDotsPainter.pitchY - UpegSizing.pinGap / 2,
        );
      }
    });

    test('연속한_두_구멍_사이_간격은_pitch와_같다', () {
      expect(
        PegboardDotsPainter.holeX(3) - PegboardDotsPainter.holeX(2),
        PegboardDotsPainter.pitchX,
      );
      expect(
        PegboardDotsPainter.holeY(4) - PegboardDotsPainter.holeY(3),
        PegboardDotsPainter.pitchY,
      );
    });

    test('셀_c의_네_모서리는_holeX_c_holeX_c_plus_1_holeY_r_holeY_r_plus_1로_감싸진다', () {
      // Cell (c, r)'s rendered rect starts at _AbsoluteGrid.pixelX(c) and
      // spans pinCellWidth — the surrounding hole pair must bracket that
      // exact span (hole just before the left edge, hole just after the
      // right edge, both a half-gap outside the cell body).
      const c = 2;
      const r = 3;
      final cellLeft = c * (UpegSizing.pinCellWidth + UpegSizing.pinGap);
      final cellRight = cellLeft + UpegSizing.pinCellWidth;
      expect(PegboardDotsPainter.holeX(c), lessThan(cellLeft));
      expect(PegboardDotsPainter.holeX(c + 1), greaterThan(cellRight));

      final cellTop = r * (boardCanvasPinCellHeight + UpegSizing.pinGap);
      final cellBottom = cellTop + boardCanvasPinCellHeight;
      expect(PegboardDotsPainter.holeY(r), lessThan(cellTop));
      expect(PegboardDotsPainter.holeY(r + 1), greaterThan(cellBottom));
    });
  });

  group('BoardCanvas 도트 스크롤 정합', () {
    testWidgets('빈_보드에서도_도트가_보인다', (tester) async {
      await tester.pumpWidget(
        _harness(
          snapshot: const LayoutSnapshotDto(
            boardKey: 'dev',
            boardCols: 6,
            placements: <PlacementDto>[],
          ),
        ),
      );
      await tester.pumpAndSettle();

      final paint = tester.widget<CustomPaint>(
        find.descendant(
          of: find.byType(BoardCanvasPegboardForTesting),
          matching: find.byType(CustomPaint),
        ),
      );
      final painter = paint.painter! as PegboardDotsPainter;
      expect(painter.showHoles, isTrue);
    });

    testWidgets('핀이_있는_보드에서는_도트_레이어가_스크롤_컨텐츠_안쪽에_위치한다', (tester) async {
      await tester.pumpWidget(
        _harness(
          snapshot: const LayoutSnapshotDto(
            boardKey: 'dev',
            boardCols: 6,
            placements: [
              PlacementDto(
                toolId: 'num.hex_to_decimal',
                x: 0,
                y: 0,
                w: 1,
                h: 1,
              ),
            ],
          ),
        ),
      );
      await tester.pumpAndSettle();

      // The dots layer must be a DESCENDANT of the vertical scroll view
      // (so it scrolls with the cells) — not a sibling/ancestor painting
      // at fixed viewport coordinates.
      final dotsFinder = find.byKey(const Key('board-canvas-grid-dots'));
      expect(dotsFinder, findsOneWidget);
      expect(
        find.descendant(
          of: find.byKey(const Key('board-canvas-vertical-scroll')),
          matching: dotsFinder,
        ),
        findsOneWidget,
      );

      final paint = tester.widget<CustomPaint>(dotsFinder);
      final painter = paint.painter! as PegboardDotsPainter;
      expect(painter.showHoles, isTrue);

      // The grid-path wallpaper layer must be suppressed (paintDots:
      // false) so it doesn't double-paint a second, non-scrolling dot
      // grid underneath the scroll-synced one.
      final pegboard = tester.widget<BoardCanvasPegboardForTesting>(
        find.byType(BoardCanvasPegboardForTesting),
      );
      expect(pegboard.paintDots, isFalse);
    });
  });
}
