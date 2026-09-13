/// Widget tests for the pegboard drop-target hover highlight
/// ("pegboard 표현 회복" spec §2/§3).
///
/// While a pin is being dragged over the grid, `_AbsoluteGrid` must show
/// a `Key('board-canvas-drop-highlight')` overlay sized to the DRAGGED
/// pin's real footprint, coloured by the FRB `preview_push` verdict:
///   * empty preview (anchor rejected)              -> warn,
///   * non-empty preview that displaces ANOTHER pin  -> warn,
///   * non-empty preview that leaves everything else
///     untouched                                     -> accent.
/// The FRB call is intercepted via the typed `pushPreviewLoaderProvider`
/// seam (same pattern as `board_canvas_push_preview_test.dart`) so these
/// tests stay dylib-free; the drag itself is a real gesture simulation
/// (`tester.startGesture` + `moveBy`), matching the drag tests in
/// `board_canvas_test.dart`.
///
/// Also covers the plain mouse-hover border on an EMPTY cell
/// (`Key('board-canvas-cell-hover')`), which is a separate, non-drag
/// affordance (spec §3).
library;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/push_preview_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/board_canvas.dart';
import 'package:upeg/src/widgets/pin.dart';

import '../test_helpers/i18n_test_catalog.dart';

const Key _highlightKey = Key('board-canvas-drop-highlight');
const Key _cellHoverKey = Key('board-canvas-cell-hover');

Widget _harness({
  required LayoutSnapshotDto snapshot,
  required PushPreviewLoader pushPreviewLoader,
}) {
  return ProviderScope(
    overrides: [
      ...i18nTestOverrides,
      toolsLoaderProvider.overrideWith(
        (ref) =>
            () => const <ToolDto>[],
      ),
      pushPreviewLoaderProvider.overrideWithValue(pushPreviewLoader),
    ],
    child: MaterialApp(
      theme: UpegTheme.darkTheme(),
      home: Scaffold(
        body: SizedBox(
          width: 2000,
          height: 1000,
          child: debugBoardCanvasGrid(snapshot: snapshot, onPinTap: (_) {}),
        ),
      ),
    ),
  );
}

/// Drags [pinFinder] (default: the sole [Pin]) one cell to the right
/// (column N -> column N+1) via a real pointer gesture, WITHOUT lifting
/// — callers assert on the mid-drag highlight, then must call
/// `gesture.up()` themselves.
Future<TestGesture> _dragOneColumnRight(
  WidgetTester tester, {
  Finder? pinFinder,
}) async {
  final pinCenter = tester.getCenter(pinFinder ?? find.byType(Pin));
  final gesture = await tester.startGesture(pinCenter);
  final step = Offset(UpegSizing.pinCellWidth + UpegSizing.pinGap, 0);
  for (int i = 1; i <= 4; i++) {
    await gesture.moveBy(step / 4);
    await tester.pump(const Duration(milliseconds: 16));
  }
  return gesture;
}

BoxDecoration _highlightDecoration(WidgetTester tester) {
  final decoratedBox = tester.widget<DecoratedBox>(
    find.descendant(
      of: find.byKey(_highlightKey),
      matching: find.byType(DecoratedBox),
    ),
  );
  return decoratedBox.decoration as BoxDecoration;
}

