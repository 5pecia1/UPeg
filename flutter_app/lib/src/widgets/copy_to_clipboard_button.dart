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

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/i18n/t.dart';

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

/// Show feedback only after the requested system clipboard write settles.
Future<bool> copyWithFeedback({
  required BuildContext context,
  required WidgetRef ref,
  required Future<void> Function() write,
  bool showSuccess = true,
  String successKey = 'clipboard.copied',
}) async {
  var copied = false;
  try {
    await write();
    copied = true;
  } on Object {
    // A platform refusal is feedback, not an escaped async exception.
  }
  if (context.mounted && (showSuccess || !copied)) {
    final key = copied ? successKey : 'clipboard.copy_failed';
    final messenger = ScaffoldMessenger.maybeOf(context);
    messenger?.clearSnackBars();
    messenger?.removeCurrentSnackBar();
    messenger?.showSnackBar(SnackBar(content: Text(tRead(ref, key))));
  }
  return copied;
}

/// Small icon button that copies [textToCopy] when tapped. Disabled
/// when [textToCopy] is empty.
class CopyToClipboardButton extends ConsumerStatefulWidget {
  const CopyToClipboardButton({
    required this.textToCopy,
    required this.writer,
    this.tooltip,
    this.successKey = 'clipboard.copied',
    super.key,
  });

  final String textToCopy;
  final ClipboardWriter writer;
  final String? tooltip;
  final String successKey;

  @override
  ConsumerState<CopyToClipboardButton> createState() =>
      _CopyToClipboardButtonState();
}

class _CopyToClipboardButtonState extends ConsumerState<CopyToClipboardButton> {
  bool _writing = false;
  String? _feedbackKey;
  Timer? _feedbackTimer;

  @override
  void dispose() {
    _feedbackTimer?.cancel();
    super.dispose();
  }

  @override
  void didUpdateWidget(CopyToClipboardButton oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.textToCopy != widget.textToCopy ||
        oldWidget.successKey != widget.successKey) {
      _feedbackTimer?.cancel();
      _feedbackKey = null;
    }
  }

  Future<void> _copy() async {
    _feedbackTimer?.cancel();
    setState(() {
      _writing = true;
      _feedbackKey = null;
    });
    final copiedText = widget.textToCopy;
    final copied = await copyWithFeedback(
      context: context,
      ref: ref,
      write: () => widget.writer.write(copiedText),
      showSuccess: false,
    );
    final feedbackKey = copied ? widget.successKey : 'clipboard.copy_failed';
    if (!mounted) return;
    setState(() {
      _writing = false;
      if (widget.textToCopy == copiedText) _feedbackKey = feedbackKey;
    });
    _feedbackTimer = Timer(const Duration(seconds: 3), () {
      if (mounted) setState(() => _feedbackKey = null);
    });
  }

  @override
  Widget build(BuildContext context) {
    final tooltip = _feedbackKey == null
        ? widget.tooltip ?? t(ref, 'clipboard.copy')
        : t(ref, _feedbackKey!);
    final icon = _feedbackKey == null
        ? Icons.copy_outlined
        : _feedbackKey == 'clipboard.copy_failed'
        ? Icons.error_outline
        : Icons.check;
    return Semantics(
      liveRegion: _feedbackKey != null,
      child: IconButton(
        tooltip: tooltip,
        icon: Icon(icon, size: 14),
        onPressed: widget.textToCopy.isNotEmpty && !_writing ? _copy : null,
        padding: const EdgeInsets.all(4),
        constraints: const BoxConstraints(minWidth: 44, minHeight: 44),
        visualDensity: VisualDensity.standard,
      ),
    );
  }
}
