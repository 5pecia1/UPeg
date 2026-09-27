/// BoardCanvas pin-resize UX: SE-corner handle, cell-snapped drag previews,
/// warning highlights, preview overlay keys, and drag commits.
///
/// Mouse drags and the keyboard `e` flow drive the same [resizeModeProvider]
/// state. Directly changing the provider without keyboard input also verifies
/// that there is only one preview path. Typed seams
/// (`resizePreviewLoaderProvider` / `resizePinCommitFnProvider`) intercept FRB
/// calls so the tests run without a dylib, mirroring
/// board_canvas_drop_highlight_test.dart.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/resize_mode_provider.dart';
import 'package:upeg/src/state/resize_pin_commit_provider.dart';
import 'package:upeg/src/state/resize_preview_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/board_canvas.dart';
import 'package:upeg/src/widgets/pin.dart';

import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/tool_fixture.dart';

const Key _resizeHighlightKey = Key('board-canvas-resize-highlight');
const Key _resizePreviewKey = Key('board-canvas-resize-preview');

const _fixedBoardCols = 6;
final _devBoardKey = BoardKey.parse('dev');

/// Pixel step for moving one cell (cell + gap).
const double _colStep = UpegSizing.pinCellWidth + UpegSizing.pinGap;

class _SeededCurrentBoard extends CurrentBoardNotifier {
  _SeededCurrentBoard(this._seed);
  final String _seed;
  @override
  BoardKey? build() => BoardKey.parse(_seed);
}

