/// Window mode (popup ↔ full) state.
///
/// The Flutter surface owns the popup/full toggle that the desktop
/// binary exposes via the tray. The actual platform call (resize +
/// decorations) lives in `platform/window.dart`; this provider only
/// holds the enum state so tray callbacks and Riverpod consumers can
/// coordinate without reaching for `windowManager` directly.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/platform/window.dart' show initialWindowMode;

/// PRD §5.9: popup ↔ full window mode toggle. Single window
/// instance — only the geometry/decorations change.
enum WindowMode { popup, full }

/// Default window mode at app start: Full unless the
/// `UPEG_DESKTOP_POPUP` env var is set. That env probe
/// lives in `platform/window.dart::initialWindowMode` and the Notifier
/// constructor reads it through dependency injection so unit tests
/// stay platform-channel-free.
const WindowMode kDefaultWindowMode = WindowMode.full;

class WindowModeNotifier extends Notifier<WindowMode> {
  /// `initial` defaults to `initialWindowMode()` (env-driven). Tests
  /// override it via `windowModeProvider.overrideWith(...)`. Reading
  /// the env at `build()` time keeps the notifier self-contained —
  /// callers don't need to push it via `ref.read(...).set(...)` during
  /// app boot, which Riverpod 3.x forbids ("providers may not modify
  /// other providers during initialization").
  WindowModeNotifier({this.initial});

  final WindowMode? initial;

  @override
  WindowMode build() => initial ?? initialWindowMode();

  /// Flip popup ↔ full. Returns the new state for callers that need
  /// to chain platform calls.
  WindowMode toggle() {
    final next = switch (state) {
      WindowMode.popup => WindowMode.full,
      WindowMode.full => WindowMode.popup,
    };
    state = next;
    return next;
  }

  /// Force a specific mode (e.g. tray "Open Dashboard" always wants
  /// full).
  void set(WindowMode mode) {
    state = mode;
  }
}

final windowModeProvider = NotifierProvider<WindowModeNotifier, WindowMode>(
  WindowModeNotifier.new,
);
