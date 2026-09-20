/// Pin-resize keyboard/mouse resize-mode state machine.
///
/// Pressing `e` (with a focused pin) — or grabbing the SE-corner
/// handle — flips into "resize mode": arrows/drag adjust the span,
/// Enter/pan-end commits, Esc cancels. This file pins only the pure
/// state-machine contract (mirrors move_mode_state_test.dart); widget
/// tests cover the BoardPage key wiring and canvas preview overlay.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/state/resize_mode_provider.dart';

const _fixedBoardCols = 6;
final _devBoardKey = BoardKey.parse('dev');

void _startU1({
  required ProviderContainer container,
  String toolId = 'num.hex_to_decimal',
  int baseCols = 1,
  int baseRows = 1,
  int manifestCols = 1,
  int manifestRows = 1,
}) {
  container
      .read(resizeModeProvider.notifier)
      .start(
        boardKey: _devBoardKey,
        toolId: ToolId.parse(toolId),
        baseCols: baseCols,
        baseRows: baseRows,
        manifestCols: manifestCols,
        manifestRows: manifestRows,
        maxCols: _fixedBoardCols,
      );
}

void main() {
  group('ResizeModeState machine', () {
    test('resizeModeProvider_initial_value_is_Idle', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      expect(c.read(resizeModeProvider), isA<ResizeModeIdle>());
    });

    test(
      'start_transitions_from_Idle_to_Active_and_seeds_base_into_current',
      () {
        final c = ProviderContainer();
        addTearDown(c.dispose);

        _startU1(container: c, baseCols: 2, baseRows: 1);

        final state = c.read(resizeModeProvider);
        expect(state, isA<ResizeModeActive>());
        final active = state as ResizeModeActive;
        expect(active.boardKey, _devBoardKey);
        expect(active.toolId, ToolId.parse('num.hex_to_decimal'));
        expect(active.baseCols, 2);
        expect(active.baseRows, 1);
        expect(active.currentCols, 2);
        expect(active.currentRows, 1);
        expect(active.maxCols, _fixedBoardCols);
        expect(active.manifestCols, 1);
        expect(active.manifestRows, 1);
      },
    );

    test('resizeBy_updates_only_current_when_Active', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      _startU1(container: c);
      c.read(resizeModeProvider.notifier).resizeBy(dCols: 1, dRows: 2);

      final state = c.read(resizeModeProvider) as ResizeModeActive;
      expect(state.baseCols, 1);
      expect(state.baseRows, 1);
      expect(state.currentCols, 2);
      expect(state.currentRows, 3);
    });

    test('resizeBy_is_no_op_when_Idle', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      c.read(resizeModeProvider.notifier).resizeBy(dCols: 1, dRows: 1);

      expect(c.read(resizeModeProvider), isA<ResizeModeIdle>());
    });

    test('resizeBy_ignores_growth_beyond_boardCols_6', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      _startU1(container: c, baseCols: _fixedBoardCols);
      c.read(resizeModeProvider.notifier).resizeBy(dCols: 1, dRows: 0);

      final state = c.read(resizeModeProvider) as ResizeModeActive;
      expect(state.currentCols, _fixedBoardCols);
    });

    test('resizeBy_ignores_shrink_below_1', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      _startU1(container: c);
      c.read(resizeModeProvider.notifier).resizeBy(dCols: -3, dRows: -3);

      final state = c.read(resizeModeProvider) as ResizeModeActive;
      expect(state.currentCols, 1);
      expect(state.currentRows, 1);
    });

    test('reset_returns_current_to_manifest_size', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      _startU1(
        container: c,
        baseCols: 3,
        baseRows: 2,
        manifestCols: 2,
        manifestRows: 1,
      );
      c.read(resizeModeProvider.notifier).resizeBy(dCols: 1, dRows: 1);
      c.read(resizeModeProvider.notifier).reset();

      final state = c.read(resizeModeProvider) as ResizeModeActive;
      expect(state.currentCols, 2);
      expect(state.currentRows, 1);
      // base keeps the snapshot taken on entry.
      expect(state.baseCols, 3);
      expect(state.baseRows, 2);
    });

    test('cancel_returns_from_Active_to_Idle', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      _startU1(container: c);
      c.read(resizeModeProvider.notifier).resizeBy(dCols: 2, dRows: 2);
      c.read(resizeModeProvider.notifier).cancel();

      expect(c.read(resizeModeProvider), isA<ResizeModeIdle>());
    });

    test('commit_returns_Active_snapshot_while_reverting_to_Idle', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      _startU1(container: c, toolId: 'fixture.echo');
      c.read(resizeModeProvider.notifier).resizeBy(dCols: 2, dRows: 0);

      final committed = c.read(resizeModeProvider.notifier).commit();
      expect(committed, isNotNull);
      expect(committed!.toolId, ToolId.parse('fixture.echo'));
      expect(committed.currentCols, 3);
      expect(committed.currentRows, 1);
      expect(c.read(resizeModeProvider), isA<ResizeModeIdle>());
    });

    test('commit_returns_null_when_Idle', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      expect(c.read(resizeModeProvider.notifier).commit(), isNull);
    });
  });
}
