/// In-flight tool dispatch tracking.
///
/// Holds the set of [`ToolId`]s whose dispatch is currently running so
/// the board Pin can render a per-pin activity indicator (PRD §6.3) and
/// the expanded-modal Run button can gate double-dispatch while a slow
/// tool is still resolving.
///
/// SoC: this provider owns only the running-set membership. The async
/// dispatch itself flows through the existing seams
/// (`dispatchStreamFnProvider` for the modal + inline pin runs,
/// `liveDispatchToolFnProvider` for board activation + live polling);
/// callers hold the lease returned by `begin` and release that exact lease
/// with `end` around their `await`.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/state/pin_provider.dart' show PinKey;

/// Central set of pin placements with a dispatch in flight.
final runningToolsProvider =
    NotifierProvider<RunningToolsNotifier, Set<PinKey>>(
      RunningToolsNotifier.new,
    );

/// Convenience selector: `true` while [toolId] has a dispatch in flight.
/// Lets a Pin watch a single boolean instead of the whole set so it only
/// rebuilds when its own running state flips.
final pinIsRunningProvider = Provider.family<bool, PinKey>(
  (ref, pinKey) => ref.watch(runningToolsProvider).contains(pinKey),
);

/// Opaque ownership token for one in-flight dispatch.
///
/// Tokens use identity equality and can only be created by [begin]. This lets
/// overlapping dispatches of the same tool settle independently without one
/// caller clearing another caller's running state.
final class RunningToolLease {
  RunningToolLease._(this._pinKey);

  final PinKey _pinKey;
}

class RunningToolsNotifier extends Notifier<Set<PinKey>> {
  final Map<PinKey, Set<RunningToolLease>> _activeLeasesByPin =
      <PinKey, Set<RunningToolLease>>{};

  @override
  Set<PinKey> build() {
    _activeLeasesByPin.clear();
    return const <PinKey>{};
  }

  /// Starts one dispatch and returns the lease that owns its running state.
  RunningToolLease begin(PinKey pinKey) {
    final lease = RunningToolLease._(pinKey);
    final activeLeases = _activeLeasesByPin.putIfAbsent(
      pinKey,
      () => <RunningToolLease>{},
    );
    final wasIdle = activeLeases.isEmpty;
    activeLeases.add(lease);
    if (wasIdle) {
      state = Set<PinKey>.unmodifiable(<PinKey>{...state, pinKey});
    }
    return lease;
  }

  /// Ends the dispatch represented by [lease].
  ///
  /// An already-ended lease, or a stale lease invalidated by [clear], is an
  /// unmatched end and intentionally does nothing.
  void end(RunningToolLease lease) {
    final activeLeases = _activeLeasesByPin[lease._pinKey];
    if (activeLeases == null || !activeLeases.remove(lease)) return;
    if (activeLeases.isNotEmpty) return;
    _activeLeasesByPin.remove(lease._pinKey);
    final nextState = <PinKey>{...state}..remove(lease._pinKey);
    state = Set<PinKey>.unmodifiable(nextState);
  }

  /// Clears every running tool and invalidates all outstanding leases.
  ///
  /// A late [end] for an invalidated lease cannot affect a dispatch started
  /// after this reset.
  void clear() {
    _activeLeasesByPin.clear();
    state = const <PinKey>{};
  }
}
