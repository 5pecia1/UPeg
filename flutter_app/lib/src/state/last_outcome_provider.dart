/// Saved dispatch outcomes per board placement, including live polling.
///
/// Inline-first activation (issue 7): when a runnable pin with no
/// required inputs is activated (`PinActivationDto.dispatchImmediate`),
/// the board fires the dispatch and records the canonical result here.
/// The board-canvas Pin body reads it back and renders the result inline,
/// so a single tap runs the tool and shows its answer in place — no
/// modal, no snackbar-only result.
///
/// Persistence: [LastOutcomeNotifier.record] attempts to write each accepted
/// outcome to native SQLite or browser storage via the FRB `recordLastOutcome`
/// call (`last_outcomes`, keyed per `(board, pin)`). Failed writes remain
/// visible as [FreshOutcome] with `persisted = false` and can be retried.
/// Boot / board switches hydrate back through
/// `loadLastOutcomes`. Hydrated entries surface as [RestoredOutcome]
/// (with the recording timestamp) so pins can distinguish "ran this
/// session" from "restored from a previous run"; a [FreshOutcome]
/// recorded in this session always wins over a restored row.
///
/// SoC: this provider owns only the outcome cache keyed by [PinKey].
/// `liveOutcomeProvider` owns Timer-source polling and records successful
/// polls here alongside user-driven (tap / Enter / palette) results.
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
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

/// One cached dispatch outcome, tagged with how it got here.
@immutable
sealed class PinOutcome {
  const PinOutcome({required this.result, this.args});

  /// The canonical dispatch result to render.
  final CanonicalToolResult result;

  /// Inputs captured when this result ran, never the currently edited draft.
  final ToolArgs? args;
}

/// An outcome produced by a dispatch in this session.
final class FreshOutcome extends PinOutcome {
  const FreshOutcome({
    required super.result,
    super.args,
    this.persisted = true,
  });

  final bool persisted;
}

/// An outcome hydrated from the persisted last-outcome store — the tool
/// has not run in this session; [updatedAt] is when its row was saved.
final class RestoredOutcome extends PinOutcome {
  const RestoredOutcome({
    required super.result,
    super.args,
    required this.updatedAt,
    required this.truncated,
  });

  /// When the restored result was originally recorded.
  final DateTime updatedAt;

  /// `true` for a legacy row shortened by the former store size cap.
  /// New writes retain complete outputs; restored partial previews stay marked.
  final bool truncated;
}

/// FRB write-through seam — swap in tests to observe/replace persistence.
typedef LastOutcomePersist =
    void Function({
      required String boardKey,
      required String pinId,
      required String toolId,
      required CanonicalToolResult result,
      String? argsJson,
    });

/// FRB hydration seam — swap in tests to feed persisted rows.
typedef LastOutcomeLoad = List<LastOutcomeDto> Function(String boardKey);

final lastOutcomePersistProvider = Provider<LastOutcomePersist>(
  (ref) =>
      ({
        required boardKey,
        required pinId,
        required toolId,
        required result,
        argsJson,
      }) => frb.recordLastOutcome(
        boardKey: boardKey,
        pinId: pinId,
        toolId: toolId,
        result: result,
        argsJson: argsJson,
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
  final Map<PinKey, int> _runs = <PinKey, int>{};
  final Set<BoardKey> _loadFailures = <BoardKey>{};
  final Set<BoardKey> _hydratedBoards = <BoardKey>{};
  int _nextRun = 0;

  /// Shared across inline and modal surfaces; an older completion must not
  /// overwrite the result of a newer invocation of the same placement.
  int beginRun(PinKey pinKey) => _runs[pinKey] = ++_nextRun;

  bool isLatestRun(PinKey pinKey, int token) => _runs[pinKey] == token;

  bool hasLoadFailure(BoardKey board) => _loadFailures.contains(board);

  void ensureHydrated(BoardKey board) {
    if (!_hydratedBoards.contains(board)) hydrateForBoard(board);
  }

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
  /// Shared persistence point for board, modal, live, and host-attach results.
  bool record(
    PinKey pinKey,
    ToolId toolId,
    CanonicalToolResult outcome, {
    ToolArgs? args,
    int? runToken,
  }) {
    if (!ref.mounted || (runToken != null && _runs[pinKey] != runToken)) {
      return false;
    }
    var persisted = true;
    try {
      ref.read(lastOutcomePersistProvider)(
        boardKey: pinKey.$1.value,
        pinId: pinKey.$2.value,
        toolId: toolId.value,
        result: outcome,
        argsJson: args?.encodeJson(),
      );
    } on Object catch (err) {
      // Write-through is best-effort: the in-memory cache already has
      // the result, so a store hiccup must not break the dispatch UX.
      debugPrint('upeg: recordLastOutcome failed: $err');
      persisted = false;
    }
    state = <PinKey, PinOutcome>{
      ...state,
      pinKey: FreshOutcome(result: outcome, args: args, persisted: persisted),
    };
    return persisted;
  }

  void retry(PinKey pinKey, ToolId toolId) {
    final previous = state[pinKey];
    if (previous == null) return;
    record(pinKey, toolId, previous.result, args: previous.args);
  }

  /// Drop [pinKey]'s cached outcome (e.g. the pin was removed). The
  /// persisted row is cleared store-side by the unpin tombstone path.
  void clear(PinKey pinKey) {
    _runs.remove(pinKey);
    if (!state.containsKey(pinKey)) return;
    state = <PinKey, PinOutcome>{...state}..remove(pinKey);
  }

  void clearBoard(BoardKey boardKey) {
    _runs.removeWhere((key, _) => key.$1 == boardKey);
    _loadFailures.remove(boardKey);
    _hydratedBoards.remove(boardKey);
    state = <PinKey, PinOutcome>{
      for (final entry in state.entries)
        if (entry.key.$1 != boardKey) entry.key: entry.value,
    };
  }

  void resetMemory() {
    _runs.clear();
    _loadFailures.clear();
    _hydratedBoards.clear();
    state = const <PinKey, PinOutcome>{};
  }

  /// Replace [boardKey]'s restored entries from its complete persisted
  /// snapshot. Entries on other boards remain cached, and session-fresh
  /// entries on every board always win.
  void hydrateForBoard(BoardKey boardKey) {
    final List<LastOutcomeDto> rows;
    try {
      rows = ref.read(lastOutcomeLoadProvider)(boardKey.value);
    } on Object catch (err) {
      _loadFailures.add(boardKey);
      state = {...state};
      debugPrint('upeg: loadLastOutcomes failed: $err');
      return;
    }
    final recovered = _loadFailures.remove(boardKey);
    _hydratedBoards.add(boardKey);
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
        args: row.argsJson == null
            ? null
            : ToolArgs.tryDecodeObject(row.argsJson!),
        updatedAt: DateTime.fromMillisecondsSinceEpoch(row.updatedAtMs.toInt()),
        truncated: row.truncated,
      );
      changed = true;
    }
    if (recovered || changed || next.length != state.length) state = next;
  }
}
