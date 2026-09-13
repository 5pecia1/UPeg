/// Pause controllability (WS4 Task B1, PRD §5.9).
///
/// Desktop Pause flips a process-local `AtomicBool` on the Rust side —
/// it has no cross-process channel and cannot reach a host living in a
/// separate daemon process. The control is therefore interactive ONLY
/// when THIS process hosts in-process (`HostStateEvent.embedded`);
/// every other host state (attached to a foreign daemon, no host, or a
/// failed bring-up) must grey it out rather than let the tray toggle
/// silently no-op against a process it cannot reach.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/rust/api/events.dart';
import 'package:upeg/src/state/host_state_provider.dart';

/// Pure total switch over every [HostStateEvent] variant. `true` ONLY
/// for `embedded` — a new variant added to the sealed class forces a
/// compile error here until this switch is updated, so the gate can
/// never silently default to "controllable".
bool isPauseControllable(HostStateEvent state) => switch (state) {
  HostStateEvent_Embedded() => true,
  HostStateEvent_Attached() => false,
  HostStateEvent_NoHost() => false,
  HostStateEvent_Failed() => false,
};

/// Whether the tray Pause control may act on this process's host right
/// now. Derived from [hostStateProvider]; defaults to `false` while
/// the stream is loading or has not emitted yet (see
/// `host_state_provider.dart`'s PoC single-event caveat) so an unknown
/// state never reads as controllable.
final pauseControllableProvider = Provider<bool>((ref) {
  final state = ref.watch(hostStateProvider).value;
  return state == null ? false : isPauseControllable(state);
});
