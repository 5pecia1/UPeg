/// Live-pin polling provider.
///
/// For every pin whose `tool.source` is `SourceDto.timer(intervalMs)`
/// this provider runs a `Timer.periodic(interval)` that re-dispatches
/// the tool with empty args and stores the latest [`CanonicalToolResult`]
/// in `state`. Other source variants (`userInput`, `shortcut`, `manual`,
/// `static`) keep the state at [`LiveOutcomePending`] and never start a
/// timer — they are user-driven.
///
/// SoC: this provider owns only the timer + outcome cache. The board
/// canvas Pin widget feeds the cached outcome into the shared canonical
/// presenter for compact label/value rendering.
///
/// Generic over example: the gate is `tool.source is SourceDto_Timer`,
/// not any specific tool id. New tools that declare a Timer source get
/// polling for free.
library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

typedef LiveDispatchFn =
    Future<CanonicalToolResult> Function({
      required ToolId toolId,
      required ToolArgs args,
    });

/// Live pin polls dispatch with the currently-selected board's context
/// (pin args-preset merge + `_upeg.board`), matching the modal bridge.
final liveDispatchToolFnProvider = Provider<LiveDispatchFn>(
  (ref) =>
      ({required ToolId toolId, required ToolArgs args}) => dispatchToolAsync(
        toolId: toolId.value,
        argsJson: args.encodeJson(),
        boardKey: ref.read(currentBoardKeyProvider)?.value,
        // Never approves: polling and pin activation are machine-timed,
        // and an approval barrier is a question for a person. A gated
        // tool reached this way stops at the barrier, which is the
        // honest outcome — the confirmation lives in the run flows
        // (`widgets/approval_confirm_dialog.dart`).
        approve: false,
      ),
);

/// Sealed result of one live polling slot.
///
/// - [`LiveOutcomePending`]: no outcome yet (cold start, or source is
///   non-Timer so polling never fires).
/// - [`LiveOutcomeFresh`]: most recent dispatch succeeded — render the
///   outcome's structured content.
/// - [`LiveOutcomeStale`]: a prior dispatch succeeded but the latest
///   one failed — render `lastOutcome` with a stale indicator.
sealed class LiveOutcomeState {
  const LiveOutcomeState();
  const factory LiveOutcomeState.pending() = LiveOutcomePending;
  const factory LiveOutcomeState.fresh(CanonicalToolResult outcome) =
      LiveOutcomeFresh;
  const factory LiveOutcomeState.stale(
    CanonicalToolResult lastOutcome,
    String errorMessage,
  ) = LiveOutcomeStale;
}

final class LiveOutcomePending extends LiveOutcomeState {
  const LiveOutcomePending();
}

final class LiveOutcomeFresh extends LiveOutcomeState {
  const LiveOutcomeFresh(this.outcome);
  final CanonicalToolResult outcome;
}

final class LiveOutcomeStale extends LiveOutcomeState {
  const LiveOutcomeStale(this.lastOutcome, this.errorMessage);
  final CanonicalToolResult lastOutcome;
  final String errorMessage;
}

/// Provider family keyed on [`ToolId`]. AutoDispose so the timer
/// cancels as soon as the last watcher (typically a board-canvas pin)
/// leaves the tree.
final liveOutcomeProvider = NotifierProvider.autoDispose
    .family<LiveOutcomeNotifier, LiveOutcomeState, ToolId>(
      LiveOutcomeNotifier.new,
    );

class LiveOutcomeNotifier extends Notifier<LiveOutcomeState> {
  LiveOutcomeNotifier(this.toolId);

  final ToolId toolId;
  Timer? _coldStartTimer;
  Timer? _ticker;
  bool _disposed = false;
  bool _inFlight = false;

  @override
  LiveOutcomeState build() {
    _disposed = false;
    _inFlight = false;
    _coldStartTimer?.cancel();
    _ticker?.cancel();

    final tool = ref.read(toolByIdProvider(toolId));
    if (tool == null) return const LiveOutcomeState.pending();

    final interval = _intervalFor(tool.source);
    if (interval == null) return const LiveOutcomeState.pending();

    final dispatch = ref.read(liveDispatchToolFnProvider);

    ref.onDispose(() {
      _disposed = true;
      _coldStartTimer?.cancel();
      _coldStartTimer = null;
      _ticker?.cancel();
      _ticker = null;
    });

    // Schedule the cold-start fire on a cancellable zero-delay timer so
    // subscribers observe `pending` for one frame before the first dispatch
    // result overwrites it, and autoDispose can cancel it before it fires.
    _coldStartTimer = Timer(Duration.zero, () => _fire(dispatch));
    _ticker = Timer.periodic(interval, (_) => _fire(dispatch));
    return const LiveOutcomeState.pending();
  }

  Duration? _intervalFor(SourceDto source) {
    return switch (source) {
      SourceDto_Timer(:final intervalMs) => Duration(
        milliseconds: intervalMs.toInt(),
      ),
      _ => null,
    };
  }

  Future<void> _fire(LiveDispatchFn dispatch) async {
    if (_disposed || _inFlight) return;

    _inFlight = true;
    try {
      final outcome = await dispatch(toolId: toolId, args: ToolArgs.empty);
      if (_disposed) return;

      _applyOutcome(outcome);
    } finally {
      _inFlight = false;
    }
  }

  void _applyOutcome(CanonicalToolResult outcome) {
    if (outcome.ok) {
      state = LiveOutcomeState.fresh(outcome);
      return;
    }
    final last = switch (state) {
      LiveOutcomeFresh(:final outcome) => outcome,
      LiveOutcomeStale(:final lastOutcome) => lastOutcome,
      LiveOutcomePending() => null,
    };
    if (last == null) {
      // First call failed; keep pending until at least one success.
      state = const LiveOutcomeState.pending();
      return;
    }
    state = LiveOutcomeState.stale(
      last,
      outcome.errorMessage ?? 'dispatch failed',
    );
  }
}
