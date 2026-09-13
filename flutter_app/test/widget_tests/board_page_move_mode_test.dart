/// F15 — UI integration for the move-mode state machine (Q7b).
///
/// The state machine itself is covered by
/// `state_tests/move_mode_state_test.dart` (Q7a). Here we pin the
/// keyboard wiring that flips the BoardPage into and out of move
/// mode + arrow nudges + Enter commit + Esc cancel.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/keyboard.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/focused_pin_provider.dart';
import 'package:upeg/src/state/layout_provider.dart';
import 'package:upeg/src/state/move_mode_provider.dart';
import 'package:upeg/src/state/move_pin_commit_provider.dart';

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
const _u1Span = 1;
const _u2Span = 2;
const _u1MaxStartX = _fixedBoardCols - _u1Span;
const _u2MaxStartX = _fixedBoardCols - _u2Span;
final _devBoardKey = BoardKey.parse('dev');
final _mediaBoardKey = BoardKey.parse('media');

const _focusedPlacement = PlacementDto(
  toolId: 'num.hex_to_decimal',
  x: 4,
  y: 2,
  w: _u1Span,
  h: _u1Span,
);

const _widePlacement = PlacementDto(
  toolId: 'fixture.wide',
  x: _u2MaxStartX,
  y: 0,
  w: _u2Span,
  h: _u1Span,
);

