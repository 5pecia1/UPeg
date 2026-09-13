/// F16 — push-placement preview overlay on BoardCanvas (Batch T cycle T5).
///
/// While [`moveModeProvider`] is [`MoveModeActive`], the canvas must
/// paint a ghost overlay of every placement the FRB
/// `preview_push(board, tool, x, y)` call returns. The overlay shows
/// the user the projected layout BEFORE they commit with Enter; the
/// snapshot below is the minimal contract — a stable widget key
/// `board-canvas-push-preview` and one label per displaced tool.
///
/// The FRB call is intercepted via the typed `pushPreviewLoaderProvider`
/// seam so the test stays dylib-free.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart' show ToolDto;
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/move_mode_provider.dart';
import 'package:upeg/src/state/push_preview_provider.dart';
import 'package:upeg/src/widgets/board_canvas.dart';

import '../test_helpers/i18n_test_catalog.dart';

const _fixedBoardCols = 6;
const _u1Span = 1;
const _u1MaxStartX = _fixedBoardCols - _u1Span;
final _devBoardKey = BoardKey.parse('dev');

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
      home: Scaffold(
        body: debugBoardCanvasGrid(snapshot: snapshot, onPinTap: (_) {}),
      ),
    ),
  );
}

void main() {
  group('BoardCanvas push preview (F16)', () {
    testWidgets('BoardCanvas_move_mode는_push_preview를_overlay로_표시한다', (
      tester,
    ) async {
      // Seed two pins on the canvas; flip into MoveModeActive with the
      // first tool at a colliding anchor. The push preview loader
      // returns a projected layout where tool_a is displaced — the
      // overlay must mount with the documented key and show at least
      // one label for the displaced tool.
      const snapshot = LayoutSnapshotDto(
        boardKey: 'dev',
        boardCols: _fixedBoardCols,
        placements: [
          PlacementDto(toolId: 'fixture.a', x: 0, y: 0, w: 1, h: 1),
          PlacementDto(toolId: 'fixture.b', x: 1, y: 0, w: 1, h: 1),
        ],
      );

      List<PlacementDto> previewLoader(
        BoardKey boardKey,
        ToolId toolId,
        int x,
        int y,
      ) {
        // The projected layout after dragging fixture.b onto (0,0):
        // fixture.b at (0,0); fixture.a displaced to (1,0).
        return const [
          PlacementDto(toolId: 'fixture.b', x: 0, y: 0, w: 1, h: 1),
          PlacementDto(toolId: 'fixture.a', x: 1, y: 0, w: 1, h: 1),
        ];
      }

      await tester.pumpWidget(
        _harness(snapshot: snapshot, pushPreviewLoader: previewLoader),
      );
      await tester.pumpAndSettle();

      // Initially no preview — the overlay key must not be mounted.
      expect(
        find.byKey(const Key('board-canvas-push-preview')),
        findsNothing,
        reason: 'Push preview must NOT mount while move-mode is Idle',
      );

      // Flip into MoveModeActive with fixture.b targeted at (0,0).
      final container = ProviderScope.containerOf(
        tester.element(find.byType(MaterialApp)),
      );
      container
          .read(moveModeProvider.notifier)
          .start(
            boardKey: _devBoardKey,
            toolId: ToolId.parse('fixture.b'),
            originX: 1,
            originY: 0,
            maxX: _u1MaxStartX,
          );
      // Nudge once so currentX/currentY drift from the origin —
      // currentX becomes 0 (collision).
      container.read(moveModeProvider.notifier).nudge(dx: -1, dy: 0);
      await tester.pumpAndSettle();

      expect(
        find.byKey(const Key('board-canvas-push-preview')),
        findsOneWidget,
        reason:
            'Push preview overlay must mount while MoveModeActive returns non-empty placements',
      );
      // The overlay must label the displaced tool so the user can tell
      // what is moving. The minimal contract is "the displaced tool id
      // appears somewhere under the preview overlay".
      expect(
        find.descendant(
          of: find.byKey(const Key('board-canvas-push-preview')),
          matching: find.textContaining('fixture.a'),
        ),
        findsWidgets,
        reason: 'Push preview overlay must label every displaced tool',
      );
    });

    testWidgets('BoardCanvas는_다른_board의_move_mode_preview를_요청하지_않는다', (
      tester,
    ) async {
      const snapshot = LayoutSnapshotDto(
        boardKey: 'media',
        boardCols: _fixedBoardCols,
        placements: [
          PlacementDto(toolId: 'fixture.media', x: 0, y: 0, w: 1, h: 1),
        ],
      );
      var loaderCalled = false;

      List<PlacementDto> previewLoader(
        BoardKey boardKey,
        ToolId toolId,
        int x,
        int y,
      ) {
        loaderCalled = true;
        return const [
          PlacementDto(toolId: 'fixture.other', x: 1, y: 0, w: 1, h: 1),
        ];
      }

      await tester.pumpWidget(
        _harness(snapshot: snapshot, pushPreviewLoader: previewLoader),
      );
      await tester.pumpAndSettle();

      final container = ProviderScope.containerOf(
        tester.element(find.byType(MaterialApp)),
      );
      container
          .read(moveModeProvider.notifier)
          .start(
            boardKey: _devBoardKey,
            toolId: ToolId.parse('fixture.dragged'),
            originX: 0,
            originY: 0,
            maxX: _u1MaxStartX,
          );
      await tester.pumpAndSettle();

      expect(loaderCalled, isFalse);
      expect(
        find.byKey(const Key('board-canvas-push-preview')),
        findsNothing,
        reason: '다른 board에서 시작한 move preview는 현재 board에 표시하지 않는다',
      );
    });
  });
}
