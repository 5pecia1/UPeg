/// A [WindowModeNotifier] that reports every `set` before applying it.
///
/// Lives on its own rather than inside a page harness because both the
/// `board_page_*` and the `popup_page_*` families need it: both surfaces
/// drive the popup ↔ full toggle, and the assertion in both is "which mode
/// did the widget ask for". Two identical copies is what it was before, and
/// two copies is one edit away from two behaviours.
library;

import 'package:upeg/src/state/window_mode_provider.dart';

class RecordingWindowModeNotifier extends WindowModeNotifier {
  RecordingWindowModeNotifier({required this.onChange, super.initial});

  final void Function(WindowMode mode) onChange;

  @override
  void set(WindowMode mode) {
    onChange(mode);
    super.set(mode);
  }
}
