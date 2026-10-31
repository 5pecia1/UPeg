/// Popup auto-hide observer (inventory rows C01 + C02).
///
/// Wraps the app's home builder so the [WindowListener] registration
/// is live regardless of route. On `onWindowBlur` it consults the
/// typed `windowModeProvider` and the `pinnedProvider` flag: hide is
/// dispatched ONLY when the window is in popup mode AND not pinned —
/// matching the 1Password-mini suppression rule.
///
/// The hide is not immediate. A blur only arms a [kPopupAutoHideGrace]
/// timer; when it fires, the window is hidden only if it is still an
/// unpinned popup that reports no focus ([WindowHider.isFocused]), and
/// `onWindowFocus` cancels it. The grace is there for the global summon
/// hotkey: on X11 its passive key grab makes GDK report a focus loss
/// (FocusOut NotifyGrab) as soon as the key goes down, before the key-down
/// callback runs. Hiding on that blur would hide the popup before the summon
/// toggle looks at it, so the toggle would find a hidden window and summon it
/// again — the popup flashes instead of closing. With the grace the toggle
/// sees the popup still shown and hides it. A real blur (a click on another
/// window) still hides the popup, one grace period later.
///
/// The window's own hide event cancels the timer as well. Otherwise a second
/// quick hotkey press, which summons the popup again while its key grab still
/// reports no focus, would let the first press's timer hide the popup it just
/// showed.
///
/// Type system / SoC notes:
/// - [WindowHider] is an abstract class — the test seam. Production
///   code uses [WindowManagerHider]; tests pass a recording double so
///   the platform channel is never touched.
/// - No FRB plumbing: desktop focus is a pure Flutter concern. The
///   FRB `focus_loss_stream` exists but never emits — we route around
///   it via `window_manager`.
library;

import 'dart:async' show Timer, unawaited;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:window_manager/window_manager.dart';

import 'package:upeg/src/state/pinned_provider.dart';
import 'package:upeg/src/state/window_mode_provider.dart';

/// How long a blur waits before the popup is hidden, see the library
/// comment. Long enough for a key-down callback (hotkey toggle) to run
/// first, short enough that clicking another window still feels immediate.
const Duration kPopupAutoHideGrace = Duration(milliseconds: 150);

/// `window_manager` event name for the native hide signal. The plugin only
/// forwards it through [WindowListener.onWindowEvent], with no constant.
const String _windowEventHide = 'hide';

/// Test seam for `windowManager.hide()` and `windowManager.isFocused()`.
/// Abstract class because Dart 3 `class` is non-final by default but
/// tagging the seam explicitly surfaces intent — the seam exists FOR
/// overrides, not for accidental subclassing of the production hider.
abstract class WindowHider {
  const WindowHider();

  /// Hide the current window. Production implementation routes to
  /// `windowManager.hide()`; tests record the call.
  Future<void> hide();

  /// Whether the window is the active one right now. Production routes to
  /// `windowManager.isFocused()`; tests return a canned answer.
  Future<bool> isFocused();
}

/// Default production hider — delegates to `window_manager`.
class WindowManagerHider extends WindowHider {
  const WindowManagerHider();

  @override
  Future<void> hide() => windowManager.hide();

  @override
  Future<bool> isFocused() => windowManager.isFocused();
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
  /// The armed grace timer: set by a qualifying blur, cleared when it fires,
  /// on focus, on the window's hide event, and on dispose.
  Timer? _graceTimer;

  @override
  void initState() {
    super.initState();
    windowManager.addListener(this);
    PopupAutoHideObserver._activeState = this;
  }

  @override
  void dispose() {
    _graceTimer?.cancel();
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

  @override
  void onWindowFocus() => _cancelGrace();

  /// A hidden window leaves the grace timer nothing to hide; see the library
  /// comment for the double hotkey press this guards against.
  @override
  void onWindowEvent(String eventName) {
    if (eventName == _windowEventHide) _cancelGrace();
  }

  void _cancelGrace() {
    _graceTimer?.cancel();
    _graceTimer = null;
  }

  /// Auto-hide applies only to an unpinned popup. Checked on blur and again
  /// when the grace period ends, since either may change in between.
  bool get _autoHides =>
      ref.read(windowModeProvider) == WindowMode.popup &&
      !ref.read(pinnedProvider);

  void _handleBlur() {
    if (_autoHides) {
      _graceTimer?.cancel();
      _graceTimer = Timer(
        kPopupAutoHideGrace,
        () => unawaited(_hideIfStillBlurred()),
      );
    }
  }

  Future<void> _hideIfStillBlurred() async {
    _graceTimer = null;
    if (!_autoHides) return;
    final focused = await widget.hider.isFocused();
    if (!mounted || focused) return;
    await widget.hider.hide();
  }

  @override
  Widget build(BuildContext context) => widget.child;
}
