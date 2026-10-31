/// Window visibility as app state: is the window actually on screen?
///
/// Background work that only matters while the user can see the window
/// (shared-state polling, Live Pin ticks) watches [windowShownProvider] and
/// pauses while the window is hidden, or minimized where the platform
/// reports it. Losing focus is not hiding: a visible popup or an unfocused
/// window keeps refreshing.
///
/// Type-system notes: the platform event source is the injectable
/// [WindowVisibilitySource] seam, the same convention as `WindowHider`,
/// `WindowSummonDriver` and `WindowModeDriver`. Production routes to
/// `window_manager`; tests override [windowVisibilitySourceProvider] with
/// `null` or a recording double. Nothing here may reach for `windowManager`
/// directly: its first access registers a method-call handler, which asserts
/// in a plain `test()` that has no Flutter binding.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:window_manager/window_manager.dart';

import 'package:upeg/src/platform/window.dart' show isWindowManagerSupported;

/// `window_manager` event names for the native show and hide signals. The
/// plugin only forwards them through [WindowListener.onWindowEvent]; it has
/// no dedicated callback or constant for either.
const String _eventShow = 'show';
const String _eventHide = 'hide';

/// Platform seam for the window events that say whether it is on screen
/// (show, hide, minimize, restore, maximize). Production routes to
/// `window_manager`; tests pass a double that keeps the listener.
abstract class WindowVisibilitySource {
  const WindowVisibilitySource();

  void addListener(WindowListener listener);
  void removeListener(WindowListener listener);
}

/// Default production source — delegates to `window_manager`.
class WindowManagerVisibilitySource extends WindowVisibilitySource {
  const WindowManagerVisibilitySource();

  @override
  void addListener(WindowListener listener) =>
      windowManager.addListener(listener);

  @override
  void removeListener(WindowListener listener) =>
      windowManager.removeListener(listener);
}

/// The event source, or `null` where `window_manager` is unsupported (the web
/// build). It is also supported under a desktop host's `flutter test`, so
/// tests that run without a binding must override this provider.
final windowVisibilitySourceProvider = Provider<WindowVisibilitySource?>(
  (ref) =>
      isWindowManagerSupported ? const WindowManagerVisibilitySource() : null,
);

/// `true` while the window is on screen; `false` once it is hidden or
/// minimized.
///
/// Events: `hide` and `show` are the native show/hide signals (GTK widget
/// signals on Linux, so presenting a hidden window always emits `show`, and
/// `WM_SHOWWINDOW` on Windows; the macOS plugin reports neither). `minimize`
/// and `restore` come from the window-state change on Linux X11, Windows and
/// macOS. Windows reports a minimized window that was maximized as
/// `maximize` when it comes back, never `restore`, so `maximize` counts as
/// shown too. GTK3 on Wayland is not known to report minimize.
///
/// The two failure directions are not equal. A hide or minimize that goes
/// unreported is harmless: the window keeps counting as shown and background
/// refresh simply continues. The risky direction is a show or restore that
/// goes unreported, which leaves refresh paused until the next event, so
/// each platform's way of bringing a window back must map to `true`.
///
/// Without a source the state stays `true`, so nothing pauses.
class WindowShownNotifier extends Notifier<bool> with WindowListener {
  @override
  bool build() {
    final source = ref.read(windowVisibilitySourceProvider);
    if (source != null) {
      source.addListener(this);
      ref.onDispose(() => source.removeListener(this));
    }
    return true;
  }

  @override
  void onWindowEvent(String eventName) {
    switch (eventName) {
      case _eventHide:
        state = false;
      case _eventShow:
        state = true;
    }
  }

  @override
  void onWindowMinimize() => state = false;

  @override
  void onWindowRestore() => state = true;

  /// Windows reports a minimized window that was maximized as `maximize` when
  /// it comes back (`WM_SIZE` with `SIZE_MAXIMIZED`), never as `restore`.
  /// Maximizing never hides a window, so counting it as shown can at worst
  /// keep refresh running.
  @override
  void onWindowMaximize() => state = true;
}

final windowShownProvider = NotifierProvider<WindowShownNotifier, bool>(
  WindowShownNotifier.new,
);
