/// Single-shot bridge for popup → board pin activation.
///
/// The popup writes the tapped tool id here and flips
/// [windowModeProvider] to [WindowMode.full]; the board page reads the
/// pending id once on mount, dispatches `pin_activation_for`, and
/// clears the slot. This keeps the two pages decoupled (popup never
/// imports board code) and avoids interleaving the activation with the
/// in-flight window-mode transition.
///
/// The state holds a validated [ToolId] instead of a raw string so popup
/// search hits cannot enqueue malformed tool identities into the board page.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/identity.dart';

class PendingActivationNotifier extends Notifier<ToolId?> {
  @override
  ToolId? build() => null;

  /// Queue [toolId] for activation. Overwrites any prior pending id so
  /// the most recent tap wins (the board page only acts on the latest).
  void set(ToolId toolId) {
    state = toolId;
  }

  /// Drop the pending id once the board page has dispatched it.
  void clear() {
    state = null;
  }
}

final pendingActivationProvider =
    NotifierProvider<PendingActivationNotifier, ToolId?>(
      PendingActivationNotifier.new,
    );
