/// Per-pin focus state used by the keyboard reorder / move-mode
/// shortcuts (inventory rows F03 / F12 / F15).
///
/// State is `null` when no pin is in keyboard focus — the default
/// for a freshly-opened BoardPage. Tapping a pin (or arrow-key
/// navigation, once landed) calls [`focus`] with the pin's tool id;
/// closing the modal or losing focus calls [`blur`].
///
/// Type system: focus stores a parsed [ToolId], so keyboard move paths
/// cannot accidentally carry arbitrary display strings past the FRB
/// boundary.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';

/// Notifier for [`focusedPinProvider`].
class FocusedPinNotifier extends Notifier<PinId?> {
  @override
  PinId? build() => null;

  /// Set the focused pin to [toolId].
  void focus(PinId pinId) {
    state = pinId;
  }

  /// Drop focus (e.g. modal closed, BoardPage replaced).
  void blur() {
    state = null;
  }
}

final focusedPinProvider = NotifierProvider<FocusedPinNotifier, PinId?>(
  FocusedPinNotifier.new,
);

/// Focus the pin represented by a layout placement. BoardCanvas and
/// keyboard paths both carry [`PlacementDto`], so this helper keeps
/// the parsing boundary in one place.
void focusPlacement(ProviderContainer container, PlacementDto placement) {
  container
      .read(focusedPinProvider.notifier)
      .focus(PinId.parse(placement.pinId));
}
