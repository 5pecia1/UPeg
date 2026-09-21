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
    test('picks_current_board_when_it_has_a_pin', () {
      final target = resolvePaletteJumpBoard(
        currentBoardKey: ops,
        boardOrder: [dev, ops, lab],
        pinnedBoards: {dev, ops},
      );
      expect(target, ops);
    });

    test(
      'picks_first_pinned_board_in_board_order_when_current_board_has_no_pin',
      () {
        final target = resolvePaletteJumpBoard(
          currentBoardKey: dev,
          boardOrder: [dev, ops, lab],
          pinnedBoards: {lab, ops},
        );
        expect(
          target,
          ops,
          reason: 'must follow board-list order (ops precedes lab)',
        );
      },
    );

    test('does_not_jump_when_pinned_nowhere', () {
      final target = resolvePaletteJumpBoard(
        currentBoardKey: dev,
        boardOrder: [dev, ops],
        pinnedBoards: const {},
      );
      expect(target, isNull);
    });

    test('jumps_to_first_pinned_board_even_when_current_board_is_null', () {
      final target = resolvePaletteJumpBoard(
        currentBoardKey: null,
        boardOrder: [dev, ops],
        pinnedBoards: {ops},
      );
      expect(target, ops);
    });

    test('pins_on_boards_missing_from_board_list_are_not_jump_targets', () {
      final target = resolvePaletteJumpBoard(
        currentBoardKey: dev,
        boardOrder: [dev, ops],
        pinnedBoards: {lab},
      );
      expect(
        target,
        isNull,
        reason: 'never jumps to a board that cannot be switched to',
      );
    });
  });
}
