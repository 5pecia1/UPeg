/// Pinned-popup state (1Password-mini-style auto-hide suppression).
///
/// `pinnedProvider` is the single source of truth for whether the
/// popup window should suppress auto-hide on focus loss. The tray
/// "Toggle pin" menu item writes here via [PinnedNotifier.toggle];
/// `PopupAutoHideObserver` reads from here to gate `windowManager.hide`.
///
/// Type system note: `bool` is acceptable because the state is
/// genuinely binary. Introducing a newtype would only add ceremony —
/// there is no other domain value this flag can be confused with.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

class PinnedNotifier extends Notifier<bool> {
  @override
  bool build() => false;

  /// Force a specific value (e.g. seed for tests, or a future
  /// restore-from-persistence hook).
  void set(bool value) {
    state = value;
  }

  /// Flip pinned ↔ unpinned. Used by the tray TogglePin callback.
  void toggle() {
    state = !state;
  }
}

final pinnedProvider = NotifierProvider<PinnedNotifier, bool>(
  PinnedNotifier.new,
);
