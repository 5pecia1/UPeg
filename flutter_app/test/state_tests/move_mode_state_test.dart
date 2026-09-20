/// F15 — keyboard move-mode state machine.
///
/// Pressing `m` / `M` (with a focused pin — modeless) flips into a
/// "move mode": arrow keys nudge a pin's anchor, Enter commits, Esc
/// reverts. Flutter implements that contract through
/// `MoveModeNotifier` and the BoardPage keyboard scope.
///
/// This file pins the pure state-machine contract; widget tests cover
/// the BoardPage arrow-key wiring and canvas preview overlay.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/state/move_mode_provider.dart';

const _fixedBoardCols = 6;
const _u1Span = 1;
const _u1MaxStartX = _fixedBoardCols - _u1Span;
final _devBoardKey = BoardKey.parse('dev');

void main() {
  group('MoveModeState machine (F15)', () {
    test('moveModeProvider_initial_value_is_Idle', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      expect(c.read(moveModeProvider), isA<MoveModeIdle>());
    });

    test('start_transitions_from_Idle_to_Active', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      c
          .read(moveModeProvider.notifier)
          .start(
            boardKey: _devBoardKey,
            toolId: ToolId.parse('num.hex_to_decimal'),
            originX: 3,
            originY: 2,
            maxX: _u1MaxStartX,
          );

      final state = c.read(moveModeProvider);
      expect(state, isA<MoveModeActive>());
      final active = state as MoveModeActive;
      expect(active.boardKey, _devBoardKey);
      expect(active.toolId, ToolId.parse('num.hex_to_decimal'));
      expect(active.originX, 3);
      expect(active.originY, 2);
      expect(active.currentX, 3);
      expect(active.currentY, 2);
    });

    test('nudge_updates_only_currentX_currentY_when_Active', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      c
          .read(moveModeProvider.notifier)
          .start(
            boardKey: _devBoardKey,
            toolId: ToolId.parse('id.uuid_v7'),
            originX: 4,
            originY: 1,
            maxX: _u1MaxStartX,
          );
      c.read(moveModeProvider.notifier).nudge(dx: 1, dy: -1);

      final state = c.read(moveModeProvider) as MoveModeActive;
      expect(state.toolId, ToolId.parse('id.uuid_v7'));
      expect(state.originX, 4);
      expect(state.originY, 1);
      expect(state.currentX, 5);
      expect(state.currentY, 0);
    });

    test('nudge_is_no_op_when_Idle', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      c.read(moveModeProvider.notifier).nudge(dx: 3, dy: 3);

      expect(c.read(moveModeProvider), isA<MoveModeIdle>());
    });

    test('cancel_returns_from_Active_to_Idle', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      c
          .read(moveModeProvider.notifier)
          .start(
            boardKey: _devBoardKey,
            toolId: ToolId.parse('memo.scratch'),
            originX: 0,
            originY: 0,
            maxX: _u1MaxStartX,
          );
      c.read(moveModeProvider.notifier).nudge(dx: 5, dy: 5);
      c.read(moveModeProvider.notifier).cancel();

      expect(c.read(moveModeProvider), isA<MoveModeIdle>());
    });

    test('commit_returns_Active_state_while_reverting_to_Idle', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      c
          .read(moveModeProvider.notifier)
          .start(
            boardKey: _devBoardKey,
            toolId: ToolId.parse('fixture.echo'),
            originX: 1,
            originY: 1,
            maxX: _u1MaxStartX,
          );
      c.read(moveModeProvider.notifier).nudge(dx: 2, dy: 0);

      final committed = c.read(moveModeProvider.notifier).commit();
      expect(committed, isNotNull);
      expect(committed!.toolId, ToolId.parse('fixture.echo'));
      expect(committed.currentX, 3);
      expect(committed.currentY, 1);
      expect(c.read(moveModeProvider), isA<MoveModeIdle>());
    });

    test('commit_returns_null_when_Idle', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      final committed = c.read(moveModeProvider.notifier).commit();
      expect(committed, isNull);
    });

    test('nudge_never_goes_below_origin', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      c
          .read(moveModeProvider.notifier)
          .start(
            boardKey: _devBoardKey,
            toolId: ToolId.parse('fixture.echo'),
            originX: 0,
            originY: 0,
            maxX: _u1MaxStartX,
          );
      c.read(moveModeProvider.notifier).nudge(dx: -5, dy: -5);

      final state = c.read(moveModeProvider) as MoveModeActive;
      expect(state.currentX, 0);
      expect(state.currentY, 0);
    });
  });
}
