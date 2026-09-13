/// Pin-resize keyboard/mouse resize-mode state machine.
///
/// Pressing `e` (with a focused pin) — or grabbing the SE-corner
/// handle — flips into "resize mode": arrows/드래그가 span을 조절하고,
/// Enter/pan-end가 commit, Esc가 cancel. 이 파일은 순수 state machine
/// 계약만 고정한다 (move_mode_state_test.dart 미러); widget 테스트가
/// BoardPage 키 배선과 canvas preview overlay를 커버한다.
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
    test('resizeModeProvider는_초기값이_Idle이다', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      expect(c.read(resizeModeProvider), isA<ResizeModeIdle>());
    });

    test('start는_Idle에서_Active로_전이하며_base를_current로_seed한다', () {
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
    });

    test('resizeBy는_Active일때_current만_갱신한다', () {
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

    test('resizeBy는_Idle일때_no_op이다', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      c.read(resizeModeProvider.notifier).resizeBy(dCols: 1, dRows: 1);

      expect(c.read(resizeModeProvider), isA<ResizeModeIdle>());
    });

    test('resizeBy는_boardCols_6을_초과하는_확대를_무시한다', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      _startU1(container: c, baseCols: _fixedBoardCols);
      c.read(resizeModeProvider.notifier).resizeBy(dCols: 1, dRows: 0);

      final state = c.read(resizeModeProvider) as ResizeModeActive;
      expect(state.currentCols, _fixedBoardCols);
    });

    test('resizeBy는_1_미만_축소를_무시한다', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      _startU1(container: c);
      c.read(resizeModeProvider.notifier).resizeBy(dCols: -3, dRows: -3);

      final state = c.read(resizeModeProvider) as ResizeModeActive;
      expect(state.currentCols, 1);
      expect(state.currentRows, 1);
    });

    test('reset은_current를_manifest_크기로_되돌린다', () {
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
      // base는 진입 시 snapshot 그대로 남는다.
      expect(state.baseCols, 3);
      expect(state.baseRows, 2);
    });

    test('cancel은_Active에서_Idle로_되돌린다', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      _startU1(container: c);
      c.read(resizeModeProvider.notifier).resizeBy(dCols: 2, dRows: 2);
      c.read(resizeModeProvider.notifier).cancel();

      expect(c.read(resizeModeProvider), isA<ResizeModeIdle>());
    });

    test('commit은_Active_snapshot을_반환하면서_Idle로_되돌린다', () {
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

    test('commit은_Idle일때_null을_반환한다', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      expect(c.read(resizeModeProvider.notifier).commit(), isNull);
    });
  });
}
