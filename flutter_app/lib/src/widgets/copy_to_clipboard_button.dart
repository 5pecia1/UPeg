/// Reusable button that writes a string to the system clipboard via
/// an injectable [ClipboardWriter] seam.
///
/// The seam exists for testability — `Clipboard.setData` from
/// `package:flutter/services.dart` requires a platform channel that
/// isn't bound in unit tests. Production code uses
/// [FlutterClipboardWriter] (which calls `Clipboard.setData`); tests
/// substitute a recording fake via Riverpod's
/// [clipboardWriterProvider].
library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

/// Abstract writer for the system clipboard. Tests provide a
/// recording implementation; production code uses
/// [FlutterClipboardWriter].
abstract class ClipboardWriter {
  const ClipboardWriter();

  /// Write [text] to the clipboard.
  Future<void> write(String text);
}

/// Production [ClipboardWriter] backed by Flutter's built-in
/// `Clipboard.setData`. Zero external packages.
class FlutterClipboardWriter extends ClipboardWriter {
  const FlutterClipboardWriter();

  @override
  Future<void> write(String text) =>
      Clipboard.setData(ClipboardData(text: text));
}

/// Riverpod handle so widgets (and tests) can swap the writer without
/// threading parameters through the widget tree.
final Provider<ClipboardWriter> clipboardWriterProvider =
    Provider<ClipboardWriter>((ref) => const FlutterClipboardWriter());

/// Small icon button that copies [textToCopy] when tapped. Disabled
/// when [textToCopy] is empty.
class CopyToClipboardButton extends StatelessWidget {
  const CopyToClipboardButton({
    required this.textToCopy,
    required this.writer,
    this.tooltip = 'copy',
    super.key,
  });

  final String textToCopy;
  final ClipboardWriter writer;
  final String tooltip;

  bool get _enabled => textToCopy.isNotEmpty;

  @override
  Widget build(BuildContext context) {
    return IconButton(
      tooltip: tooltip,
      icon: const Icon(Icons.copy_outlined, size: 14),
      onPressed: _enabled ? () => writer.write(textToCopy) : null,
      padding: const EdgeInsets.all(4),
      constraints: const BoxConstraints(minWidth: 24, minHeight: 24),
      visualDensity: VisualDensity.compact,
    );
  }
}
