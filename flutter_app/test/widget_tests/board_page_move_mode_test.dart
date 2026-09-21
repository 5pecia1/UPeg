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
    test(
      'handleMoveModeCommand_enters_Active_on_StartMove_with_a_focused_pin',
      () {
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
      },
    );

    test(
      'handleMoveModeCommand_starts_at_the_current_placement_anchor_on_StartMove',
      () {
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
      },
    );

    test('handleMoveModeCommand_ignores_StartMove_without_a_focused_pin', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      final handled = handleMoveModeCommand(
        c,
        cmd: const KeyboardCommandDto.startMove(),
      );

      expect(handled, isFalse);
      expect(c.read(moveModeProvider), isA<MoveModeIdle>());
    });

    test('handleMoveModeCommand_nudges_on_Arrow_Right_while_Active', () {
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

    test('handleMoveModeCommand_returns_to_Idle_on_Cancel_while_Active', () {
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

    test(
      'handleMoveModeCommand_calls_the_seam_on_Commit_while_Active',
      () async {
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
      },
    );

    test(
      'commit_does_not_save_the_move_on_a_board_other_than_the_starting_one',
      () async {
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
      },
    );

    test('handleMoveModeCommand_ignores_Arrow_while_Idle', () {
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

    test('nudge_does_not_move_past_the_rightmost_column', () {
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

      // At the last U1 anchor in 6 columns (0-5), a right move clamps.
      final handled1 = handleMoveModeCommand(
        c,
        cmd: const KeyboardCommandDto.move(direction: DirectionDto.right),
      );
      expect(handled1, isTrue);
      var state = c.read(moveModeProvider) as MoveModeActive;
      expect(state.currentX, _u1MaxStartX);
    });

    test('nudge_clamps_the_right_edge_of_a_two_cell_pin_to_its_span', () {
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

    test('nudge_does_not_move_into_a_negative_column', () {
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

      // A left move at column 0 clamps to 0.
      final handled1 = handleMoveModeCommand(
        c,
        cmd: const KeyboardCommandDto.move(direction: DirectionDto.left),
      );
      expect(handled1, isTrue);
      final state = c.read(moveModeProvider) as MoveModeActive;
      expect(state.currentX, 0);
    });

    test('move_mode_does_not_move_left_of_column_0', () {
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
