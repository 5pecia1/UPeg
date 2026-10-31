/// Live-pin polling provider.
///
/// For every pin whose `tool.source` is `SourceDto.timer(intervalMs)`
/// this provider runs a `Timer.periodic(interval)` that re-dispatches
/// the tool with empty args and stores the latest [`CanonicalToolResult`]
/// in `state`. Other source variants (`userInput`, `shortcut`, `manual`,
/// `static`) keep the state at [`LiveOutcomePending`] and never start a
/// timer — they are user-driven.
///
/// While the window is hidden or minimized ([windowShownProvider]) ticks are
/// skipped, and the first tick after it is shown again fires immediately.
/// Only Timer pins read the window state; user-driven pins never do.
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
import 'package:upeg/src/platform/window_visibility.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/last_outcome_provider.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

typedef LiveDispatchFn =
    Future<CanonicalToolResult> Function({
      PinKey? pinKey,
      required ToolId toolId,
      required ToolArgs args,
    });

/// The instance key isolates lifecycle state; [ToolId] remains the catalog
/// lookup and dispatch target.
typedef LivePinKey = ({PinKey pinKey, ToolId toolId});

/// Live pin polls dispatch with the currently-selected board's context
/// (pin args-preset merge + `_upeg.board`), matching the modal bridge.
final liveDispatchToolFnProvider = Provider<LiveDispatchFn>(
  (ref) =>
      ({PinKey? pinKey, required ToolId toolId, required ToolArgs args}) =>
          ref.read(toolkitDispatchProvider)(
            toolId: toolId,
            args: args,
            boardKey:
                pinKey?.$1.value ?? ref.read(currentBoardKeyProvider)?.value,
            pinId: pinKey?.$2.value,
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
/// - [`LiveOutcomePending`]: no outcome yet (cold start, hidden window, or
///   source is non-Timer so polling never fires).
/// - [`LiveOutcomeFresh`]: most recent dispatch succeeded — render the
///   outcome's structured content.
/// - [`LiveOutcomeStale`]: a prior dispatch succeeded but the latest
///   one failed — render `lastOutcome` with a stale indicator.
/// - [`LiveOutcomeFailed`]: dispatches have failed and none ever
///   succeeded, so there is nothing to render but the failure itself.
sealed class LiveOutcomeState {
  const LiveOutcomeState();
  const factory LiveOutcomeState.pending() = LiveOutcomePending;
  const factory LiveOutcomeState.fresh(CanonicalToolResult outcome) =
      LiveOutcomeFresh;
  const factory LiveOutcomeState.stale(
    CanonicalToolResult lastOutcome,
    String errorMessage,
  ) = LiveOutcomeStale;
  const factory LiveOutcomeState.failed(String errorMessage) =
      LiveOutcomeFailed;
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

final class LiveOutcomeFailed extends LiveOutcomeState {
  const LiveOutcomeFailed(this.errorMessage);
  final String errorMessage;
}

/// Provider family keyed on [`LivePinKey`]. AutoDispose so the timer
/// cancels as soon as the last watcher (typically a board-canvas pin)
/// leaves the tree.
final liveOutcomeProvider = NotifierProvider.autoDispose
    .family<LiveOutcomeNotifier, LiveOutcomeState, LivePinKey>(
      LiveOutcomeNotifier.new,
    );

class LiveOutcomeNotifier extends Notifier<LiveOutcomeState> {
  LiveOutcomeNotifier(this.livePin);

  final LivePinKey livePin;

  PinKey get pinKey => livePin.pinKey;
  ToolId get toolId => livePin.toolId;
  Timer? _coldStartTimer;
  Timer? _ticker;
  bool _disposed = false;
  bool _inFlight = false;
  LastOutcomeNotifier? _lastOutcomes;

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
    final lastOutcomes = ref.read(lastOutcomeProvider.notifier);
    _lastOutcomes = lastOutcomes;
    final saved = ref.read(pinLastOutcomeProvider(pinKey));
    final initialState = saved != null && saved.result.ok
        ? LiveOutcomeState.stale(saved.result, 'saved')
        : const LiveOutcomeState.pending();
    scheduleMicrotask(() {
      if (_disposed || !ref.mounted) return;
      lastOutcomes.ensureHydrated(pinKey.$1);
      final hydrated = ref.read(pinLastOutcomeProvider(pinKey));
      if (state is LiveOutcomePending && hydrated?.result.ok == true) {
        state = LiveOutcomeState.stale(hydrated!.result, 'saved');
      }
    });

    ref.onDispose(() {
      _disposed = true;
      _coldStartTimer?.cancel();
      _coldStartTimer = null;
      _ticker?.cancel();
      _ticker = null;
    });

    // Ticks are skipped while the window is hidden, so catch up as soon as
    // it is shown again. Read only here, past the Timer-source early return:
    // user-driven pins never depend on the window state.
    ref.listen<bool>(windowShownProvider, (previous, shown) {
      if (previous == false && shown) unawaited(_fire(dispatch));
    });

    // Schedule the cold-start fire on a cancellable zero-delay timer so
    // subscribers observe `pending` for one frame before the first dispatch
    // result overwrites it, and autoDispose can cancel it before it fires.
    _coldStartTimer = Timer(Duration.zero, () => _fire(dispatch));
    _ticker = Timer.periodic(interval, (_) => _fire(dispatch));
    return initialState;
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
    // Nothing is on screen to update; the re-show listener in build() fires
    // the catch-up dispatch.
    if (!ref.read(windowShownProvider)) return;

    _inFlight = true;
    final lastOutcomes = _lastOutcomes!;
    final runToken = lastOutcomes.beginRun(pinKey);
    try {
      final outcome = await dispatch(
        pinKey: pinKey,
        toolId: toolId,
        args: ToolArgs.empty,
      );
      if (outcome.ok) {
        if (!lastOutcomes.isLatestRun(pinKey, runToken)) return;
        lastOutcomes.record(
          pinKey,
          toolId,
          outcome,
          args: ToolArgs.empty,
          runToken: runToken,
        );
      }
      if (_disposed ||
          !ref.mounted ||
          !lastOutcomes.isLatestRun(pinKey, runToken)) {
        return;
      }

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
    final errorMessage = outcome.errorMessage ?? 'dispatch failed';
    final last = switch (state) {
      LiveOutcomeFresh(:final outcome) => outcome,
      LiveOutcomeStale(:final lastOutcome) => lastOutcome,
      LiveOutcomePending() || LiveOutcomeFailed() => null,
    };
    if (last == null) {
      // No success to fall back on: surface the failure instead of staying
      // pending until one arrives.
      state = LiveOutcomeState.failed(errorMessage);
      return;
    }
    state = LiveOutcomeState.stale(last, errorMessage);
  }
}
