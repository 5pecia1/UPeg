/// Keyboard dispatch for the pin-resize state machine
/// (board_page_move_mode_test.dart 미러).
///
/// 순수 state machine 계약은 `state_tests/resize_mode_state_test.dart`가
/// 커버한다. 여기서는 [handleResizeModeCommand] 배선을 고정한다:
/// StartResize seed(placement 유효 크기 + manifest footprint + board
/// cols), ResizeBy 적용, Commit의 setSpan/clearSpan/no-op 분기,
/// Cancel의 무기록, 그리고 move-mode와의 상호 배타.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/keyboard.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart' show PegboardUnitsDto;
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/focused_pin_provider.dart';
import 'package:upeg/src/state/layout_provider.dart';
import 'package:upeg/src/state/move_mode_provider.dart';
import 'package:upeg/src/state/resize_mode_provider.dart';
import 'package:upeg/src/state/resize_pin_commit_provider.dart';
import 'package:upeg/src/state/tools_provider.dart';

import '../test_helpers/tool_fixture.dart';

class _SeededCurrentBoardNotifier extends CurrentBoardNotifier {
  _SeededCurrentBoardNotifier(this._seed);

  final String _seed;

  @override
  BoardKey? build() => BoardKey.parse(_seed);

  @override
  void select(BoardKey boardKey) {
    state = boardKey;
  }

  @override
  void clear() {
    state = null;
  }
}

const _fixedBoardCols = 6;
final _devBoardKey = BoardKey.parse('dev');
final _mediaBoardKey = BoardKey.parse('media');

/// 유효 크기 2x1 (span override) — manifest는 u1(1x1)인 tool.
const _widePlacement = PlacementDto(
  toolId: 'fixture.wide',
  x: 1,
  y: 2,
  w: 2,
  h: 1,
  spanCols: 2,
  spanRows: 1,
);

ProviderContainer _container({List<Override> extra = const []}) {
  final c = ProviderContainer(
    overrides: [
      currentBoardKeyProvider.overrideWith(
        () => _SeededCurrentBoardNotifier('dev'),
      ),
      layoutLoaderProvider.overrideWithValue(
        (query) => const LayoutSnapshotDto(
          boardKey: 'dev',
          boardCols: _fixedBoardCols,
          placements: [_widePlacement],
        ),
      ),
      toolsLoaderProvider.overrideWith(
        (ref) =>
            () => [
              fixtureToolDto(
                id: 'fixture.wide',
                pegboardUnits: PegboardUnitsDto.u1,
              ),
            ],
      ),
      ...extra,
    ],
  );
  return c;
}

Future<ProviderContainer> _readyContainer({
  List<Override> extra = const [],
}) async {
  final c = _container(extra: extra);
  // toolByIdProvider가 동기적으로 manifest를 읽을 수 있게 catalog를 미리
  // 로드해 둔다.
  await c.read(toolsProvider.future);
  c.read(focusedPinProvider.notifier).focus(ToolId.parse('fixture.wide'));
  return c;
}

