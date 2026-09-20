/// Keyboard dispatch for the pin-resize state machine
/// (mirror of board_page_move_mode_test.dart).
///
/// The pure state machine contract is covered by
/// `state_tests/resize_mode_state_test.dart`. Here we pin the
/// [handleResizeModeCommand] wiring: StartResize seeding (placement
/// effective size + manifest footprint + board cols), ResizeBy
/// application, the Commit setSpan/clearSpan/no-op branches, Cancel
/// recording nothing, and mutual exclusion with move-mode.
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

/// Effective size 2x1 (span override) — a tool whose manifest is u1(1x1).
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
  // Preload the catalog so toolByIdProvider can read the manifest
  // synchronously.
  await c.read(toolsProvider.future);
  c.read(focusedPinProvider.notifier).focus(ToolId.parse('fixture.wide'));
  return c;
}

void main() {
  group('Resize-mode keyboard dispatch', () {
    test(
      'StartResize_seeds_Active_from_the_focused_pin_effective_size_and_manifest',
      () async {
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
      },
    );

    test('StartResize_is_ignored_without_a_focused_pin', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      final handled = handleResizeModeCommand(
        c,
        cmd: const KeyboardCommandDto.startResize(),
      );

      expect(handled, isFalse);
      expect(c.read(resizeModeProvider), isA<ResizeModeIdle>());
    });

    test('ResizeBy_applies_the_delta_while_Active', () async {
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

    test('ResizeBy_is_ignored_while_Idle', () {
      final c = ProviderContainer();
      addTearDown(c.dispose);

      final handled = handleResizeModeCommand(
        c,
        cmd: const KeyboardCommandDto.resizeBy(cols: 1, rows: 0),
      );

      expect(handled, isFalse);
    });

    test('ResetSpan_returns_current_to_the_manifest_size', () async {
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

    test(
      'Commit_sends_a_changed_span_to_the_seam_as_a_setSpan_action',
      () async {
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
      },
    );

    test('Commit_sends_a_clearSpan_action_at_manifest_size', () async {
      ResizeCommitAction? observedAction;
      Future<void> recorder(ToolId toolId, ResizeCommitAction action) async {
        observedAction = action;
      }

      final c = await _readyContainer(
        extra: [resizePinCommitFnProvider.overrideWithValue(recorder)],
      );
      addTearDown(c.dispose);

      handleResizeModeCommand(c, cmd: const KeyboardCommandDto.startResize());
      // 2x1 → 1x1 == manifest footprint: no override is left behind.
      handleResizeModeCommand(
        c,
        cmd: const KeyboardCommandDto.resizeBy(cols: -1, rows: 0),
      );
      handleResizeModeCommand(c, cmd: const KeyboardCommandDto.commit());
      await Future<void>.delayed(Duration.zero);

      expect(observedAction, isA<ResizeCommitClearSpan>());
    });

    test('Commit_skips_the_seam_when_the_base_size_is_unchanged', () async {
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

    test(
      'commit_does_not_save_on_a_board_other_than_the_starting_one',
      () async {
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
      },
    );

    test('Cancel_calls_nothing_and_returns_to_Idle', () async {
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

    test('StartResize_cancels_an_active_move_mode', () async {
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

    test(
      'StartMove_cancels_an_active_resize_mode_and_passes_to_the_move_handler',
      () async {
        final c = await _readyContainer();
        addTearDown(c.dispose);

        handleResizeModeCommand(c, cmd: const KeyboardCommandDto.startResize());
        expect(c.read(resizeModeProvider), isA<ResizeModeActive>());

        // The resize handler does not consume StartMove (=false) — the
        // BoardPage move intercept then starts the move.
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
      },
    );
  });
}