void main() {
  group('BoardCanvas 드롭 하이라이트', () {
    testWidgets('충돌없는_드롭은_accent_하이라이트를_보여준다', (tester) async {
      const snapshot = LayoutSnapshotDto(
        boardKey: 'dev',
        boardCols: 6,
        placements: [PlacementDto(toolId: 'fixture.a', x: 0, y: 0, w: 1, h: 1)],
      );
      // The dragged tool moving alone, nothing else displaced -> accept.
      List<PlacementDto> acceptLoader(
        BoardKey boardKey,
        ToolId toolId,
        int x,
        int y,
      ) {
        return [PlacementDto(toolId: toolId.value, x: x, y: y, w: 1, h: 1)];
      }

      await tester.pumpWidget(
        _harness(snapshot: snapshot, pushPreviewLoader: acceptLoader),
      );
      final gesture = await _dragOneColumnRight(tester);

      expect(find.byKey(_highlightKey), findsOneWidget);
      final decoration = _highlightDecoration(tester);
      expect(decoration.color, UpegTokens.dark.accent.withValues(alpha: 0.18));
      final border = decoration.border as Border;
      expect(border.top.color, UpegTokens.dark.accent);

      // Rect matches the hovered cell (col 1) and the dragged pin's own
      // 1x1 footprint — not the hovered cell's occupant (there is none).
      final positioned = tester.widget<Positioned>(
        find.ancestor(
          of: find.byKey(_highlightKey),
          matching: find.byType(Positioned),
        ),
      );
      expect(
        positioned.left,
        1 * (UpegSizing.pinCellWidth + UpegSizing.pinGap),
      );
      expect(positioned.top, 0.0);
      expect(positioned.width, UpegSizing.pinCellWidth);
      expect(positioned.height, boardCanvasPinCellHeight);

      await gesture.up();
      await tester.pumpAndSettle();
    });

    testWidgets('다른_pin을_밀어내는_드롭은_warn_하이라이트를_보여준다', (tester) async {
      const snapshot = LayoutSnapshotDto(
        boardKey: 'dev',
        boardCols: 6,
        placements: [
          PlacementDto(toolId: 'fixture.a', x: 0, y: 0, w: 1, h: 1),
          PlacementDto(toolId: 'fixture.b', x: 1, y: 0, w: 1, h: 1),
        ],
      );
      // Committing here would push fixture.b out of its current (1, 0)
      // slot -> the preview reports fixture.b displaced to (2, 0).
      List<PlacementDto> pushLoader(
        BoardKey boardKey,
        ToolId toolId,
        int x,
        int y,
      ) {
        return [
          PlacementDto(toolId: toolId.value, x: x, y: y, w: 1, h: 1),
          const PlacementDto(toolId: 'fixture.b', x: 2, y: 0, w: 1, h: 1),
        ];
      }

      await tester.pumpWidget(
        _harness(snapshot: snapshot, pushPreviewLoader: pushLoader),
      );
      // fixture.a renders first in the placements list — its Pin sits at
      // column 0 (fixture.b at column 1).
      final gesture = await _dragOneColumnRight(
        tester,
        pinFinder: find.byType(Pin).first,
      );

      expect(find.byKey(_highlightKey), findsOneWidget);
      final decoration = _highlightDecoration(tester);
      expect(decoration.color, UpegTokens.dark.warn.withValues(alpha: 0.18));
      final border = decoration.border as Border;
      expect(border.top.color, UpegTokens.dark.warn);

      await gesture.up();
      await tester.pumpAndSettle();
    });

    testWidgets('빈_preview로_거부되는_드롭은_warn_하이라이트를_보여준다', (tester) async {
      const snapshot = LayoutSnapshotDto(
        boardKey: 'dev',
        boardCols: 6,
        placements: [PlacementDto(toolId: 'fixture.a', x: 0, y: 0, w: 1, h: 1)],
      );
      // Empty preview list == FRB rejected the anchor.
      List<PlacementDto> rejectLoader(
        BoardKey boardKey,
        ToolId toolId,
        int x,
        int y,
      ) => const <PlacementDto>[];

      await tester.pumpWidget(
        _harness(snapshot: snapshot, pushPreviewLoader: rejectLoader),
      );
      final gesture = await _dragOneColumnRight(tester);

      expect(find.byKey(_highlightKey), findsOneWidget);
      final decoration = _highlightDecoration(tester);
      expect(decoration.color, UpegTokens.dark.warn.withValues(alpha: 0.18));

      await gesture.up();
      await tester.pumpAndSettle();
    });

    testWidgets('드롭_완료_후에는_하이라이트가_사라진다', (tester) async {
      const snapshot = LayoutSnapshotDto(
        boardKey: 'dev',
        boardCols: 6,
        placements: [PlacementDto(toolId: 'fixture.a', x: 0, y: 0, w: 1, h: 1)],
      );
      List<PlacementDto> acceptLoader(
        BoardKey boardKey,
        ToolId toolId,
        int x,
        int y,
      ) {
        return [PlacementDto(toolId: toolId.value, x: x, y: y, w: 1, h: 1)];
      }

      await tester.pumpWidget(
        _harness(snapshot: snapshot, pushPreviewLoader: acceptLoader),
      );
      final gesture = await _dragOneColumnRight(tester);
      expect(find.byKey(_highlightKey), findsOneWidget);

      await gesture.up();
      await tester.pumpAndSettle();

      expect(find.byKey(_highlightKey), findsNothing);
    });

    testWidgets('드래그_취소_후에는_하이라이트가_사라진다', (tester) async {
      const snapshot = LayoutSnapshotDto(
        boardKey: 'dev',
        boardCols: 6,
        placements: [PlacementDto(toolId: 'fixture.a', x: 0, y: 0, w: 1, h: 1)],
      );
      List<PlacementDto> acceptLoader(
        BoardKey boardKey,
        ToolId toolId,
        int x,
        int y,
      ) {
        return [PlacementDto(toolId: toolId.value, x: x, y: y, w: 1, h: 1)];
      }

      await tester.pumpWidget(
        _harness(snapshot: snapshot, pushPreviewLoader: acceptLoader),
      );
      final gesture = await _dragOneColumnRight(tester);
      expect(find.byKey(_highlightKey), findsOneWidget);

      await gesture.cancel();
      await tester.pumpAndSettle();

      expect(find.byKey(_highlightKey), findsNothing);
    });
  });

  group('BoardCanvas 빈 셀 마우스 hover', () {
    testWidgets('마우스가_빈_셀_위에_있으면_경계를_보여준다', (tester) async {
      const snapshot = LayoutSnapshotDto(
        boardKey: 'dev',
        boardCols: 6,
        placements: [PlacementDto(toolId: 'fixture.a', x: 0, y: 0, w: 1, h: 1)],
      );
      List<PlacementDto> loader(
        BoardKey boardKey,
        ToolId toolId,
        int x,
        int y,
      ) => const <PlacementDto>[];

      await tester.pumpWidget(
        _harness(snapshot: snapshot, pushPreviewLoader: loader),
      );

      expect(find.byKey(_cellHoverKey), findsNothing);

      // Column 2 is empty (fixture.a occupies column 0 only).
      final emptyCellCenter =
          tester.getTopLeft(find.byType(Pin)) +
          Offset(
            2 * (UpegSizing.pinCellWidth + UpegSizing.pinGap) +
                UpegSizing.pinCellWidth / 2,
            boardCanvasPinCellHeight / 2,
          );

      final gesture = await tester.createGesture(kind: PointerDeviceKind.mouse);
      addTearDown(() => gesture.removePointer());
      await gesture.addPointer(location: emptyCellCenter);
      await tester.pump();

      expect(find.byKey(_cellHoverKey), findsOneWidget);

      // Moving off-canvas clears the affordance again.
      await gesture.moveTo(const Offset(-500, -500));
      await tester.pump();
      expect(find.byKey(_cellHoverKey), findsNothing);
    });
  });
}
