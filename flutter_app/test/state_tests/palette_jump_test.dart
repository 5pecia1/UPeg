/// Unit tests for [`resolvePaletteJumpBoard`] — the pure jump-target
/// policy behind the palette ↔ board integration. Priority contract:
/// current board first, then the first pinned board in board-list
/// order, and no jump at all for tools pinned nowhere.
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/state/palette_jump.dart';

void main() {
  final dev = BoardKey.parse('dev');
  final ops = BoardKey.parse('ops');
  final lab = BoardKey.parse('lab');

  group('resolvePaletteJumpBoard', () {
    test('현재_보드에_핀이_있으면_현재_보드를_고른다', () {
      final target = resolvePaletteJumpBoard(
        currentBoardKey: ops,
        boardOrder: [dev, ops, lab],
        pinnedBoards: {dev, ops},
      );
      expect(target, ops);
    });

    test('현재_보드에_핀이_없으면_보드_순서상_첫_번째_핀_보드를_고른다', () {
      final target = resolvePaletteJumpBoard(
        currentBoardKey: dev,
        boardOrder: [dev, ops, lab],
        pinnedBoards: {lab, ops},
      );
      expect(target, ops, reason: '보드 목록 순서(ops가 lab보다 앞)를 따라야 한다');
    });

    test('핀이_어디에도_없으면_점프하지_않는다', () {
      final target = resolvePaletteJumpBoard(
        currentBoardKey: dev,
        boardOrder: [dev, ops],
        pinnedBoards: const {},
      );
      expect(target, isNull);
    });

    test('현재_보드가_null이어도_첫_번째_핀_보드로_점프한다', () {
      final target = resolvePaletteJumpBoard(
        currentBoardKey: null,
        boardOrder: [dev, ops],
        pinnedBoards: {ops},
      );
      expect(target, ops);
    });

    test('보드_목록에_없는_보드의_핀은_점프_대상이_아니다', () {
      final target = resolvePaletteJumpBoard(
        currentBoardKey: dev,
        boardOrder: [dev, ops],
        pinnedBoards: {lab},
      );
      expect(target, isNull, reason: '전환할 수 없는 보드로는 점프하지 않는다');
    });
  });
}
