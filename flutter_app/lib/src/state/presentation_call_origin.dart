/// Ephemeral context for presentation follow-up calls.
///
/// This deliberately does not use the persisted last-outcome cache. A row or
/// result action is only trustworthy while it is backed by the exact call
/// which produced it: its inputs, host and completed-run generation.
library;

import 'package:flutter/foundation.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

@immutable
final class PresentationCallIdentity {
  const PresentationCallIdentity({
    required this.toolId,
    required this.args,
    required this.host,
    required this.generation,
  });

  final String toolId;
  final ToolArgs args;

  /// The board/host execution context supplied to the dispatcher. Null means
  /// the global desktop host, which is distinct from a named board host.
  final String? host;
  final int generation;

  @override
  bool operator ==(Object other) =>
      other is PresentationCallIdentity &&
      toolId == other.toolId &&
      args == other.args &&
      host == other.host &&
      generation == other.generation;

  @override
  int get hashCode => Object.hash(toolId, args, host, generation);
}

@immutable
final class PresentationRefreshRequest {
  const PresentationRefreshRequest(this.origin);

  final PresentationCallIdentity origin;
}

@immutable
final class PresentationRefreshRun {
  const PresentationRefreshRun({required this.request, required this.call});

  final PresentationRefreshRequest request;
  final PresentationCallIdentity call;
}

/// State reported after a write-follow-up settles.
sealed class PresentationWriteStatus {
  const PresentationWriteStatus();
}

final class PresentationWriteSucceeded extends PresentationWriteStatus {
  const PresentationWriteSucceeded({required this.refreshPending});
  final bool refreshPending;
}

/// A write that may have reached the tool but did not return a confirmed
/// result. The origin stays visibly stale; it is never replayed for the user.
final class PresentationWriteUnconfirmed extends PresentationWriteStatus {
  const PresentationWriteUnconfirmed();
}

/// The follow-up returned a canonical, confirmed failure. Unlike transport
/// loss/cancellation this is a settled result, but it still must not refresh
/// the read origin.
final class PresentationWriteFailed extends PresentationWriteStatus {
  const PresentationWriteFailed();
}

final class PresentationRefreshFailed extends PresentationWriteStatus {
  const PresentationRefreshFailed();
}

/// Small modal-local state machine for a row → form → result action flow.
///
/// The page owns dispatch and navigation. This controller owns only identity
/// checks and the one allowed automatic read refresh, keeping approval and
/// capability dispatch on their existing route.
class PresentationCallOriginController extends ChangeNotifier {
  int _nextGeneration = 0;
  PresentationCallIdentity? _active;
  PresentationCallIdentity? _origin;
  bool _originMayRefresh = false;
  PresentationWriteStatus? _writeStatus;

  PresentationCallIdentity? get active => _active;
  PresentationCallIdentity? get origin => _origin;
  PresentationWriteStatus? get writeStatus => _writeStatus;

  /// Desktop dispatch derives its host from the currently selected board. A
  /// follow-up must not silently run under a different board after navigation.
  bool activeHostMatches(String? currentHost) => _active?.host == currentHost;

  /// Starts a new visible call. Starting another list/result call replaces
  /// both its active identity and any prior refresh origin.
  PresentationCallIdentity begin({
    required String toolId,
    required ToolArgs args,
    required String? host,
  }) {
    final identity = PresentationCallIdentity(
      toolId: toolId,
      args: args,
      host: host,
      generation: ++_nextGeneration,
    );
    _active = identity;
    _origin = null;
    _originMayRefresh = false;
    _writeStatus = null;
    notifyListeners();
    return identity;
  }

  /// Starts a form/result follow-up without losing the row action's read
  /// origin. A caller must use this rather than [begin] for that path.
  PresentationCallIdentity beginFollowup({
    required String toolId,
    required ToolArgs args,
    required String? host,
  }) {
    final identity = PresentationCallIdentity(
      toolId: toolId,
      args: args,
      host: host,
      generation: ++_nextGeneration,
    );
    _active = identity;
    notifyListeners();
    return identity;
  }

  /// Marks a successful read result as eligible for actions. Late results do
  /// not become active merely because their tool id matches.
  bool acceptResult(PresentationCallIdentity identity) {
    if (_active != identity) return false;
    return true;
  }

  /// Row actions retain the list call as the origin through nested forms.
  bool captureOriginForRowAction(
    PresentationCallIdentity identity, {
    required bool originEffectIsRead,
  }) {
    if (!acceptResult(identity)) return false;
    _origin = identity;
    _originMayRefresh = originEffectIsRead;
    _writeStatus = null;
    notifyListeners();
    return true;
  }

  /// Return the original read call only after a confirmed write. The caller
  /// must dispatch it through the normal approval/capability route.
  PresentationRefreshRequest? writeSucceeded({
    required PresentationCallIdentity write,
    required bool refreshOrigin,
  }) {
    if (_active != write) return null;
    final captured = _origin;
    final refresh = refreshOrigin && captured != null && _originMayRefresh;
    _writeStatus = PresentationWriteSucceeded(refreshPending: refresh);
    notifyListeners();
    return refresh ? PresentationRefreshRequest(captured) : null;
  }

  /// Creates a distinct generation for the automatic re-read. The origin's
  /// inputs and host are copied, but a late response cannot pass as this new
  /// request merely by matching those values.
  PresentationRefreshRun? beginRefresh(PresentationRefreshRequest request) {
    if (_origin != request.origin) return null;
    final call = PresentationCallIdentity(
      toolId: request.origin.toolId,
      args: request.origin.args,
      host: request.origin.host,
      generation: ++_nextGeneration,
    );
    _active = call;
    notifyListeners();
    return PresentationRefreshRun(request: request, call: call);
  }

  /// Cancellation, disconnect and a failed write leave the result
  /// unconfirmed. In particular, no automatic write retry is possible here.
  void writeUnconfirmed(PresentationCallIdentity write) {
    if (_active != write) return;
    _writeStatus = const PresentationWriteUnconfirmed();
    notifyListeners();
  }

  void writeFailed(PresentationCallIdentity write) {
    if (_active != write) return;
    _writeStatus = const PresentationWriteFailed();
    notifyListeners();
  }

  /// The page must call this again when the read response returns. This blocks
  /// a refresh from project A overwriting a newer project B result.
  bool acceptRefresh(PresentationRefreshRun refresh) {
    return _origin == refresh.request.origin && _active == refresh.call;
  }

  void refreshFailed(PresentationRefreshRun refresh) {
    if (!acceptRefresh(refresh)) return;
    _writeStatus = const PresentationRefreshFailed();
    notifyListeners();
  }

  void replaceActive(PresentationCallIdentity identity) {
    _active = identity;
    notifyListeners();
  }

  /// Closing/replacing the origin makes any in-flight refresh ineligible.
  void clear() {
    _active = null;
    _origin = null;
    _originMayRefresh = false;
    _writeStatus = null;
    notifyListeners();
  }
}
