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

import 'package:upeg/src/identity.dart';

/// Central set of tool ids with a dispatch in flight.
final runningToolsProvider =
    NotifierProvider<RunningToolsNotifier, Set<ToolId>>(
      RunningToolsNotifier.new,
    );

/// Convenience selector: `true` while [toolId] has a dispatch in flight.
/// Lets a Pin watch a single boolean instead of the whole set so it only
/// rebuilds when its own running state flips.
final toolIsRunningProvider = Provider.family<bool, ToolId>(
  (ref, toolId) => ref.watch(runningToolsProvider).contains(toolId),
);

/// Opaque ownership token for one in-flight dispatch.
///
/// Tokens use identity equality and can only be created by [begin]. This lets
/// overlapping dispatches of the same tool settle independently without one
/// caller clearing another caller's running state.
final class RunningToolLease {
  RunningToolLease._(this._toolId);

  final ToolId _toolId;
}

class RunningToolsNotifier extends Notifier<Set<ToolId>> {
  final Map<ToolId, Set<RunningToolLease>> _activeLeasesByTool =
      <ToolId, Set<RunningToolLease>>{};

  @override
  Set<ToolId> build() {
    _activeLeasesByTool.clear();
    return const <ToolId>{};
  }

  /// Starts one dispatch and returns the lease that owns its running state.
  RunningToolLease begin(ToolId toolId) {
    final lease = RunningToolLease._(toolId);
    final activeLeases = _activeLeasesByTool.putIfAbsent(
      toolId,
      () => <RunningToolLease>{},
    );
    final wasIdle = activeLeases.isEmpty;
    activeLeases.add(lease);
    if (wasIdle) {
      state = Set<ToolId>.unmodifiable(<ToolId>{...state, toolId});
    }
    return lease;
  }

  /// Ends the dispatch represented by [lease].
  ///
  /// An already-ended lease, or a stale lease invalidated by [clear], is an
  /// unmatched end and intentionally does nothing.
  void end(RunningToolLease lease) {
    final activeLeases = _activeLeasesByTool[lease._toolId];
    if (activeLeases == null || !activeLeases.remove(lease)) return;
    if (activeLeases.isNotEmpty) return;
    _activeLeasesByTool.remove(lease._toolId);
    final nextState = <ToolId>{...state}..remove(lease._toolId);
    state = Set<ToolId>.unmodifiable(nextState);
  }

  /// Clears every running tool and invalidates all outstanding leases.
  ///
  /// A late [end] for an invalidated lease cannot affect a dispatch started
  /// after this reset.
  void clear() {
    _activeLeasesByTool.clear();
    state = const <ToolId>{};
  }
}
