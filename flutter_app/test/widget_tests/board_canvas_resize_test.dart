/// BoardCanvas pin-resize UX: SE-corner handle, 셀 스냅 드래그 미리보기,
/// warn 하이라이트, preview overlay 키, drag 커밋.
///
/// 마우스 드래그와 키보드 `e` 흐름은 같은 [resizeModeProvider] state를
/// 구동한다 — preview 경로가 하나뿐임을 키보드 없이 provider를 직접
/// 뒤집는 케이스로도 고정한다. FRB 호출은 typed seam
/// (`resizePreviewLoaderProvider` / `resizePinCommitFnProvider`)으로
/// 가로채서 dylib 없이 돈다 (board_canvas_drop_highlight_test.dart 미러).
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

/// 한 셀 이동에 해당하는 픽셀 스텝 (셀 + 갭).
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
  placements: [PlacementDto(toolId: 'fixture.a', x: 0, y: 0, w: 1, h: 1)],
);

/// 그대로 통과하는 projection: resize 대상만 요청된 span으로 돌려준다.
List<PlacementDto> _acceptLoader(
  BoardKey boardKey,
  ToolId toolId,
  int cols,
  int rows,
) {
  return [PlacementDto(toolId: toolId.value, x: 0, y: 0, w: cols, h: rows)];
}

/// 핸들을 오른쪽으로 한 셀만큼 드래그하고, 손을 떼지 않은 채 반환한다.
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
  group('BoardCanvas 리사이즈 핸들', () {
    testWidgets('핀은_SE_코너에_리사이즈_핸들을_보여준다', (tester) async {
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
      // 핸들은 핀 rect의 오른쪽 아래 구석에 있다.
      final handleCenter = tester.getCenter(find.byKey(pinResizeHandleKey));
      final pinRect = tester.getRect(find.byType(Pin));
      expect(handleCenter.dx, greaterThan(pinRect.center.dx));
      expect(handleCenter.dy, greaterThan(pinRect.center.dy));
    });

    testWidgets('핸들_드래그는_셀_스냅된_span_미리보기를_보여준다', (tester) async {
      final requestedSpans = <(int, int)>[];
      List<PlacementDto> recordingLoader(
        BoardKey boardKey,
        ToolId toolId,
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

      // 한 셀만큼 드래그 → 2x1로 스냅된 span rect (accent).
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

    testWidgets('다른_pin을_밀어내는_리사이즈는_warn_하이라이트와_push_라벨을_보여준다', (tester) async {
      const snapshot = LayoutSnapshotDto(
        boardKey: 'dev',
        boardCols: _fixedBoardCols,
        placements: [
          PlacementDto(toolId: 'fixture.a', x: 0, y: 0, w: 1, h: 1),
          PlacementDto(toolId: 'fixture.b', x: 1, y: 0, w: 1, h: 1),
        ],
      );
      // fixture.a가 2칸으로 커지면 fixture.b가 (2,0)으로 밀린다.
      List<PlacementDto> pushLoader(
        BoardKey boardKey,
        ToolId toolId,
        int cols,
        int rows,
      ) {
        return [
          PlacementDto(toolId: toolId.value, x: 0, y: 0, w: cols, h: rows),
          const PlacementDto(toolId: 'fixture.b', x: 2, y: 0, w: 1, h: 1),
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
      // 밀려나는 pin은 move-mode와 같은 라벨 overlay로 표시된다.
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
    });

    testWidgets('빈_preview는_리사이즈_overlay를_숨긴다', (tester) async {
      List<PlacementDto> rejectLoader(
        BoardKey boardKey,
        ToolId toolId,
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

    testWidgets('핸들_드래그를_놓으면_commit_seam으로_setSpan을_보낸다', (tester) async {
      ToolId? observedTool;
      ResizeCommitAction? observedAction;
      Future<void> recorder(ToolId toolId, ResizeCommitAction action) async {
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

      expect(observedTool, ToolId.parse('fixture.a'));
      final action = observedAction;
      expect(action, isA<ResizeCommitSetSpan>());
      final setSpan = action as ResizeCommitSetSpan;
      expect(setSpan.cols, 2);
      expect(setSpan.rows, 1);
      // 드래그가 끝나면 preview overlay도 사라진다.
      expect(find.byKey(_resizeHighlightKey), findsNothing);
    });

    testWidgets('움직이지_않은_드래그는_commit을_보내지_않는다', (tester) async {
      var called = false;
      Future<void> recorder(ToolId toolId, ResizeCommitAction action) async {
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

    testWidgets('키보드로_켠_resize_mode도_같은_preview_overlay를_사용한다', (tester) async {
      // 마우스 없이 provider state만 Active로 뒤집는다 — 드래그와 같은
      // 단일 preview 경로임을 고정 (board_canvas_push_preview_test.dart
      // 미러).
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
            toolId: ToolId.parse('fixture.a'),
            baseCols: 1,
            baseRows: 1,
            manifestCols: 1,
            manifestRows: 1,
            maxCols: _fixedBoardCols,
          );
      container.read(resizeModeProvider.notifier).resizeBy(dCols: 1, dRows: 0);
      await tester.pumpAndSettle();

      expect(find.byKey(_resizeHighlightKey), findsOneWidget);

      container.read(resizeModeProvider.notifier).cancel();
      await tester.pumpAndSettle();
      expect(find.byKey(_resizeHighlightKey), findsNothing);
    });

    testWidgets('다른_board의_resize_mode는_preview를_요청하지_않는다', (tester) async {
      var loaderCalled = false;
      List<PlacementDto> recordingLoader(
        BoardKey boardKey,
        ToolId toolId,
        int cols,
        int rows,
      ) {
        loaderCalled = true;
        return _acceptLoader(boardKey, toolId, cols, rows);
      }

      const mediaSnapshot = LayoutSnapshotDto(
        boardKey: 'media',
        boardCols: _fixedBoardCols,
        placements: [PlacementDto(toolId: 'fixture.a', x: 0, y: 0, w: 1, h: 1)],
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
            toolId: ToolId.parse('fixture.a'),
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
