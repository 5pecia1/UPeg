/// Tests for the central pegboard mutation controller.
///
/// FRB writes are replaced with provider overrides so these tests verify the
/// Flutter provider graph contract without loading the native dylib.
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/keyboard.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart' as rust_tools;
import 'package:upeg/src/state/last_outcome_provider.dart';
import 'package:upeg/src/state/layout_provider.dart';
import 'package:upeg/src/state/pegboard_mutations_provider.dart';
import 'package:upeg/src/state/pin_provider.dart';

void main() {
  group('PegboardMutations', () {
    test('pin_mutation_invalidates_layout_and_pinned_board_cache', () async {
      final boardKey = BoardKey.parse('dev');
      final toolId = ToolId.parse('num.hex_to_decimal');
      var layoutLoads = 0;
      var pinnedLoads = 0;
      var pinnedBoardLoads = 0;
      BoardKey? mutatedBoard;
      ToolId? mutatedTool;

      final container = ProviderContainer(
        overrides: [
          layoutLoaderProvider.overrideWithValue((query) {
            layoutLoads++;
            return LayoutSnapshotDto(
              boardKey: query.boardKey.value,
              boardCols: 6,
              placements: const <PlacementDto>[],
            );
          }),
          isPinnedLoaderProvider.overrideWith(
            (ref) => (_, _) {
              pinnedLoads++;
              return pinnedLoads.isEven;
            },
          ),
          pinnedBoardsLoaderProvider.overrideWith(
            (ref) => (_) {
              pinnedBoardLoads++;
              return pinnedBoardLoads.isEven ? {boardKey} : const <BoardKey>{};
            },
          ),
          pinToolMutatorProvider.overrideWith(
            (ref) => (board, tool) {
              mutatedBoard = board;
              mutatedTool = tool;
            },
          ),
        ],
      );
      addTearDown(container.dispose);

      await container.read(layoutProvider(LayoutQuery.all(boardKey)).future);
      container.read(pinnedProvider((boardKey, toolId)));
      container.read(pinnedBoardsForToolProvider(toolId));
      expect(layoutLoads, 1);
      expect(pinnedLoads, 1);
      expect(pinnedBoardLoads, 1);

      await container.read(pegboardMutationsProvider).pin(boardKey, toolId);

      await container.read(layoutProvider(LayoutQuery.all(boardKey)).future);
      container.read(pinnedProvider((boardKey, toolId)));
      container.read(pinnedBoardsForToolProvider(toolId));

      expect(mutatedBoard, boardKey);
      expect(mutatedTool, toolId);
      expect(layoutLoads, 2);
      expect(pinnedLoads, 2);
      expect(pinnedBoardLoads, 2);
    });

    test('move_mutation_invalidates_current_board_layout', () async {
      final boardKey = BoardKey.parse('dev');
      final toolId = ToolId.parse('num.hex_to_decimal');
      var layoutLoads = 0;
      BoardKey? movedBoard;
      ToolId? movedTool;
      int? movedX;
      int? movedY;

      final container = ProviderContainer(
        overrides: [
          layoutLoaderProvider.overrideWithValue((query) {
            layoutLoads++;
            return LayoutSnapshotDto(
              boardKey: query.boardKey.value,
              boardCols: 6,
              placements: const <PlacementDto>[],
            );
          }),
          movePinMutatorProvider.overrideWith(
            (ref) => (board, tool, anchorX, anchorY) {
              movedBoard = board;
              movedTool = tool;
              movedX = anchorX;
              movedY = anchorY;
            },
          ),
        ],
      );
      addTearDown(container.dispose);

      await container.read(layoutProvider(LayoutQuery.all(boardKey)).future);
      expect(layoutLoads, 1);

      await container
          .read(pegboardMutationsProvider)
          .move(boardKey, toolId, anchorX: 3, anchorY: 2);

      await container.read(layoutProvider(LayoutQuery.all(boardKey)).future);

      expect(movedBoard, boardKey);
      expect(movedTool, toolId);
      expect(movedX, 3);
      expect(movedY, 2);
      expect(layoutLoads, 2);
    });

    test('move_mutation_does_not_invalidate_layout_on_FRB_failure', () async {
      final boardKey = BoardKey.parse('dev');
      final toolId = ToolId.parse('num.hex_to_decimal');
      var layoutLoads = 0;

      final container = ProviderContainer(
        overrides: [
          layoutLoaderProvider.overrideWithValue((query) {
            layoutLoads++;
            return LayoutSnapshotDto(
              boardKey: query.boardKey.value,
              boardCols: 6,
              placements: const <PlacementDto>[],
            );
          }),
          movePinMutatorProvider.overrideWith(
            (ref) =>
                (_, _, _, _) => throw Exception('invalid anchor'),
          ),
        ],
      );
      addTearDown(container.dispose);

      await container.read(layoutProvider(LayoutQuery.all(boardKey)).future);
      expect(layoutLoads, 1);

      await container
          .read(pegboardMutationsProvider)
          .move(boardKey, toolId, anchorX: 999, anchorY: 0);

      await container.read(layoutProvider(LayoutQuery.all(boardKey)).future);

      expect(layoutLoads, 1);
    });

    test(
      'reorder_mutation_calls_atomic_reorder_mutator_and_invalidates_layout',
      () async {
        final boardKey = BoardKey.parse('dev');
        final toolId = ToolId.parse('num.hex_to_decimal');
        var layoutLoads = 0;
        BoardKey? reorderedBoard;
        ToolId? reorderedTool;
        OrderDirectionDto? reorderedDirection;

        final container = ProviderContainer(
          overrides: [
            layoutLoaderProvider.overrideWithValue((query) {
              layoutLoads++;
              return LayoutSnapshotDto(
                boardKey: query.boardKey.value,
                boardCols: 6,
                placements: const <PlacementDto>[],
              );
            }),
            reorderPinMutatorProvider.overrideWith(
              (ref) => (board, tool, direction) {
                reorderedBoard = board;
                reorderedTool = tool;
                reorderedDirection = direction;
              },
            ),
          ],
        );
        addTearDown(container.dispose);

        await container.read(layoutProvider(LayoutQuery.all(boardKey)).future);
        expect(layoutLoads, 1);

        await container
            .read(pegboardMutationsProvider)
            .reorder(boardKey, toolId, OrderDirectionDto.next);

        await container.read(layoutProvider(LayoutQuery.all(boardKey)).future);

        expect(reorderedBoard, boardKey);
        expect(reorderedTool, toolId);
        expect(reorderedDirection, OrderDirectionDto.next);
        expect(layoutLoads, 2);
      },
    );

    test('unpin_mutation_clears_last_outcome_cache_for_that_tool', () async {
      final boardKey = BoardKey.parse('dev');
      final toolId = ToolId.parse('num.hex_to_decimal');
      final other = ToolId.parse('id.uuid_v7');

      final container = ProviderContainer(
        overrides: [
          unpinToolMutatorProvider.overrideWith((ref) => (_, _) {}),
          isPinnedLoaderProvider.overrideWith(
            (ref) =>
                (_, _) => false,
          ),
          pinnedBoardsLoaderProvider.overrideWith(
            (ref) =>
                (_) => const <BoardKey>{},
          ),
          lastOutcomePersistProvider.overrideWithValue(
            ({required boardKey, required toolId, required result}) {},
          ),
          lastOutcomeLoadProvider.overrideWithValue((_) => const []),
        ],
      );
      addTearDown(container.dispose);

      const outcome = rust_tools.CanonicalToolResult(ok: true, outputs: []);
      container.read(lastOutcomeProvider.notifier)
        ..record(toolId, outcome)
        ..record(other, outcome);

      await container.read(pegboardMutationsProvider).unpin(boardKey, toolId);

      // The store deletes the row in the tombstone transaction, and the
      // in-memory cache must be cleared at the same time so a re-pin
      // never resurrects a stale outcome.
      final state = container.read(lastOutcomeProvider);
      expect(state.containsKey(toolId), isFalse);
      expect(state.containsKey(other), isTrue);
    });

    test('setSpan_mutation_forwards_span_and_invalidates_layout', () async {
      final boardKey = BoardKey.parse('dev');
      final toolId = ToolId.parse('num.hex_to_decimal');
      var layoutLoads = 0;
      BoardKey? observedBoard;
      ToolId? observedTool;
      int? observedCols;
      int? observedRows;

      final container = ProviderContainer(
        overrides: [
          layoutLoaderProvider.overrideWithValue((query) {
            layoutLoads++;
            return LayoutSnapshotDto(
              boardKey: query.boardKey.value,
              boardCols: 6,
              placements: const <PlacementDto>[],
            );
          }),
          isPinnedLoaderProvider.overrideWith(
            (ref) =>
                (_, _) => true,
          ),
          pinnedBoardsLoaderProvider.overrideWith(
            (ref) =>
                (_) => const <BoardKey>{},
          ),
          setPinSpanMutatorProvider.overrideWith(
            (ref) => (board, tool, cols, rows) {
              observedBoard = board;
              observedTool = tool;
              observedCols = cols;
              observedRows = rows;
            },
          ),
        ],
      );
      addTearDown(container.dispose);

      await container.read(layoutProvider(LayoutQuery.all(boardKey)).future);
      expect(layoutLoads, 1);

      await container
          .read(pegboardMutationsProvider)
          .setSpan(boardKey, toolId, cols: 3, rows: 2);

      await container.read(layoutProvider(LayoutQuery.all(boardKey)).future);
      expect(observedBoard, boardKey);
      expect(observedTool, toolId);
      expect(observedCols, 3);
      expect(observedRows, 2);
      expect(layoutLoads, 2);
    });

    test('clearSpan_mutation_clears_override_and_invalidates_layout', () async {
      final boardKey = BoardKey.parse('dev');
      final toolId = ToolId.parse('num.hex_to_decimal');
      var layoutLoads = 0;
      BoardKey? observedBoard;
      ToolId? observedTool;

      final container = ProviderContainer(
        overrides: [
          layoutLoaderProvider.overrideWithValue((query) {
            layoutLoads++;
            return LayoutSnapshotDto(
              boardKey: query.boardKey.value,
              boardCols: 6,
              placements: const <PlacementDto>[],
            );
          }),
          isPinnedLoaderProvider.overrideWith(
            (ref) =>
                (_, _) => true,
          ),
          pinnedBoardsLoaderProvider.overrideWith(
            (ref) =>
                (_) => const <BoardKey>{},
          ),
          clearPinSpanMutatorProvider.overrideWith(
            (ref) => (board, tool) {
              observedBoard = board;
              observedTool = tool;
            },
          ),
        ],
      );
      addTearDown(container.dispose);

      await container.read(layoutProvider(LayoutQuery.all(boardKey)).future);
      expect(layoutLoads, 1);

      await container
          .read(pegboardMutationsProvider)
          .clearSpan(boardKey, toolId);

      await container.read(layoutProvider(LayoutQuery.all(boardKey)).future);
      expect(observedBoard, boardKey);
      expect(observedTool, toolId);
      expect(layoutLoads, 2);
    });
  });
}
