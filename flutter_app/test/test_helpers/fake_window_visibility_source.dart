/// Recording double for the window visibility seam
/// (`lib/src/platform/window_visibility.dart`).
///
/// Tests that read the polling or Live providers without a Flutter binding
/// must not reach `windowManager`, so they override
/// `windowVisibilitySourceProvider` with this double (or with `null`). It
/// keeps the listeners the app registers and replays window events to them
/// the way the `window_manager` plugin does: `onWindowEvent` first, then the
/// dedicated callback for the events that have one.
library;

import 'package:upeg/src/platform/window_visibility.dart';
import 'package:window_manager/window_manager.dart' show WindowListener;

class FakeWindowVisibilitySource extends WindowVisibilitySource {
  final List<WindowListener> listeners = <WindowListener>[];

  @override
  void addListener(WindowListener listener) => listeners.add(listener);

  @override
  void removeListener(WindowListener listener) => listeners.remove(listener);

  /// Deliver a native window event by name to every registered listener.
  void emit(String eventName) {
    for (final listener in List<WindowListener>.of(listeners)) {
      listener.onWindowEvent(eventName);
      switch (eventName) {
        case 'minimize':
          listener.onWindowMinimize();
        case 'restore':
          listener.onWindowRestore();
        case 'maximize':
          listener.onWindowMaximize();
        case 'blur':
          listener.onWindowBlur();
        case 'focus':
          listener.onWindowFocus();
      }
    }
  }

  void hide() => emit('hide');
  void show() => emit('show');
  void minimize() => emit('minimize');
  void restore() => emit('restore');

  /// What Windows sends when a minimized, maximized window comes back.
  void maximize() => emit('maximize');
}