Widget _harness({
  required LayoutSnapshotDto snapshot,
  required ResizePreviewLoader resizePreviewLoader,
  ResizePinCommitFn? commitFn,
}) {
  return ProviderScope(
    overrides: [
      ...i18nTestOverrides,
      currentBoardKeyProvider.overrideWith(() => _SeededCurrentBoard('dev')),
      toolsLoaderProvider.overrideWith(
        (ref) =>
            () => [
              fixtureToolDto(
                id: 'fixture.a',
                pegboardUnits: PegboardUnitsDto.u1,
              ),
              fixtureToolDto(
                id: 'fixture.b',
                pegboardUnits: PegboardUnitsDto.u1,
              ),
            ],
      ),
      resizePreviewLoaderProvider.overrideWithValue(resizePreviewLoader),
      if (commitFn != null)
        resizePinCommitFnProvider.overrideWithValue(commitFn),
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

const _singlePinSnapshot = LayoutSnapshotDto(
  boardKey: 'dev',
  boardCols: _fixedBoardCols,
  placements: [
    PlacementDto(
      toolId: 'fixture.a',
      pinId: 'fixture.a',
      x: 0,
      y: 0,
      w: 1,
      h: 1,
    ),
  ],
);

/// Pass-through projection: returns only the resize target with the requested span.
List<PlacementDto> _acceptLoader(
  BoardKey boardKey,
  PinId toolId,
  int cols,
  int rows,
) {
  return [
    PlacementDto(
      toolId: toolId.value,
      pinId: toolId.value,
      x: 0,
      y: 0,
      w: cols,
      h: rows,
    ),
  ];
}

/// Drags the handle one cell to the right and returns without releasing it.
Future<TestGesture> _dragHandleOneColumnRight(
  WidgetTester tester, {
  Finder? handleFinder,
}) async {
  final center = tester.getCenter(
    handleFinder ?? find.byKey(pinResizeHandleKey),
  );
  final gesture = await tester.startGesture(center);
  const step = Offset(_colStep, 0);
  for (int i = 1; i <= 4; i++) {
    await gesture.moveBy(step / 4);
    await tester.pump(const Duration(milliseconds: 16));
  }
  return gesture;
}

BoxDecoration _highlightDecoration(WidgetTester tester) {
  final decoratedBox = tester.widget<DecoratedBox>(
    find.descendant(
      of: find.byKey(_resizeHighlightKey),
      matching: find.byType(DecoratedBox),
    ),
  );
  return decoratedBox.decoration as BoxDecoration;
}

void main() {
  group('BoardCanvas resize handle', () {
    testWidgets('a_pin_shows_a_resize_handle_in_its_SE_corner', (tester) async {
      await tester.pumpWidget(
        _harness(
          snapshot: _singlePinSnapshot,
          resizePreviewLoader: _acceptLoader,
        ),
      );
      await tester.pumpAndSettle();

      expect(find.byKey(pinResizeHandleKey), findsOneWidget);
      expect(
        find.byTooltip(i18nEn('pin.resize.handle_tooltip')),
        findsOneWidget,
      );
      // The handle sits in the bottom-right corner of the pin rect.
      final handleCenter = tester.getCenter(find.byKey(pinResizeHandleKey));
      final pinRect = tester.getRect(find.byType(Pin));
      expect(handleCenter.dx, greaterThan(pinRect.center.dx));
      expect(handleCenter.dy, greaterThan(pinRect.center.dy));
    });

    testWidgets('dragging_the_handle_shows_a_cell_snapped_span_preview', (
      tester,
    ) async {
      final requestedSpans = <(int, int)>[];
      List<PlacementDto> recordingLoader(
        BoardKey boardKey,
        PinId toolId,
        int cols,
        int rows,
      ) {
        requestedSpans.add((cols, rows));
        return _acceptLoader(boardKey, toolId, cols, rows);
      }

      await tester.pumpWidget(
        _harness(
          snapshot: _singlePinSnapshot,
          resizePreviewLoader: recordingLoader,
        ),
      );
      await tester.pumpAndSettle();

      expect(find.byKey(_resizeHighlightKey), findsNothing);

      final gesture = await _dragHandleOneColumnRight(tester);

      // Dragging one cell produces a span rect snapped to 2x1 (accent).
      expect(find.byKey(_resizeHighlightKey), findsOneWidget);
      expect(requestedSpans.last, (2, 1));
      final positioned = tester.widget<Positioned>(
        find
            .ancestor(
              of: find.byKey(_resizeHighlightKey),
              matching: find.byType(Positioned),
            )
            .first,
      );
      expect(positioned.left, 0.0);
      expect(positioned.top, 0.0);
      expect(positioned.width, 2 * UpegSizing.pinCellWidth + UpegSizing.pinGap);
      expect(positioned.height, boardCanvasPinCellHeight);
      final decoration = _highlightDecoration(tester);
      expect(decoration.color, UpegTokens.dark.accent.withValues(alpha: 0.18));

      await gesture.up();
      await tester.pumpAndSettle();
    });

    testWidgets(
      'a_resize_that_pushes_another_pin_shows_a_warn_highlight_and_push_label',
      (tester) async {
        const snapshot = LayoutSnapshotDto(
          boardKey: 'dev',
          boardCols: _fixedBoardCols,
          placements: [
            PlacementDto(
              toolId: 'fixture.a',
              pinId: 'fixture.a',
              x: 0,
              y: 0,
              w: 1,
              h: 1,
            ),
            PlacementDto(
              toolId: 'fixture.b',
              pinId: 'fixture.b',
              x: 1,
              y: 0,
              w: 1,
              h: 1,
            ),
          ],
        );
        // Expanding fixture.a to two columns pushes fixture.b to (2,0).
        List<PlacementDto> pushLoader(
          BoardKey boardKey,
          PinId toolId,
          int cols,
          int rows,
        ) {
          return [
            PlacementDto(
              toolId: toolId.value,
              pinId: toolId.value,
              x: 0,
              y: 0,
              w: cols,
              h: rows,
            ),
            const PlacementDto(
              toolId: 'fixture.b',
              pinId: 'fixture.b',
              x: 2,
              y: 0,
              w: 1,
              h: 1,
            ),
          ];
        }

        await tester.pumpWidget(
          _harness(snapshot: snapshot, resizePreviewLoader: pushLoader),
        );
        await tester.pumpAndSettle();

        final gesture = await _dragHandleOneColumnRight(
          tester,
          handleFinder: find.byKey(pinResizeHandleKey).first,
        );

        final decoration = _highlightDecoration(tester);
        expect(decoration.color, UpegTokens.dark.warn.withValues(alpha: 0.18));
        final border = decoration.border as Border;
        expect(border.top.color, UpegTokens.dark.warn);
        // A displaced pin uses the same label overlay as move mode.
        expect(find.byKey(_resizePreviewKey), findsOneWidget);
        expect(
          find.descendant(
            of: find.byKey(_resizePreviewKey),
            matching: find.textContaining('fixture.b'),
          ),
          findsWidgets,
        );

        await gesture.up();
        await tester.pumpAndSettle();
      },
    );

    testWidgets('an_empty_preview_hides_the_resize_overlay', (tester) async {
      List<PlacementDto> rejectLoader(
        BoardKey boardKey,
        PinId toolId,
        int cols,
        int rows,
      ) => const <PlacementDto>[];

      await tester.pumpWidget(
        _harness(
          snapshot: _singlePinSnapshot,
          resizePreviewLoader: rejectLoader,
        ),
      );
      await tester.pumpAndSettle();

      final gesture = await _dragHandleOneColumnRight(tester);

      expect(find.byKey(_resizeHighlightKey), findsNothing);
      expect(find.byKey(_resizePreviewKey), findsNothing);

      await gesture.up();
      await tester.pumpAndSettle();
    });

    testWidgets('releasing_a_handle_drag_sends_setSpan_to_the_commit_seam', (
      tester,
    ) async {
      PinId? observedTool;
      ResizeCommitAction? observedAction;
      Future<void> recorder(PinId toolId, ResizeCommitAction action) async {
        observedTool = toolId;
        observedAction = action;
      }

      await tester.pumpWidget(
        _harness(
          snapshot: _singlePinSnapshot,
          resizePreviewLoader: _acceptLoader,
          commitFn: recorder,
        ),
      );
      await tester.pumpAndSettle();

      final gesture = await _dragHandleOneColumnRight(tester);
      await gesture.up();
      await tester.pumpAndSettle();

      expect(observedTool, PinId.parse('fixture.a'));
      final action = observedAction;
      expect(action, isA<ResizeCommitSetSpan>());
      final setSpan = action as ResizeCommitSetSpan;
      expect(setSpan.cols, 2);
      expect(setSpan.rows, 1);
      // The preview overlay disappears when the drag ends.
      expect(find.byKey(_resizeHighlightKey), findsNothing);
    });

    testWidgets('a_drag_without_movement_does_not_commit', (tester) async {
      var called = false;
      Future<void> recorder(PinId toolId, ResizeCommitAction action) async {
        called = true;
      }

      await tester.pumpWidget(
        _harness(
          snapshot: _singlePinSnapshot,
          resizePreviewLoader: _acceptLoader,
          commitFn: recorder,
        ),
      );
      await tester.pumpAndSettle();

      final gesture = await tester.startGesture(
        tester.getCenter(find.byKey(pinResizeHandleKey)),
      );
      await gesture.up();
      await tester.pumpAndSettle();

      expect(called, isFalse);
    });

    testWidgets(
      'keyboard_activated_resize_mode_uses_the_same_preview_overlay',
      (tester) async {
        // Set the provider state to Active without using the mouse. This
        // verifies the same single preview path as dragging, mirroring
        // board_canvas_push_preview_test.dart.
        await tester.pumpWidget(
          _harness(
            snapshot: _singlePinSnapshot,
            resizePreviewLoader: _acceptLoader,
          ),
        );
        await tester.pumpAndSettle();

        expect(find.byKey(_resizeHighlightKey), findsNothing);

        final container = ProviderScope.containerOf(
          tester.element(find.byType(MaterialApp)),
        );
        container
            .read(resizeModeProvider.notifier)
            .start(
              boardKey: _devBoardKey,
              toolId: PinId.parse('fixture.a'),
              baseCols: 1,
              baseRows: 1,
              manifestCols: 1,
              manifestRows: 1,
              maxCols: _fixedBoardCols,
            );
        container
            .read(resizeModeProvider.notifier)
            .resizeBy(dCols: 1, dRows: 0);
        await tester.pumpAndSettle();

        expect(find.byKey(_resizeHighlightKey), findsOneWidget);

        container.read(resizeModeProvider.notifier).cancel();
        await tester.pumpAndSettle();
        expect(find.byKey(_resizeHighlightKey), findsNothing);
      },
    );

    testWidgets('resize_mode_for_another_board_does_not_request_a_preview', (
      tester,
    ) async {
      var loaderCalled = false;
      List<PlacementDto> recordingLoader(
        BoardKey boardKey,
        PinId toolId,
        int cols,
        int rows,
      ) {
        loaderCalled = true;
        return _acceptLoader(boardKey, toolId, cols, rows);
      }

      const mediaSnapshot = LayoutSnapshotDto(
        boardKey: 'media',
        boardCols: _fixedBoardCols,
        placements: [
          PlacementDto(
            toolId: 'fixture.a',
            pinId: 'fixture.a',
            x: 0,
            y: 0,
            w: 1,
            h: 1,
          ),
        ],
      );
      await tester.pumpWidget(
        _harness(snapshot: mediaSnapshot, resizePreviewLoader: recordingLoader),
      );
      await tester.pumpAndSettle();

      final container = ProviderScope.containerOf(
        tester.element(find.byType(MaterialApp)),
      );
      container
          .read(resizeModeProvider.notifier)
          .start(
            boardKey: _devBoardKey,
            toolId: PinId.parse('fixture.a'),
            baseCols: 1,
            baseRows: 1,
            manifestCols: 1,
            manifestRows: 1,
            maxCols: _fixedBoardCols,
          );
      await tester.pumpAndSettle();

      expect(loaderCalled, isFalse);
      expect(find.byKey(_resizeHighlightKey), findsNothing);
    });
  });
}
