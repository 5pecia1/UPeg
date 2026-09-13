/// Status-bar live snapshot provider.
///
/// Wraps the FRB `statusSnapshot()` sync call in a Riverpod `Notifier`
/// so the widget never calls FRB directly (and tests override the
/// provider with a `_FakeStatusNotifier`). The notifier owns a
/// `Timer.periodic` that re-reads the snapshot every
/// [kStatusRefreshInterval] so on-disk paused-flag flips and
/// MCP import load state changes propagate without an event subscription.
library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/rust/api/status.dart' as frb;

/// Re-export the FRB DTO so callers (widgets, tests) don't have to
/// import the generated file directly. Keeps the import surface narrow
/// — change the binding location and only this file moves.
typedef StatusSnapshotDto = frb.StatusSnapshotDto;

/// Polling interval for the status-bar refresh tick. 2 seconds is
/// cheap (sync FRB call + filesystem existence check) and matches
/// the perceived latency budget for "did the pause flag flip yet?".
const Duration kStatusRefreshInterval = Duration(seconds: 2);

/// Synchronous reader contract. Tests override [statusReaderProvider]
/// with a counter-backed closure; production reads `frb.statusSnapshot`
/// directly. Keeping the closure typed avoids stringly-typed glue
/// between the widget and the FRB shim.
typedef StatusReaderFn = StatusSnapshotDto Function();

/// Default reader: the FRB sync shim. Production path.
StatusSnapshotDto _defaultStatusReader() => frb.statusSnapshot();

/// Provider seam for the snapshot reader. Tests override this with a
/// fake; production uses the FRB call. Decouples [StatusNotifier]
/// from the FRB import so test scopes never touch native code.
final statusReaderProvider = Provider<StatusReaderFn>(
  (ref) => _defaultStatusReader,
);

/// Riverpod Notifier driving `StatusBar`. Reads the snapshot every
/// [kStatusRefreshInterval] so the bar reflects on-disk state changes
/// without a dedicated event channel. Override in tests by extending
/// [StatusNotifier] and returning a fixed snapshot from `build()`.
class StatusNotifier extends Notifier<StatusSnapshotDto> {
  Timer? _ticker;

  @override
  StatusSnapshotDto build() {
    final reader = ref.read(statusReaderProvider);
    _ticker = Timer.periodic(kStatusRefreshInterval, (_) => _refresh());
    ref.onDispose(() {
      _ticker?.cancel();
      _ticker = null;
    });
    return reader();
  }

  void _refresh() {
    state = ref.read(statusReaderProvider)();
  }
}

final statusSnapshotProvider =
    NotifierProvider<StatusNotifier, StatusSnapshotDto>(StatusNotifier.new);
