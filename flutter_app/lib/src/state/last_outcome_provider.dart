/// Last non-timer dispatch outcome per board placement.
///
/// Inline-first activation (issue 7): when a runnable pin with no
/// required inputs is activated (`PinActivationDto.dispatchImmediate`),
/// the board fires the dispatch and records the canonical result here.
/// The board-canvas Pin body reads it back and renders the result inline,
/// so a single tap runs the tool and shows its answer in place — no
/// modal, no snackbar-only result.
///
/// Persistence: [LastOutcomeNotifier.record] is write-through — every
/// recorded outcome also lands in the shared native store via the FRB
/// `recordLastOutcome` call (`last_outcomes`, keyed per
/// `(board, pin)`), and boot / board switches hydrate back through
/// `loadLastOutcomes`. Hydrated entries surface as [RestoredOutcome]
/// (with the recording timestamp) so pins can distinguish "ran this
/// session" from "restored from a previous run"; a [FreshOutcome]
/// recorded in this session always wins over a restored row.
///
/// SoC: this provider owns only the outcome cache keyed by [PinKey].
/// `liveOutcomeProvider` still owns the Timer-source polling path; this
/// covers the user-driven (tap / Enter / palette) dispatch path.
library;

import 'dart:async' show scheduleMicrotask;

import 'package:flutter/foundation.dart' show debugPrint, immutable;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/last_outcomes.dart'
    as frb
    show loadLastOutcomes, recordLastOutcome;
import 'package:upeg/src/rust/api/last_outcomes.dart' show LastOutcomeDto;
import 'package:upeg/src/rust/api/tools.dart' show CanonicalToolResult;
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/pin_provider.dart' show PinKey;

/// One cached dispatch outcome, tagged with how it got here.
@immutable
sealed class PinOutcome {
  const PinOutcome({required this.result});

  /// The canonical dispatch result to render.
  final CanonicalToolResult result;
}

/// An outcome produced by a dispatch in this session.
final class FreshOutcome extends PinOutcome {
  const FreshOutcome({required super.result});
}

/// An outcome hydrated from the persisted last-outcome store — the tool
/// has not run in this session; [updatedAt] is when it last did.
final class RestoredOutcome extends PinOutcome {
  const RestoredOutcome({
    required super.result,
    required this.updatedAt,
    required this.truncated,
  });

  /// When the restored result was originally recorded.
  final DateTime updatedAt;

  /// `true` when the store's size cap dropped output entries at record
  /// time — the pin renders an ellipsis on the preview.
  final bool truncated;
}

/// FRB write-through seam — swap in tests to observe/replace persistence.
typedef LastOutcomePersist =
    void Function({
      required String boardKey,
      required String pinId,
      required String toolId,
      required CanonicalToolResult result,
    });

/// FRB hydration seam — swap in tests to feed persisted rows.
typedef LastOutcomeLoad = List<LastOutcomeDto> Function(String boardKey);

final lastOutcomePersistProvider = Provider<LastOutcomePersist>(
  (ref) =>
      ({required boardKey, required pinId, required toolId, required result}) =>
          frb.recordLastOutcome(
            boardKey: boardKey,
            pinId: pinId,
            toolId: toolId,
            result: result,
          ),
);

final lastOutcomeLoadProvider = Provider<LastOutcomeLoad>(
  (ref) =>
      (boardKey) => frb.loadLastOutcomes(boardKey: boardKey),
);

/// Central map of placement identity → most recent user-driven outcome.
final lastOutcomeProvider =
    NotifierProvider<LastOutcomeNotifier, Map<PinKey, PinOutcome>>(
      LastOutcomeNotifier.new,
    );

/// Convenience selector: the last recorded outcome for [pinKey], or
/// `null` when the tool has not been dispatched in this session and no
/// persisted result was restored. Lets a single Pin watch only its own
/// slot instead of the whole map.
final pinLastOutcomeProvider = Provider.family<PinOutcome?, PinKey>((
  ref,
  pinKey,
) {
  return ref.watch(lastOutcomeProvider)[pinKey];
});

class LastOutcomeNotifier extends Notifier<Map<PinKey, PinOutcome>> {
  @override
  Map<PinKey, PinOutcome> build() {
    // Board switches re-hydrate from the store so a pin that never ran
    // in this session still shows its last persisted result.
    ref.listen<BoardKey?>(currentBoardKeyProvider, (previous, next) {
      if (next != null && next != previous) hydrateForBoard(next);
    });
    // Boot: the selection may already be restored when this notifier
    // first builds. State cannot be assigned during build, so the
    // initial hydration is deferred one microtask.
    final initial = ref.read(currentBoardKeyProvider);
    if (initial != null) {
      scheduleMicrotask(() => hydrateForBoard(initial));
    }
    return const <PinKey, PinOutcome>{};
  }

  /// Record the latest [outcome] for one placement as a session-fresh entry
  /// (overwriting any previous entry so the pin always shows the
  /// freshest result) and write it through to the persisted store —
  /// success or failure alike; rendering policy stays with consumers.
  ///
  /// This is the single persistence point for board and host-attach calls.
  void record(PinKey pinKey, ToolId toolId, CanonicalToolResult outcome) {
    state = <PinKey, PinOutcome>{
      ...state,
      pinKey: FreshOutcome(result: outcome),
    };
    try {
      ref.read(lastOutcomePersistProvider)(
        boardKey: pinKey.$1.value,
        pinId: pinKey.$2.value,
        toolId: toolId.value,
        result: outcome,
      );
    } on Object catch (err) {
      // Write-through is best-effort: the in-memory cache already has
      // the result, so a store hiccup must not break the dispatch UX.
      debugPrint('upeg: recordLastOutcome failed: $err');
    }
  }

  /// Drop [pinKey]'s cached outcome (e.g. the pin was removed). The
  /// persisted row is cleared store-side by the unpin tombstone path.
  void clear(PinKey pinKey) {
    if (!state.containsKey(pinKey)) return;
    state = <PinKey, PinOutcome>{...state}..remove(pinKey);
  }

  /// Replace [boardKey]'s restored entries from its complete persisted
  /// snapshot. Entries on other boards remain cached, and session-fresh
  /// entries on every board always win.
  void hydrateForBoard(BoardKey boardKey) {
    final List<LastOutcomeDto> rows;
    try {
      rows = ref.read(lastOutcomeLoadProvider)(boardKey.value);
    } on Object catch (err) {
      // Hydration is a cache warm-up, never a boot blocker.
      debugPrint('upeg: loadLastOutcomes failed: $err');
      return;
    }
    final next = <PinKey, PinOutcome>{
      for (final entry in state.entries)
        if (entry.key.$1 != boardKey || entry.value is FreshOutcome)
          entry.key: entry.value,
    };
    var changed = false;
    for (final row in rows) {
      final pinId = PinId.tryParse(row.pinId);
      if (pinId == null) continue;
      final pinKey = (boardKey, pinId);
      if (next[pinKey] is FreshOutcome) continue;
      next[pinKey] = RestoredOutcome(
        result: row.result,
        updatedAt: DateTime.fromMillisecondsSinceEpoch(row.updatedAtMs.toInt()),
        truncated: row.truncated,
      );
      changed = true;
    }
    if (changed || next.length != state.length) state = next;
  }
}
