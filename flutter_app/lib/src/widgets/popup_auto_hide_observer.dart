/// Popup auto-hide observer (inventory rows C01 + C02).
///
/// Wraps the app's home builder so the [WindowListener] registration
/// is live regardless of route. On `onWindowBlur` it consults the
/// typed `windowModeProvider` and the `pinnedProvider` flag: hide is
/// dispatched ONLY when the window is in popup mode AND not pinned —
/// matching the 1Password-mini suppression rule.
///
/// Type system / SoC notes:
/// - [WindowHider] is an abstract class — the test seam. Production
///   code uses [WindowManagerHider]; tests pass a recording double so
///   the platform channel is never touched.
/// - No FRB plumbing: desktop focus is a pure Flutter concern. The
///   FRB `focus_loss_stream` exists but never emits — we route around
///   it via `window_manager`.
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:window_manager/window_manager.dart';

import 'package:upeg/src/state/pinned_provider.dart';
import 'package:upeg/src/state/window_mode_provider.dart';

/// Test seam for `windowManager.hide()`. Abstract class because Dart 3
/// `class` is non-final by default but tagging the seam explicitly
/// surfaces intent — the seam exists FOR overrides, not for accidental
/// subclassing of the production hider.
abstract class WindowHider {
  const WindowHider();

  /// Hide the current window. Production implementation routes to
  /// `windowManager.hide()`; tests record the call.
  Future<void> hide();
}

/// Default production hider — delegates to `window_manager`.
class WindowManagerHider extends WindowHider {
  const WindowManagerHider();

  @override
  Future<void> hide() => windowManager.hide();
}

class PopupAutoHideObserver extends ConsumerStatefulWidget {
  const PopupAutoHideObserver({
    super.key,
    required this.child,
    this.hider = const WindowManagerHider(),
  });

  /// Child subtree this observer wraps. Kept as a vanilla [Widget] so
  /// the observer can mount above any route builder (MaterialApp.home,
  /// page subtrees, etc.) without coupling to a specific page widget.
  final Widget child;

  /// Injected hider — defaults to the production
  /// [WindowManagerHider]. Tests pass a recording double.
  final WindowHider hider;

  /// Drives the same code path as a real `onWindowBlur` callback.
  /// Exposed `@visibleForTesting` because the underlying State class is
  /// private and `WindowListener` callbacks can't be invoked from
  /// platform-free widget tests.
  @visibleForTesting
  void debugTriggerBlur() {
    final state = _activeState;
    if (state == null) {
      throw StateError(
        'PopupAutoHideObserver.debugTriggerBlur called with no mounted state',
      );
    }
    state._handleBlur();
  }

  // The active state instance for this widget. The widget tree may
  // contain at most one PopupAutoHideObserver at a time (it's mounted
  // at the root of MaterialApp.home); a `static` slot keeps the
  // testing accessor a one-liner without exposing private types.
  static _PopupAutoHideObserverState? _activeState;

  @override
  ConsumerState<PopupAutoHideObserver> createState() =>
      _PopupAutoHideObserverState();
}

class _PopupAutoHideObserverState extends ConsumerState<PopupAutoHideObserver>
    with WindowListener {
  @override
  void initState() {
    super.initState();
    windowManager.addListener(this);
    PopupAutoHideObserver._activeState = this;
  }

  @override
  void dispose() {
    windowManager.removeListener(this);
    if (identical(PopupAutoHideObserver._activeState, this)) {
      PopupAutoHideObserver._activeState = null;
    }
    super.dispose();
  }

  @override
  void onWindowBlur() {
    _handleBlur();
  }

  void _handleBlur() {
    final mode = ref.read(windowModeProvider);
    final pinned = ref.read(pinnedProvider);
    if (mode == WindowMode.popup && !pinned) {
      widget.hider.hide();
    }
  }

  @override
  Widget build(BuildContext context) => widget.child;
}