void main() {
  group('Resize-mode keyboard dispatch', () {
    test('StartResize는_focused_pin의_유효크기와_manifest로_Active를_seed한다', () async {
      final c = await _readyContainer();
      addTearDown(c.dispose);

      final handled = handleResizeModeCommand(
        c,
        cmd: const KeyboardCommandDto.startResize(),
      );

      expect(handled, isTrue);
      final active = c.read(resizeModeProvider) as ResizeModeActive;
      expect(active.boardKey, _devBoardKey);
      expect(active.toolId, ToolId.parse('fixture.wide'));
      expect(active.baseCols, 2);
      expect(active.baseRows, 1);
      expect(active.currentCols, 2);
      expect(active.currentRows, 1);
      expect(active.maxCols, _fixedBoardCols);
      expect(active.manifestCols, 1);
      expect(active.manifestRows, 1);
    });

    test('StartResize는_focused_pin_없으면_무시한다', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      final handled = handleResizeModeCommand(
        c,
        cmd: const KeyboardCommandDto.startResize(),
      );

      expect(handled, isFalse);
      expect(c.read(resizeModeProvider), isA<ResizeModeIdle>());
    });

    test('ResizeBy는_Active일때_delta를_적용한다', () async {
      final c = await _readyContainer();
      addTearDown(c.dispose);

      handleResizeModeCommand(c, cmd: const KeyboardCommandDto.startResize());
      final handled = handleResizeModeCommand(
        c,
        cmd: const KeyboardCommandDto.resizeBy(cols: 1, rows: 1),
      );

      expect(handled, isTrue);
      final active = c.read(resizeModeProvider) as ResizeModeActive;
      expect(active.currentCols, 3);
      expect(active.currentRows, 2);
    });

    test('ResizeBy는_Idle일때_무시한다', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      final handled = handleResizeModeCommand(
        c,
        cmd: const KeyboardCommandDto.resizeBy(cols: 1, rows: 0),
      );

      expect(handled, isFalse);
    });

    test('ResetSpan은_current를_manifest_크기로_되돌린다', () async {
      final c = await _readyContainer();
      addTearDown(c.dispose);

      handleResizeModeCommand(c, cmd: const KeyboardCommandDto.startResize());
      final handled = handleResizeModeCommand(
        c,
        cmd: const KeyboardCommandDto.resetSpan(),
      );

      expect(handled, isTrue);
      final active = c.read(resizeModeProvider) as ResizeModeActive;
      expect(active.currentCols, 1);
      expect(active.currentRows, 1);
    });

    test('Commit은_변경된_span을_setSpan_액션으로_seam에_보낸다', () async {
      ToolId? observedTool;
      ResizeCommitAction? observedAction;
      Future<void> recorder(ToolId toolId, ResizeCommitAction action) async {
        observedTool = toolId;
        observedAction = action;
      }

      final c = await _readyContainer(
        extra: [resizePinCommitFnProvider.overrideWithValue(recorder)],
      );
      addTearDown(c.dispose);

      handleResizeModeCommand(c, cmd: const KeyboardCommandDto.startResize());
      handleResizeModeCommand(
        c,
        cmd: const KeyboardCommandDto.resizeBy(cols: 1, rows: 0),
      );
      final handled = handleResizeModeCommand(
        c,
        cmd: const KeyboardCommandDto.commit(),
      );

      expect(handled, isTrue);
      await Future<void>.delayed(Duration.zero);

      expect(c.read(resizeModeProvider), isA<ResizeModeIdle>());
      expect(observedTool, ToolId.parse('fixture.wide'));
      final action = observedAction;
      expect(action, isA<ResizeCommitSetSpan>());
      final setSpan = action as ResizeCommitSetSpan;
      expect(setSpan.cols, 3);
      expect(setSpan.rows, 1);
    });

    test('Commit은_manifest_크기면_clearSpan_액션을_보낸다', () async {
      ResizeCommitAction? observedAction;
      Future<void> recorder(ToolId toolId, ResizeCommitAction action) async {
        observedAction = action;
      }

      final c = await _readyContainer(
        extra: [resizePinCommitFnProvider.overrideWithValue(recorder)],
      );
      addTearDown(c.dispose);

      handleResizeModeCommand(c, cmd: const KeyboardCommandDto.startResize());
      // 2x1 → 1x1 == manifest footprint: override를 남기지 않는다.
      handleResizeModeCommand(
        c,
        cmd: const KeyboardCommandDto.resizeBy(cols: -1, rows: 0),
      );
      handleResizeModeCommand(c, cmd: const KeyboardCommandDto.commit());
      await Future<void>.delayed(Duration.zero);

      expect(observedAction, isA<ResizeCommitClearSpan>());
    });

    test('Commit은_base_크기_그대로면_seam을_호출하지_않는다', () async {
      var called = false;
      Future<void> recorder(ToolId toolId, ResizeCommitAction action) async {
        called = true;
      }

      final c = await _readyContainer(
        extra: [resizePinCommitFnProvider.overrideWithValue(recorder)],
      );
      addTearDown(c.dispose);

      handleResizeModeCommand(c, cmd: const KeyboardCommandDto.startResize());
      final handled = handleResizeModeCommand(
        c,
        cmd: const KeyboardCommandDto.commit(),
      );
      await Future<void>.delayed(Duration.zero);

      expect(handled, isTrue);
      expect(c.read(resizeModeProvider), isA<ResizeModeIdle>());
      expect(called, isFalse);
    });

    test('commit은_시작한_board가_아니면_저장하지_않는다', () async {
      var called = false;
      Future<void> recorder(ToolId toolId, ResizeCommitAction action) async {
        called = true;
      }

      final c = await _readyContainer(
        extra: [resizePinCommitFnProvider.overrideWithValue(recorder)],
      );
      addTearDown(c.dispose);

      handleResizeModeCommand(c, cmd: const KeyboardCommandDto.startResize());
      handleResizeModeCommand(
        c,
        cmd: const KeyboardCommandDto.resizeBy(cols: 1, rows: 0),
      );
      c.read(currentBoardKeyProvider.notifier).select(_mediaBoardKey);
      handleResizeModeCommand(c, cmd: const KeyboardCommandDto.commit());
      await Future<void>.delayed(Duration.zero);

      expect(c.read(resizeModeProvider), isA<ResizeModeIdle>());
      expect(called, isFalse);
    });

    test('Cancel은_아무것도_호출하지_않고_Idle로_돌아온다', () async {
      var called = false;
      Future<void> recorder(ToolId toolId, ResizeCommitAction action) async {
        called = true;
      }

      final c = await _readyContainer(
        extra: [resizePinCommitFnProvider.overrideWithValue(recorder)],
      );
      addTearDown(c.dispose);

      handleResizeModeCommand(c, cmd: const KeyboardCommandDto.startResize());
      handleResizeModeCommand(
        c,
        cmd: const KeyboardCommandDto.resizeBy(cols: 2, rows: 1),
      );
      final handled = handleResizeModeCommand(
        c,
        cmd: const KeyboardCommandDto.cancel(),
      );
      await Future<void>.delayed(Duration.zero);

      expect(handled, isTrue);
      expect(c.read(resizeModeProvider), isA<ResizeModeIdle>());
      expect(called, isFalse);
    });

    test('StartResize는_활성_move_mode를_취소한다', () async {
      final c = await _readyContainer();
      addTearDown(c.dispose);

      c
          .read(moveModeProvider.notifier)
          .start(
            boardKey: _devBoardKey,
            toolId: ToolId.parse('fixture.wide'),
            originX: 1,
            originY: 2,
            maxX: _fixedBoardCols - 2,
          );
      expect(c.read(moveModeProvider), isA<MoveModeActive>());

      final handled = handleResizeModeCommand(
        c,
        cmd: const KeyboardCommandDto.startResize(),
      );

      expect(handled, isTrue);
      expect(c.read(moveModeProvider), isA<MoveModeIdle>());
      expect(c.read(resizeModeProvider), isA<ResizeModeActive>());
    });

    test('StartMove는_활성_resize_mode를_취소하고_move_handler에_넘긴다', () async {
      final c = await _readyContainer();
      addTearDown(c.dispose);

      handleResizeModeCommand(c, cmd: const KeyboardCommandDto.startResize());
      expect(c.read(resizeModeProvider), isA<ResizeModeActive>());

      // resize 핸들러는 StartMove를 소비하지 않는다(=false) — BoardPage의
      // move 인터셉트가 이어서 move를 시작한다.
      final handledByResize = handleResizeModeCommand(
        c,
        cmd: const KeyboardCommandDto.startMove(),
      );
      expect(handledByResize, isFalse);
      expect(c.read(resizeModeProvider), isA<ResizeModeIdle>());

      final handledByMove = handleMoveModeCommand(
        c,
        cmd: const KeyboardCommandDto.startMove(),
      );
      expect(handledByMove, isTrue);
      expect(c.read(moveModeProvider), isA<MoveModeActive>());
    });
  });
}
