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
import '../test_helpers/board_canvas_harness.dart';

void main() {
  group('BoardCanvas', () {
    testWidgets('BoardCanvas_moves_a_pin_with_an_immediate_drag', (
      tester,
    ) async {
      ToolId? gotTool;
      int? gotX;
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
              body: SizedBox(
                width: 2000,
                height: 1000,
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
                  onMovePin:
                      ({
                        required boardKey,
                        required toolId,
                        required anchorX,
                        required anchorY,
                      }) {
                        gotTool = toolId;
                        gotX = anchorX;
                        gotY = anchorY;
                      },
                ),
              ),
            ),
          ),
        ),
      );

      final pinFinder = find.byType(Pin);
      expect(pinFinder, findsOneWidget);
      final pinCenter = tester.getCenter(pinFinder);
      final TestGesture gesture = await tester.startGesture(pinCenter);
      final targetOffset = Offset(
        UpegSizing.pinCellWidth + UpegSizing.pinGap,
        0,
      );
      for (int i = 1; i <= 4; i++) {
        await gesture.moveBy(targetOffset / 4);
        await tester.pump(const Duration(milliseconds: 16));
      }
      await gesture.up();
      await tester.pumpAndSettle();

      expect(gotTool, ToolId.parse('num.hex_to_decimal'));
      // Column 1 is the drop target the cursor landed in. `anchorY`
      // stays 0 since no vertical drag.
      expect(gotX, 1);
      expect(gotY, 0);
    });

    testWidgets(
      'BoardCanvas_pre_expands_the_drop_target_when_a_pin_is_dragged_to_the_bottom_edge',
      (tester) async {
        // Verifies that dragging a pin below the initial rendered grid triggers
        // dynamic row expansion via Draggable.onDragUpdate, NOT requiring the
        // pointer to pass through existing _DropCell widgets.
        const cellH = boardCanvasPinCellHeight;
        const gap = UpegSizing.pinGap;
        const columns = 6;
        const initialRows = 2;
        final initialGridHeight = cellH * initialRows + gap * (initialRows - 1);

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
                body: SizedBox(
                  width: 1200,
                  height: 2000,
                  child: debugBoardCanvasGrid(
                    snapshot: const LayoutSnapshotDto(
                      boardKey: 'dev',
                      boardCols: columns,
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

        // Verify initial grid height matches exactly initialRows.
        final initialGridSize = tester.getSize(
          find.byKey(const Key('board-canvas-absolute-grid')),
        );
        expect(
          initialGridSize.height,
          initialGridHeight.toDouble(),
          reason: 'grid must start at exactly initialRows before drag',
        );

        // Drag the pin far below the initial grid using fine-grained moves.
        // The onDragUpdate callback fires on each move, computing the grid row
        // from global pointer position and calling _expandForHoveredRow.
        const dragTargetRow = 4;
        final verticalStep = cellH + gap;
        const dragSlices = 20;
        final totalOffset = Offset(0, verticalStep * dragTargetRow);
        final pinCenter = tester.getCenter(find.byType(Pin));

        final TestGesture gesture = await tester.startGesture(pinCenter);
        await gesture.moveBy(const Offset(1, 1));
        await tester.pump(const Duration(milliseconds: 16));

        for (int i = 1; i <= dragSlices; i++) {
          await gesture.moveBy(totalOffset / dragSlices.toDouble());
          await tester.pump(const Duration(milliseconds: 16));
        }

        // Before lifting: grid must have expanded beyond initial rows.
        // This proves that onDragUpdate-driven expansion works even when
        // the pointer moves below the initially rendered cells.
        final midDragGridSize = tester.getSize(
          find.byKey(const Key('board-canvas-absolute-grid')),
        );
        expect(
          midDragGridSize.height,
          greaterThan(initialGridSize.height),
          reason:
              'grid must expand during drag via onDragUpdate before drop occurs',
        );

        await gesture.up();
        await tester.pumpAndSettle();
      },
    );

    testWidgets(
      'BoardCanvas_keeps_expanding_the_bottom_drop_target_during_a_drag',
      (tester) async {
        tester.view.physicalSize = const Size(2200, 2200);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);

        ToolId? gotTool;
        int? gotX;
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
                    onMovePin:
                        ({
                          required boardKey,
                          required toolId,
                          required anchorX,
                          required anchorY,
                        }) {
                          gotTool = toolId;
                          gotX = anchorX;
                          gotY = anchorY;
                        },
                  ),
                ),
              ),
            ),
          ),
        );

        const initialRenderedRows = 2;
        final expectedInitialHeight =
            boardCanvasPinCellHeight * initialRenderedRows +
            UpegSizing.pinGap * (initialRenderedRows - 1);
        final canvasSize = tester.getSize(
          find.byKey(const Key('board-canvas-absolute-grid')),
        );
        expect(
          canvasSize.height,
          expectedInitialHeight,
          reason: 'hover-only expansion must not be pre-rendered permanently',
        );

        // Drag from row 0 down to row 4 in one gesture. Each small move lets the
        // hovered bottom row expand the next buffer before the pointer reaches it.
        final pinFinder = find.byType(Pin);
        final pinCenter = tester.getCenter(pinFinder);
        const verticalStep = boardCanvasPinCellHeight + UpegSizing.pinGap;
        const targetRow = 4;
        const dragSlicesPerRow = 4;

        final TestGesture gesture = await tester.startGesture(pinCenter);
        await gesture.moveBy(const Offset(1, 1));
        await tester.pump(const Duration(milliseconds: 16));

        final targetOffset = Offset(0, verticalStep * targetRow);
        for (int i = 1; i <= targetRow * dragSlicesPerRow; i++) {
          await gesture.moveBy(
            targetOffset / (targetRow * dragSlicesPerRow).toDouble(),
          );
          await tester.pump(const Duration(milliseconds: 16));
        }
        await gesture.up();
        await tester.pump(const Duration(milliseconds: 16));

        expect(gotTool, ToolId.parse('num.hex_to_decimal'));
        expect(gotX, 0);
        expect(gotY, targetRow);
      },
    );

    testWidgets(
      'BoardCanvas_extends_the_drop_target_ahead_of_the_pointer_throughout_a_long_drag',
      (tester) async {
        tester.view.physicalSize = const Size(2200, 2200);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);

        const cellH = boardCanvasPinCellHeight;
        const gap = UpegSizing.pinGap;
        const columns = 6;
        const initialRows = 2;

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
                body: SizedBox(
                  width: 2000,
                  height: 2000,
                  child: debugBoardCanvasGrid(
                    snapshot: const LayoutSnapshotDto(
                      boardKey: 'dev',
                      boardCols: columns,
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

        // Verify initial grid.
        final initialGridSize = tester.getSize(
          find.byKey(const Key('board-canvas-absolute-grid')),
        );
        final initialHeight = cellH * initialRows + gap * (initialRows - 1);
        expect(
          initialGridSize.height,
          initialHeight.toDouble(),
          reason: 'grid must start at exactly initialRows before drag',
        );

        // Start dragging downward.
        final pinCenter = tester.getCenter(find.byType(Pin));
        const targetRow = 14;
        const dragSlices = 14 * 8;
        final totalOffset = Offset(0, (cellH + gap) * targetRow);
        final TestGesture gesture = await tester.startGesture(pinCenter);
        await gesture.moveBy(const Offset(1, 1));
        await tester.pump(const Duration(milliseconds: 16));

        double heightAfterCheckpoint(int row) {
          // Expected grid height when expansion has reached at least [row].
          return (cellH * row + gap * (row - 1)).toDouble();
        }

        for (int i = 1; i <= dragSlices; i++) {
          await gesture.moveBy(totalOffset / dragSlices.toDouble());
          await tester.pump(const Duration(milliseconds: 16));

          // Checkpoint at row 8: grid should already cover row 14+
          // (pointer at row 8 + lookahead 6 = row 14 minimum).
          if (i == 8 * 8) {
            final ckSize = tester.getSize(
              find.byKey(const Key('board-canvas-absolute-grid')),
            );
            expect(
              ckSize.height,
              greaterThanOrEqualTo(heightAfterCheckpoint(14)),
              reason:
                  'at pointer row 8, grid must already cover row 14+ '
                  '(pointer + lookahead expansion)',
            );
            // Guardrail: grid width is exactly 6 columns — no horizontal expansion.
            final expectedWidth =
                UpegSizing.pinCellWidth * columns + gap * (columns - 1);
            expect(
              ckSize.width,
              expectedWidth.toDouble(),
              reason:
                  'grid must remain exactly 6 columns wide '
                  '(no horizontal expansion during vertical drag)',
            );
          }
        }

        // Final check after lifting: grid must cover target row 14+.
        final finalSize = tester.getSize(
          find.byKey(const Key('board-canvas-absolute-grid')),
        );
        expect(
          finalSize.height,
          greaterThanOrEqualTo(heightAfterCheckpoint(14)),
          reason: 'final grid must cover at least targetRow=$targetRow',
        );

        await gesture.up();
        await tester.pumpAndSettle();
      },
    );

    testWidgets(
      'BoardCanvas_keeps_the_drop_target_ahead_of_the_pointer_even_on_a_fast_long_drag',
      (tester) async {
        tester.view.physicalSize = const Size(2200, 2200);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);

        const cellH = boardCanvasPinCellHeight;
        const gap = UpegSizing.pinGap;
        const columns = 6;
        // Mirrors board_canvas.dart:_dragExpansionLookaheadRows.
        const lookaheadRows = 6;

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
                body: SizedBox(
                  width: 2000,
                  height: 2000,
                  child: debugBoardCanvasGrid(
                    snapshot: const LayoutSnapshotDto(
                      boardKey: 'dev',
                      boardCols: columns,
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

        // Verify initial grid.
        final initialGridSize = tester.getSize(
          find.byKey(const Key('board-canvas-absolute-grid')),
        );
        const initialRows = 2;
        expect(
          initialGridSize.height,
          (cellH * initialRows + gap * (initialRows - 1)).toDouble(),
          reason: 'grid must start at exactly initialRows before drag',
        );

        // Rapid drag to row 10 with fewer slices per row than the
        // existing continuous-drag test (3 vs 8) — simulates faster pointer.
        final pinCenter = tester.getCenter(find.byType(Pin));
        const targetRow = 10;
        const dragSlices = 10 * 3;
        final totalOffset = Offset(0, (cellH + gap) * targetRow);

        final TestGesture gesture = await tester.startGesture(pinCenter);
        await gesture.moveBy(const Offset(1, 1));
        await tester.pump(const Duration(milliseconds: 16));

        for (int i = 1; i <= dragSlices; i++) {
          await gesture.moveBy(totalOffset / dragSlices.toDouble());
          await tester.pump(const Duration(milliseconds: 16));

          // Checkpoint at row 6: grid should already cover row 12+
          // (pointer at row 6 + lookahead 6 = row 12 minimum).
          if (i == 6 * 3) {
            final ckSize = tester.getSize(
              find.byKey(const Key('board-canvas-absolute-grid')),
            );
            final expectedHeight =
                (cellH * (6 + lookaheadRows) + gap * (6 + lookaheadRows - 1))
                    .toDouble();
            expect(
              ckSize.height,
              greaterThanOrEqualTo(expectedHeight),
              reason:
                  'at pointer row 6 with rapid drag (3 slices/row), '
                  'grid must already cover row ${6 + lookaheadRows}+ '
                  '(pointer + $lookaheadRows lookahead)',
            );
          }
        }

        // Final check after lifting: grid must cover targetRow.
        final finalSize = tester.getSize(
          find.byKey(const Key('board-canvas-absolute-grid')),
        );
        final expectedFinalHeight = (cellH * targetRow + gap * (targetRow - 1))
            .toDouble();
        expect(
          finalSize.height,
          greaterThanOrEqualTo(expectedFinalHeight),
          reason: 'final grid must cover at least targetRow=$targetRow',
        );

        await gesture.up();
        await tester.pumpAndSettle();
      },
    );

    testWidgets(
      'BoardCanvas_does_not_change_the_placement_when_a_drag_is_cancelled_mid_expansion',
      (tester) async {
        tester.view.physicalSize = const Size(2200, 2200);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);

        const cellH = boardCanvasPinCellHeight;
        const gap = UpegSizing.pinGap;

        ToolId? gotTool;
        int? gotX;
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
                    onMovePin:
                        ({
                          required boardKey,
                          required toolId,
                          required anchorX,
                          required anchorY,
                        }) {
                          gotTool = toolId;
                          gotX = anchorX;
                          gotY = anchorY;
                        },
                  ),
                ),
              ),
            ),
          ),
        );

        // Record baseline: no move should have been recorded yet.
        expect(gotTool, isNull);
        expect(gotX, isNull);
        expect(gotY, isNull);

        // Start dragging downward — trigger expansion.
        final pinCenter = tester.getCenter(find.byType(Pin));
        const dragRows = 6;
        const slices = 12;
        final totalOffset = Offset(0, (cellH + gap) * dragRows);
        final TestGesture gesture = await tester.startGesture(pinCenter);
        await gesture.moveBy(const Offset(1, 1));
        await tester.pump(const Duration(milliseconds: 16));

        // Move down partway to trigger row expansion.
        for (int i = 0; i < slices; i++) {
          await gesture.moveBy(totalOffset / slices.toDouble());
          await tester.pump(const Duration(milliseconds: 16));
        }

        // Verify expansion happened: grid height should be larger than initial.
        final expandedSize = tester.getSize(
          find.byKey(const Key('board-canvas-absolute-grid')),
        );
        final initialHeight = cellH * 2 + gap; // 2 initial rows
        expect(
          expandedSize.height,
          greaterThan(initialHeight.toDouble()),
          reason: 'grid must have expanded during drag',
        );

        // Cancel the drag (simulating onDraggableCanceled — e.g., user pressed Escape
        // or the system cancelled the gesture).
        await gesture.cancel();
        await tester.pumpAndSettle();

        // After cancellation, placement must remain unchanged — no move committed.
        expect(gotTool, isNull, reason: 'drag cancel must not commit a move');
        expect(gotX, isNull, reason: 'drag cancel must not commit a move');
        expect(gotY, isNull, reason: 'drag cancel must not commit a move');

        // Grid expansion can remain (it'll reset on next layout change),
        // but the key assertion is: placement data is untouched.
      },
    );

    testWidgets('BoardCanvas_always_shows_the_pin_move_handle', (tester) async {
      await tester.pumpWidget(
        boardCanvasHarness(
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

      // Modeless: the move handle is always visible — there is no edit
      // toggle to reveal it.
      expect(find.byKey(pinMoveHandleKey), findsOneWidget);
    });

    testWidgets('BoardCanvas_does_not_hide_the_move_handle_in_modeless', (
      tester,
    ) async {
      await tester.pumpWidget(
        boardCanvasHarness(
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

      // No edit-mode gating: the handle never disappears.
      expect(find.byKey(pinMoveHandleKey), findsOneWidget);
    });

    testWidgets('BoardCanvas_moves_a_pin_by_dragging_in_modeless', (
      tester,
    ) async {
      var moved = false;
      await tester.pumpWidget(
        boardCanvasHarness(
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
          onMovePin:
              ({
                required boardKey,
                required toolId,
                required anchorX,
                required anchorY,
              }) {
                moved = true;
              },
        ),
      );

      final pinCenter = tester.getCenter(find.byType(Pin));
      final gesture = await tester.startGesture(pinCenter);
      final step = Offset(UpegSizing.pinCellWidth + UpegSizing.pinGap, 0);
      for (int i = 1; i <= 4; i++) {
        await gesture.moveBy(step / 4);
        await tester.pump(const Duration(milliseconds: 16));
      }
      await gesture.up();
      await tester.pumpAndSettle();

      // Modeless: dragging the pin body always commits a move.
      expect(moved, isTrue);
    });

    testWidgets('BoardCanvas_forwards_a_pin_tap_to_the_parent', (tester) async {
      String? tappedId;
      await tester.pumpWidget(
        boardCanvasHarness(
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
          onPinTap: (p) => tappedId = p.toolId,
        ),
      );

      await tester.tap(find.byType(Pin));
      await tester.pumpAndSettle();

      expect(tappedId, 'num.hex_to_decimal');
    });

    testWidgets('BoardCanvas_runs_a_plain_pin_click_as_onPinTap_in_modeless', (
      tester,
    ) async {
      String? tapped;
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

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
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
                onPinTap: (p) => tapped = p.toolId,
              ),
            ),
          ),
        ),
      );

      await tester.tap(find.byType(Pin));
      await tester.pumpAndSettle();

      // Modeless: tap = run (inline-first activation). There is no
      // edit-mode "tap focuses instead of runs" branch anymore.
      expect(tapped, 'num.hex_to_decimal');
    });
  });
}
