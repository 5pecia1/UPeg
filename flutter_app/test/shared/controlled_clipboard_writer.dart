import 'dart:async';

import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';

class ControlledClipboardWriter extends ClipboardWriter {
  final completion = Completer<void>();
  final writes = <String>[];

  @override
  Future<void> write(String text) {
    writes.add(text);
    return completion.future;
  }

  void succeed() => completion.complete();
  void fail() => completion.completeError(StateError('Clipboard unavailable'));
}
