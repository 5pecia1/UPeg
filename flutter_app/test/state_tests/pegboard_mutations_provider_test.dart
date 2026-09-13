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
    test('pin_mutation은_layout과_pinned_board_cache를_무효화한다', () async {
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

    test('move_mutation은_현재_board_layout을_무효화한다', () async {
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

    test('move_mutation은_FRB_실패시_layout을_무효화하지_않는다', () async {
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
      'reorder_mutation은_atomic_reorder_mutator를_호출하고_layout을_무효화한다',
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

    test('unpin_mutation은_해당_tool의_last_outcome_캐시를_정리한다', () async {
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

      // 스토어는 tombstone 트랜잭션에서 행을 지우고, 메모리 캐시도 같은
      // 시점에 비워져야 재핀이 사라진 결과를 되살리지 않는다.
      final state = container.read(lastOutcomeProvider);
      expect(state.containsKey(toolId), isFalse);
      expect(state.containsKey(other), isTrue);
    });

    test('setSpan_mutation은_span을_전달하고_layout을_무효화한다', () async {
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

    test('clearSpan_mutation은_override를_지우고_layout을_무효화한다', () async {
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