void main() {
  group('Move-mode keyboard dispatch (F15)', () {
    test('handleMoveModeCommand는_StartMove_+_focused_pin이면_Active로_들어간다', () {
      final c = ProviderContainer(
        overrides: [
          currentBoardKeyProvider.overrideWith(
            () => _SeededCurrentBoardNotifier('dev'),
          ),
          layoutLoaderProvider.overrideWithValue(
            (boardKey) => const LayoutSnapshotDto(
              boardKey: 'dev',
              boardCols: _fixedBoardCols,
              placements: [_focusedPlacement],
            ),
          ),
        ],
      );
      addTearDown(c.dispose);

      c
          .read(focusedPinProvider.notifier)
          .focus(ToolId.parse('num.hex_to_decimal'));
      final handled = handleMoveModeCommand(
        c,
        cmd: const KeyboardCommandDto.startMove(),
      );

      expect(handled, isTrue);
      expect(c.read(moveModeProvider), isA<MoveModeActive>());
    });

    test('handleMoveModeCommand는_StartMove_시_현재_placement_anchor에서_시작한다', () {
      final c = ProviderContainer(
        overrides: [
          currentBoardKeyProvider.overrideWith(
            () => _SeededCurrentBoardNotifier('dev'),
          ),
          layoutLoaderProvider.overrideWithValue(
            (boardKey) => const LayoutSnapshotDto(
              boardKey: 'dev',
              boardCols: _fixedBoardCols,
              placements: [_focusedPlacement],
            ),
          ),
        ],
      );
      addTearDown(c.dispose);

      c
          .read(focusedPinProvider.notifier)
          .focus(ToolId.parse('num.hex_to_decimal'));
      final handled = handleMoveModeCommand(
        c,
        cmd: const KeyboardCommandDto.startMove(),
      );

      expect(handled, isTrue);
      final active = c.read(moveModeProvider) as MoveModeActive;
      expect(active.originX, 4);
      expect(active.originY, 2);
      expect(active.currentX, 4);
      expect(active.currentY, 2);
    });

    test('handleMoveModeCommand는_focused_pin_없으면_StartMove를_무시한다', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      final handled = handleMoveModeCommand(
        c,
        cmd: const KeyboardCommandDto.startMove(),
      );

      expect(handled, isFalse);
      expect(c.read(moveModeProvider), isA<MoveModeIdle>());
    });

    test('handleMoveModeCommand는_Active일때_Arrow_Right로_nudge한다', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      c.read(focusedPinProvider.notifier).focus(ToolId.parse('id.uuid_v7'));
      c
          .read(moveModeProvider.notifier)
          .start(
            boardKey: _devBoardKey,
            toolId: ToolId.parse('id.uuid_v7'),
            originX: 0,
            originY: 0,
            maxX: _u1MaxStartX,
          );

      final handled = handleMoveModeCommand(
        c,
        cmd: const KeyboardCommandDto.move(direction: DirectionDto.right),
      );

      expect(handled, isTrue);
      final state = c.read(moveModeProvider) as MoveModeActive;
      expect(state.currentX, 1);
      expect(state.currentY, 0);
    });

    test('handleMoveModeCommand는_Active일때_Cancel로_Idle로_돌아온다', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      c.read(focusedPinProvider.notifier).focus(ToolId.parse('memo.scratch'));
      c
          .read(moveModeProvider.notifier)
          .start(
            boardKey: _devBoardKey,
            toolId: ToolId.parse('memo.scratch'),
            originX: 1,
            originY: 1,
            maxX: _u1MaxStartX,
          );

      final handled = handleMoveModeCommand(
        c,
        cmd: const KeyboardCommandDto.cancel(),
      );

      expect(handled, isTrue);
      expect(c.read(moveModeProvider), isA<MoveModeIdle>());
    });

    test('handleMoveModeCommand는_Active일때_Commit으로_seam을_호출한다', () async {
      ToolId? observedTool;
      int? observedX;
      int? observedY;
      Future<void> recorder(ToolId toolId, int x, int y) async {
        observedTool = toolId;
        observedX = x;
        observedY = y;
      }

      final c = ProviderContainer(
        overrides: [
          currentBoardKeyProvider.overrideWith(
            () => _SeededCurrentBoardNotifier('dev'),
          ),
          movePinCommitFnProvider.overrideWithValue(recorder),
        ],
      );
      addTearDown(c.dispose);

      c.read(focusedPinProvider.notifier).focus(ToolId.parse('fixture.echo'));
      c
          .read(moveModeProvider.notifier)
          .start(
            boardKey: _devBoardKey,
            toolId: ToolId.parse('fixture.echo'),
            originX: 2,
            originY: 1,
            maxX: _u1MaxStartX,
          );
      c.read(moveModeProvider.notifier).nudge(dx: 1, dy: 0);

      final handled = handleMoveModeCommand(
        c,
        cmd: const KeyboardCommandDto.commit(),
      );

      expect(handled, isTrue);
      // commit is sync flip-to-Idle + fire-and-forget FRB call.
      await Future<void>.delayed(Duration.zero);

      expect(c.read(moveModeProvider), isA<MoveModeIdle>());
      expect(observedTool, ToolId.parse('fixture.echo'));
      expect(observedX, 3);
      expect(observedY, 1);
    });

    test('commit은_시작한_board가_아니면_이동을_저장하지_않는다', () async {
      ToolId? observedTool;
      Future<void> recorder(ToolId toolId, int x, int y) async {
        observedTool = toolId;
      }

      final c = ProviderContainer(
        overrides: [
          currentBoardKeyProvider.overrideWith(
            () => _SeededCurrentBoardNotifier('dev'),
          ),
          layoutLoaderProvider.overrideWithValue(
            (boardKey) => const LayoutSnapshotDto(
              boardKey: 'dev',
              boardCols: _fixedBoardCols,
              placements: [_focusedPlacement],
            ),
          ),
          movePinCommitFnProvider.overrideWithValue(recorder),
        ],
      );
      addTearDown(c.dispose);

      c
          .read(focusedPinProvider.notifier)
          .focus(ToolId.parse('num.hex_to_decimal'));
      expect(
        handleMoveModeCommand(c, cmd: const KeyboardCommandDto.startMove()),
        isTrue,
      );
      c.read(currentBoardKeyProvider.notifier).select(_mediaBoardKey);

      expect(
        handleMoveModeCommand(c, cmd: const KeyboardCommandDto.commit()),
        isTrue,
      );
      await Future<void>.delayed(Duration.zero);

      expect(c.read(moveModeProvider), isA<MoveModeIdle>());
      expect(observedTool, isNull);
    });

    test('handleMoveModeCommand는_Idle일때_Arrow를_무시한다', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      final handled = handleMoveModeCommand(
        c,
        cmd: const KeyboardCommandDto.move(direction: DirectionDto.left),
      );

      // Idle → handler does NOT consume arrow events so the BoardPage
      // dispatcher keeps falling through to its other handlers.
      expect(handled, isFalse);
    });

    test('nudge는_오른쪽_끝_열에서_더_이상_오른쪽으로_이동하지_않는다', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      c
          .read(focusedPinProvider.notifier)
          .focus(ToolId.parse('num.hex_to_decimal'));
      c
          .read(moveModeProvider.notifier)
          .start(
            boardKey: _devBoardKey,
            toolId: ToolId.parse('num.hex_to_decimal'),
            originX: _u1MaxStartX,
            originY: 0,
            maxX: _u1MaxStartX,
          );

      // 6열(0-5)에서 U1의 마지막 anchor에 있고, 오른쪽 이동 시도 → clamp.
      final handled1 = handleMoveModeCommand(
        c,
        cmd: const KeyboardCommandDto.move(direction: DirectionDto.right),
      );
      expect(handled1, isTrue);
      var state = c.read(moveModeProvider) as MoveModeActive;
      expect(state.currentX, _u1MaxStartX);
    });

    test('nudge는_2칸_pin의_오른쪽_끝을_span에_맞춰_고정한다', () {
      final c = ProviderContainer(
        overrides: [
          currentBoardKeyProvider.overrideWith(
            () => _SeededCurrentBoardNotifier('dev'),
          ),
          layoutLoaderProvider.overrideWithValue(
            (boardKey) => const LayoutSnapshotDto(
              boardKey: 'dev',
              boardCols: _fixedBoardCols,
              placements: [_widePlacement],
            ),
          ),
        ],
      );
      addTearDown(c.dispose);

      c.read(focusedPinProvider.notifier).focus(ToolId.parse('fixture.wide'));
      final started = handleMoveModeCommand(
        c,
        cmd: const KeyboardCommandDto.startMove(),
      );
      expect(started, isTrue);

      var state = c.read(moveModeProvider) as MoveModeActive;
      expect(state.currentX, _u2MaxStartX);
      expect(state.maxX, _u2MaxStartX);

      final nudged = handleMoveModeCommand(
        c,
        cmd: const KeyboardCommandDto.move(direction: DirectionDto.right),
      );
      expect(nudged, isTrue);
      state = c.read(moveModeProvider) as MoveModeActive;
      expect(state.currentX, _u2MaxStartX);
    });

    test('nudge는_음수_열로_이동하지_않는다', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      c
          .read(focusedPinProvider.notifier)
          .focus(ToolId.parse('num.hex_to_decimal'));
      c
          .read(moveModeProvider.notifier)
          .start(
            boardKey: _devBoardKey,
            toolId: ToolId.parse('num.hex_to_decimal'),
            originX: 0,
            originY: 0,
            maxX: _u1MaxStartX,
          );

      // column 0에서 왼쪽으로 이동 시도 → clamp to 0.
      final handled1 = handleMoveModeCommand(
        c,
        cmd: const KeyboardCommandDto.move(direction: DirectionDto.left),
      );
      expect(handled1, isTrue);
      final state = c.read(moveModeProvider) as MoveModeActive;
      expect(state.currentX, 0);
    });

    test('move_mode는_0열_왼쪽으로_이동하지_않는다', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      c.read(focusedPinProvider.notifier).focus(ToolId.parse('id.uuid_v7'));
      c
          .read(moveModeProvider.notifier)
          .start(
            boardKey: _devBoardKey,
            toolId: ToolId.parse('id.uuid_v7'),
            originX: 2,
            originY: 0,
            maxX: _u1MaxStartX,
          );

      // Nudge right to 3, then left back to 0.
      handleMoveModeCommand(
        c,
        cmd: const KeyboardCommandDto.move(direction: DirectionDto.right),
      );
      expect((c.read(moveModeProvider) as MoveModeActive).currentX, 3);

      // Three left moves → 2, 1, 0.
      for (int i = 0; i < 3; i++) {
        handleMoveModeCommand(
          c,
          cmd: const KeyboardCommandDto.move(direction: DirectionDto.left),
        );
      }
      var state = c.read(moveModeProvider) as MoveModeActive;
      expect(state.currentX, 0);

      // One more left → stays at 0 (lower-bound clamp).
      handleMoveModeCommand(
        c,
        cmd: const KeyboardCommandDto.move(direction: DirectionDto.left),
      );
      state = c.read(moveModeProvider) as MoveModeActive;
      expect(state.currentX, 0);
    });
  });
}
