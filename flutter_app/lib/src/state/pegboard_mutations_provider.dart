/// Central pegboard mutation controller.
///
/// Widgets should write pin/layout state through [PegboardMutations] instead
/// of calling FRB directly. This keeps mutation side effects and provider
/// invalidation rules in one place.
library;

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/keyboard.dart' show OrderDirectionDto;
import 'package:upeg/src/rust/api/pegboard.dart' as frb;
import 'package:upeg/src/state/last_outcome_provider.dart';
import 'package:upeg/src/state/layout_provider.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/state/tag_provider.dart';

typedef MovePinMutator =
    void Function(BoardKey boardKey, ToolId toolId, int anchorX, int anchorY);

typedef ReorderPinMutator =
    void Function(
      BoardKey boardKey,
      ToolId toolId,
      OrderDirectionDto direction,
    );

final movePinMutatorProvider = Provider<MovePinMutator>(
  (ref) => (boardKey, toolId, anchorX, anchorY) {
    frb.movePin(
      boardKey: boardKey.value,
      toolId: toolId.value,
      anchorX: anchorX,
      anchorY: anchorY,
    );
  },
);

final reorderPinMutatorProvider = Provider<ReorderPinMutator>(
  (ref) => (boardKey, toolId, direction) {
    frb.reorderPin(
      boardKey: boardKey.value,
      toolId: toolId.value,
      direction: direction,
    );
  },
);

typedef SetPinSpanMutator =
    void Function(BoardKey boardKey, ToolId toolId, int cols, int rows);

typedef ClearPinSpanMutator = void Function(BoardKey boardKey, ToolId toolId);

final setPinSpanMutatorProvider = Provider<SetPinSpanMutator>(
  (ref) => (boardKey, toolId, cols, rows) {
    frb.setPinSpan(
      boardKey: boardKey.value,
      toolId: toolId.value,
      cols: cols,
      rows: rows,
    );
  },
);

final clearPinSpanMutatorProvider = Provider<ClearPinSpanMutator>(
  (ref) => (boardKey, toolId) {
    frb.clearPinSpan(boardKey: boardKey.value, toolId: toolId.value);
  },
);

class PegboardMutations {
  const PegboardMutations(this.ref);

  final Ref ref;

  Future<void> pin(BoardKey boardKey, ToolId toolId) async {
    final mutate = ref.read(pinToolMutatorProvider);
    if (_runMutation(
      'pinTool',
      boardKey,
      toolId,
      () => mutate(boardKey, toolId),
    )) {
      _refreshPin(boardKey, toolId);
    }
  }

  Future<void> unpin(BoardKey boardKey, ToolId toolId) async {
    final mutate = ref.read(unpinToolMutatorProvider);
    if (_runMutation(
      'unpinTool',
      boardKey,
      toolId,
      () => mutate(boardKey, toolId),
    )) {
      // The store deletes the persisted last outcome inside the unpin
      // tombstone transaction; drop the in-memory cache entry too so a
      // re-pin never resurrects a result the store no longer has. A
      // board switch re-hydrates any outcome still persisted elsewhere.
      ref.read(lastOutcomeProvider.notifier).clear(toolId);
      _refreshPin(boardKey, toolId);
    }
  }

  Future<void> move(
    BoardKey boardKey,
    ToolId toolId, {
    required int anchorX,
    required int anchorY,
  }) async {
    final mutate = ref.read(movePinMutatorProvider);
    final ok = _runMutation(
      'movePin',
      boardKey,
      toolId,
      () => mutate(boardKey, toolId, anchorX, anchorY),
    );
    if (ok) {
      bumpLayoutRevisionRef(ref, boardKey);
    }
  }

  Future<void> reorder(
    BoardKey boardKey,
    ToolId toolId,
    OrderDirectionDto direction,
  ) async {
    final mutate = ref.read(reorderPinMutatorProvider);
    final ok = _runMutation(
      'reorderPin',
      boardKey,
      toolId,
      () => mutate(boardKey, toolId, direction),
    );
    if (ok) {
      bumpLayoutRevisionRef(ref, boardKey);
    }
  }

  /// Persist a user span override for `toolId` (pin-resize commit).
  /// The caller (resize state machine) has already clamped `(cols,
  /// rows)` to the legal `ColSpan`/`RowSpan` range.
  Future<void> setSpan(
    BoardKey boardKey,
    ToolId toolId, {
    required int cols,
    required int rows,
  }) async {
    final mutate = ref.read(setPinSpanMutatorProvider);
    if (_runMutation(
      'setPinSpan',
      boardKey,
      toolId,
      () => mutate(boardKey, toolId, cols, rows),
    )) {
      _refreshPin(boardKey, toolId);
    }
  }

  /// Drop the span override so the manifest footprint applies again
  /// (context-menu "reset size" + resize commit at manifest size).
  Future<void> clearSpan(BoardKey boardKey, ToolId toolId) async {
    final mutate = ref.read(clearPinSpanMutatorProvider);
    if (_runMutation(
      'clearPinSpan',
      boardKey,
      toolId,
      () => mutate(boardKey, toolId),
    )) {
      _refreshPin(boardKey, toolId);
    }
  }

  Future<void> setPinColor(
    BoardKey boardKey,
    ToolId toolId, {
    String? color,
  }) async {
    try {
      frb.setPinColor(
        boardKey: boardKey.value,
        toolId: toolId.value,
        color: color,
      );
      _refreshPin(boardKey, toolId);
    } on Object catch (err, stack) {
      debugPrint('upeg: setPinColor failed for $toolId on $boardKey: $err');
      debugPrint(stack.toString());
    }
  }

  /// Runs [mutate], logging and swallowing any FRB error. Returns `true`
  /// when the mutation succeeded so the caller can run its refresh; `false`
  /// (already logged) otherwise. Centralizes the "log + swallow" policy so
  /// it stays identical across pin / unpin / move.
  bool _runMutation(
    String op,
    BoardKey boardKey,
    ToolId toolId,
    VoidCallback mutate,
  ) {
    try {
      mutate();
      return true;
    } on Object catch (err, stack) {
      debugPrint('upeg: $op failed for $toolId on $boardKey: $err');
      debugPrint(stack.toString());
      return false;
    }
  }

  void _refreshPin(BoardKey boardKey, ToolId toolId) {
    ref
      ..invalidate(pinnedProvider((boardKey, toolId)))
      ..invalidate(pinnedBoardsForToolProvider(toolId))
      ..invalidate(tagOptionsForBoardProvider(boardKey));
    bumpLayoutRevisionRef(ref, boardKey);
  }
}

final pegboardMutationsProvider = Provider<PegboardMutations>(
  PegboardMutations.new,
);
