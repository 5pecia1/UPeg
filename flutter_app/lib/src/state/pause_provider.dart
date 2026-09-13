/// Pause mutator provider.
///
/// The tray dispatch reads this instead of calling FRB directly so
/// widget tests can override it with a recording closure. The shape
/// mirrors `pin_provider.dart` (loader+mutator typedef pair).
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/rust/api/pause.dart' as frb;

/// Flip the pause flag.
typedef TogglePausedFn = frb.PausedStateSnapshotDto Function();

final togglePausedProvider = Provider<TogglePausedFn>(
  (ref) =>
      () => frb.togglePaused(),
);
