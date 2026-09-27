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
    void Function(BoardKey boardKey, PinId pinId, int anchorX, int anchorY);

typedef ReorderPinMutator =
    void Function(BoardKey boardKey, PinId pinId, OrderDirectionDto direction);

typedef RemovePinMutator = void Function(BoardKey boardKey, PinId pinId);

final movePinMutatorProvider = Provider<MovePinMutator>(
  (ref) => (boardKey, pinId, anchorX, anchorY) {
    frb.movePin(
      boardKey: boardKey.value,
      pinId: pinId.value,
      anchorX: anchorX,
      anchorY: anchorY,
    );
  },
);

final reorderPinMutatorProvider = Provider<ReorderPinMutator>(
  (ref) => (boardKey, pinId, direction) {
    frb.reorderPin(
      boardKey: boardKey.value,
      pinId: pinId.value,
      direction: direction,
    );
  },
);

final removePinMutatorProvider = Provider<RemovePinMutator>(
  (ref) =>
      (boardKey, pinId) =>
          frb.removePin(boardKey: boardKey.value, pinId: pinId.value),
);

typedef SetPinSpanMutator =
    void Function(BoardKey boardKey, PinId pinId, int cols, int rows);

typedef ClearPinSpanMutator = void Function(BoardKey boardKey, PinId pinId);

final setPinSpanMutatorProvider = Provider<SetPinSpanMutator>(
  (ref) => (boardKey, pinId, cols, rows) {
    frb.setPinSpan(
      boardKey: boardKey.value,
      pinId: pinId.value,
      cols: cols,
      rows: rows,
    );
  },
);

final clearPinSpanMutatorProvider = Provider<ClearPinSpanMutator>(
  (ref) => (boardKey, pinId) {
    frb.clearPinSpan(boardKey: boardKey.value, pinId: pinId.value);
  },
);

class PegboardMutations {
  const PegboardMutations(this.ref);

  final Ref ref;

  Future<PinId?> add(BoardKey boardKey, ToolId toolId) async {
    try {
      final pinId = PinId.parse(
        ref.read(addPinMutatorProvider)(boardKey, toolId),
      );
      _refreshBoard(boardKey);
      return pinId;
    } on Object catch (err, stack) {
      debugPrint('upeg: addPin failed for $toolId on $boardKey: $err');
      debugPrint(stack.toString());
      return null;
    }
  }

  Future<void> pin(BoardKey boardKey, ToolId toolId) async {
    await add(boardKey, toolId);
  }

  /// Legacy tool-wide removal is retained only for obsolete callers. Pin
  /// surfaces use [remove], which targets exactly one placement.
  Future<void> unpin(BoardKey boardKey, ToolId toolId) async {
    try {
      frb.unpinTool(boardKey: boardKey.value, toolId: toolId.value);
      _refreshBoard(boardKey);
    } on Object catch (err, stack) {
      debugPrint('upeg: unpinTool failed for $toolId on $boardKey: $err');
      debugPrint(stack.toString());
    }
  }

  Future<void> remove(PinKey pinKey) async {
    final mutate = ref.read(removePinMutatorProvider);
    if (_runMutation(
      'removePin',
      pinKey.$1,
      pinKey.$2,
      () => mutate(pinKey.$1, pinKey.$2),
    )) {
      // The store deletes the persisted last outcome inside the unpin
      // tombstone transaction; drop the in-memory cache entry too so a
      // re-pin never resurrects a result the store no longer has. A
      // board switch re-hydrates any outcome still persisted elsewhere.
      ref.read(lastOutcomeProvider.notifier).clear(pinKey);
      _refreshBoard(pinKey.$1);
    }
  }

  Future<void> move(
    PinKey pinKey, {
    required int anchorX,
    required int anchorY,
  }) async {
    final mutate = ref.read(movePinMutatorProvider);
    final ok = _runMutation(
      'movePin',
      pinKey.$1,
      pinKey.$2,
      () => mutate(pinKey.$1, pinKey.$2, anchorX, anchorY),
    );
    if (ok) {
      _refreshBoard(pinKey.$1);
    }
  }

  Future<void> reorder(PinKey pinKey, OrderDirectionDto direction) async {
    final mutate = ref.read(reorderPinMutatorProvider);
    final ok = _runMutation(
      'reorderPin',
      pinKey.$1,
      pinKey.$2,
      () => mutate(pinKey.$1, pinKey.$2, direction),
    );
    if (ok) {
      _refreshBoard(pinKey.$1);
    }
  }

  /// Persist a user span override for `toolId` (pin-resize commit).
  /// The caller (resize state machine) has already clamped `(cols,
  /// rows)` to the legal `ColSpan`/`RowSpan` range.
  Future<void> setSpan(
    PinKey pinKey, {
    required int cols,
    required int rows,
  }) async {
    final mutate = ref.read(setPinSpanMutatorProvider);
    if (_runMutation(
      'setPinSpan',
      pinKey.$1,
      pinKey.$2,
      () => mutate(pinKey.$1, pinKey.$2, cols, rows),
    )) {
      _refreshBoard(pinKey.$1);
    }
  }

  /// Drop the span override so the manifest footprint applies again
  /// (context-menu "reset size" + resize commit at manifest size).
  Future<void> clearSpan(PinKey pinKey) async {
    final mutate = ref.read(clearPinSpanMutatorProvider);
    if (_runMutation(
      'clearPinSpan',
      pinKey.$1,
      pinKey.$2,
      () => mutate(pinKey.$1, pinKey.$2),
    )) {
      _refreshBoard(pinKey.$1);
    }
  }

  Future<void> setPinColor(PinKey pinKey, {String? color}) async {
    try {
      frb.setPinColor(
        boardKey: pinKey.$1.value,
        pinId: pinKey.$2.value,
        color: color,
      );
      _refreshBoard(pinKey.$1);
    } on Object catch (err, stack) {
      debugPrint('upeg: setPinColor failed for $pinKey: $err');
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
    PinId pinId,
    VoidCallback mutate,
  ) {
    try {
      mutate();
      return true;
    } on Object catch (err, stack) {
      debugPrint('upeg: $op failed for $pinId on $boardKey: $err');
      debugPrint(stack.toString());
      return false;
    }
  }

  void _refreshBoard(BoardKey boardKey) {
    ref.invalidate(tagOptionsForBoardProvider(boardKey));
    bumpLayoutRevisionRef(ref, boardKey);
  }
}

final pegboardMutationsProvider = Provider<PegboardMutations>(
  PegboardMutations.new,
);
