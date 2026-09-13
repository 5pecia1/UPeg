/// Riverpod bridge for the FRB `hostStateStream`.
///
/// Phase 6 PoC: the Rust side emits a single replay event matching
/// the state captured at `initApp` and closes the stream. Phase 7
/// wires a real broadcaster — this provider's shape is the final one
/// so Phase 7 only needs to replace the source stream.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/rust/api/events.dart';

/// Indirection so widget tests can override the source stream.
final hostStateStreamProvider = Provider<Stream<HostStateEvent>>(
  (ref) => hostStateStream(),
);

/// Latest [HostStateEvent] observed. Defaults to `noHost` until the
/// first event arrives so consumers always have a value to switch on.
final hostStateProvider = StreamProvider<HostStateEvent>((ref) {
  return ref.watch(hostStateStreamProvider);
});
