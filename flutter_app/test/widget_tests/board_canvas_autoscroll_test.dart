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

void main() {
  group('BoardCanvas', () {
    testWidgets('BoardCanvas는 제약된 뷰포트에서 아래로 드래그하면 자동 스크롤한다', (tester) async {
      // This test uses a constrained viewport (600x300) so the vertical
      // scroll container is active from the start. The grid quickly expands
      // past the 300px viewport during drag, enabling autoscroll. When the
      // pointer approaches the bottom edge of the scroll viewport, the board
      // must both expand rows AND scroll downward so newly-created drop
      // targets become reachable without lift/re-drag.
      tester.view.physicalSize = const Size(600, 300);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            ...i18nTestOverrides,
            toolsLoaderProvider.overrideWith(
              (ref) =>
                  () => const <ToolDto>[],
            ),
            pushPreviewLoaderProvider.overrideWithValue(
              (boardKey, toolId, anchorX, anchorY) => const <PlacementDto>[],
            ),
          ],
          child: MaterialApp(
            home: Scaffold(
              body: debugBoardCanvasGrid(
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
                onPinTap: (_) {},
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      // Record the initial scroll offset as a baseline.
      final findScrollable = find.byType(Scrollable);
      final scrollable = tester.state<ScrollableState>(
        findScrollable.at(1), // inner = vertical scroll
      );
      final initialOffset = scrollable.position.pixels;

      // Start dragging the pin downward.
      final pinCenter = tester.getCenter(find.byType(Pin));
      const dragTargetRow = 10;
      const cellH = boardCanvasPinCellHeight;
      const gap = UpegSizing.pinGap;
      const rowSize = cellH + gap;
      final totalOffset = Offset(0, rowSize * dragTargetRow);
      const dragSlices = 40;

      final TestGesture gesture = await tester.startGesture(pinCenter);
      await gesture.moveBy(const Offset(1, 1));
      await tester.pump(const Duration(milliseconds: 16));

      for (int i = 1; i <= dragSlices; i++) {
        await gesture.moveBy(totalOffset / dragSlices.toDouble());
        await tester.pump(const Duration(milliseconds: 16));
      }

      // Mid-drag check: scroll offset must have increased beyond zero.
      // This proves autoscroll is working in a constrained viewport.
      final midDragScrollable = tester.state<ScrollableState>(
        findScrollable.at(1),
      );
      final midDragOffset = midDragScrollable.position.pixels;
      expect(
        midDragOffset,
        greaterThan(initialOffset),
        reason:
            'vertical scroll must auto-offset during downward drag in a '
            'constrained viewport — newly-expanded drop targets must be reachable',
      );

      // Grid must have also expanded.
      final midDragGridSize = tester.getSize(
        find.byKey(const Key('board-canvas-absolute-grid')),
      );
      expect(
        midDragGridSize.height,
        greaterThan(cellH * 2 + gap),
        reason: 'grid must have expanded below initial 2 rows',
      );

      await gesture.up();
      await tester.pumpAndSettle();
    });

    testWidgets('BoardCanvas는 제약된 뷰포트에서 위로 드래그하면 위쪽으로 자동 스크롤한다', (
      tester,
    ) async {
      // Regression: upward movement is not upward infinite expansion. A pin
      // already placed on a lower row must be able to drag/drop into existing
      // rows above it by scrolling the constrained viewport upward.
      tester.view.physicalSize = const Size(600, 300);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      ToolId? gotTool;
      int? gotY;
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            ...i18nTestOverrides,
            toolsLoaderProvider.overrideWith(
              (ref) =>
                  () => const <ToolDto>[],
            ),
            pushPreviewLoaderProvider.overrideWithValue(
              (boardKey, toolId, anchorX, anchorY) => const <PlacementDto>[],
            ),
          ],
          child: MaterialApp(
            home: Scaffold(
              body: debugBoardCanvasGrid(
                snapshot: const LayoutSnapshotDto(
                  boardKey: 'dev',
                  boardCols: 6,
                  placements: [
                    PlacementDto(
                      toolId: 'num.hex_to_decimal',
                      x: 0,
                      y: 8,
                      w: 1,
                      h: 1,
                    ),
                  ],
                ),
                onPinTap: (_) {},
                onMovePin:
                    ({
                      required boardKey,
                      required toolId,
                      required anchorX,
                      required anchorY,
                    }) {
                      gotTool = toolId;
                      gotY = anchorY;
                    },
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      await tester.ensureVisible(find.byType(Pin));
      await tester.pumpAndSettle();

      final findScrollable = find.byType(Scrollable);
      final scrollable = tester.state<ScrollableState>(
        findScrollable.at(1), // inner = vertical scroll
      );
      final initialOffset = scrollable.position.pixels;
      expect(
        initialOffset,
        greaterThan(0),
        reason: 'lower-row pin setup must start with vertical scroll available',
      );

      final pinCenter = tester.getCenter(find.byType(Pin));
      final TestGesture gesture = await tester.startGesture(pinCenter);
      const topEdgeY = 40.0;
      const rowSize = boardCanvasPinCellHeight + UpegSizing.pinGap;
      final approachOffset = Offset(0, topEdgeY - pinCenter.dy);
      const approachSlices = 12;
      for (int i = 1; i <= approachSlices; i++) {
        await gesture.moveBy(approachOffset / approachSlices.toDouble());
        await tester.pump(const Duration(milliseconds: 16));
      }

      final offsetAfterApproach = scrollable.position.pixels;

      // Keep the pointer inside the top autoscroll zone. Tiny horizontal
      // movement keeps DragUpdate events flowing without changing columns.
      for (int i = 0; i < 24; i++) {
        await gesture.moveBy(
          i.isEven ? const Offset(1, 0) : const Offset(-1, 0),
        );
        await tester.pump(const Duration(milliseconds: 16));
      }

      final midDragOffset = scrollable.position.pixels;
      expect(
        midDragOffset,
        lessThan(offsetAfterApproach - rowSize),
        reason:
            'vertical scroll must keep auto-offsetting upward while the drag '
            'stays in the constrained viewport top edge zone',
      );
      expect(
        midDragOffset,
        greaterThanOrEqualTo(0),
        reason: 'upward autoscroll must stay clamped at the scroll top',
      );

      await gesture.up();
      await tester.pumpAndSettle();

      expect(gotTool, ToolId.parse('num.hex_to_decimal'));
      expect(
        gotY,
        lessThan(8),
        reason: 'drop must commit to an existing row above the original row',
      );
    });

    testWidgets('BoardCanvas는 제약된 뷰포트에서 스크롤 오프셋이 0이다', (tester) async {
      // Regression: with a large viewport the autoscroll path should be a
      // no-op (the scroll container exists but never scrolls because all
      // content fits in view).
      tester.view.physicalSize = const Size(2200, 2200);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            ...i18nTestOverrides,
            toolsLoaderProvider.overrideWith(
              (ref) =>
                  () => const <ToolDto>[],
            ),
          ],
          child: MaterialApp(
            home: Scaffold(
              body: SizedBox(
                width: 2000,
                height: 2000,
                child: debugBoardCanvasGrid(
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
                  onPinTap: (_) {},
                ),
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      final findScrollable2 = find.byType(Scrollable);
      final scrollable = tester.state<ScrollableState>(findScrollable2.at(1));
      expect(
        scrollable.position.pixels,
        0,
        reason: 'large viewport must not need scroll — offset stays at zero',
      );
    });
  });
}
