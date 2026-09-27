/// Live tail of a still-running Tool's output.
///
/// A running Tool can emit more text than a modal card or board tile can
/// hold; the canonical result is the answer. The capped tail shows the last
/// few lines as a sign of life, not a transcript.
///
/// [LiveTail] is the pure, widget-free fold from chunks to capped lines, so
/// its capping rule is unit-testable without a pump. [LiveOutputTailView]
/// renders it in monospace.
library;

import 'package:flutter/material.dart';

import 'package:upeg/src/theme/upeg_theme.dart';

/// Lines the expanded modal keeps, enough to watch a build in a 640px-tall card.
const int modalLiveTailMaxLines = 8;

/// Lines an inline pin keeps. A short board tile prioritizes the result.
const int inlineLiveTailMaxLines = 3;

/// Widget key for the modal's tail block.
const Key modalLiveOutputTailKey = Key('expanded-modal-live-tail');

/// Widget key for an inline pin's tail block.
const Key inlineLiveOutputTailKey = Key('inline-live-tail');

/// How many UTF-16 code units of one open line the tail keeps.
///
/// A megabyte-long line, such as a base64 blob or minified bundle, made
/// [LiveTail.pending] unbounded and each chunk recopy it, making the fold
/// quadratic. The pane can render only one ellipsised row per line, so the
/// cap loses no visible text. Mirrors TUI `TUI_LIVE_TAIL_MAX_LINE_BYTES`.
const int liveTailMaxLineUnits = 512;

/// Line feed — commits the open line.
const int _lineFeed = 0x0A;

/// Carriage return: a bare `\r` redraws the line (as progress bars and
/// spinners emit); `\r\n` is one terminator.
const int _carriageReturn = 0x0D;

/// Highest UTF-16 code unit that opens a surrogate pair, and the lowest.
/// A cap that landed between the halves would leave a lone surrogate in
/// the string, which renders as a replacement glyph.
const int _highSurrogateStart = 0xD800;
const int _highSurrogateEnd = 0xDBFF;

/// The last [maxLines] lines of a run's output so far.
///
/// Chunks can end mid-line, so [pending] holds the remainder until its
/// terminator. [lines] is bounded by [maxLines] and [pending] by
/// [liveTailMaxLineUnits], keeping memory flat with or without newlines.
///
/// [pendingCarriageReturn] makes `\r\n` split across chunks one terminator:
/// a bare `\r` clears the open line, while a following `\n` commits it.
@immutable
final class LiveTail {
  const LiveTail._({
    required this.lines,
    required this.pending,
    required this.maxLines,
    required this.pendingCarriageReturn,
  });

  /// An empty tail that will keep at most [maxLines] lines.
  const LiveTail.empty({required this.maxLines})
    : lines = const <String>[],
      pending = '',
      pendingCarriageReturn = false;

  /// Completed lines, already capped to [maxLines], oldest first.
  final List<String> lines;

  /// Trailing text whose terminator has not arrived yet, capped at
  /// [liveTailMaxLineUnits].
  final String pending;

  /// How many lines this tail keeps.
  final int maxLines;

  /// A `\r` ended the last chunk and its companion `\n` may still be in
  /// the next one.
  final bool pendingCarriageReturn;

  bool get isEmpty => lines.isEmpty && pending.isEmpty;

  bool get isNotEmpty => !isEmpty;

  /// Fold one output chunk in, returning the new tail.
  ///
  /// Scans terminators, so cost is linear in the chunk and surrogate pairs
  /// (which never contain `\r` or `\n`) survive intact.
  LiveTail append(String chunk) {
    if (chunk.isEmpty) return this;
    final completed = <String>[...lines];
    var open = pending;
    var carry = pendingCarriageReturn;
    var index = 0;

    void commit() {
      completed.add(open);
      open = '';
      if (completed.length > maxLines) {
        completed.removeRange(0, completed.length - maxLines);
      }
    }

    // The first character decides a `\r` left from the previous chunk.
    if (carry) {
      carry = false;
      if (chunk.codeUnitAt(0) == _lineFeed) {
        commit();
        index = 1;
      } else {
        open = '';
      }
    }

    while (index < chunk.length) {
      final terminator = _nextTerminator(chunk, index);
      if (terminator < 0) {
        open = _capped(open, chunk.substring(index));
        break;
      }
      open = _capped(open, chunk.substring(index, terminator));
      if (chunk.codeUnitAt(terminator) == _lineFeed) {
        commit();
        index = terminator + 1;
        continue;
      }
      // A carriage return. Its companion decides what it meant.
      final next = terminator + 1;
      if (next >= chunk.length) {
        carry = true;
        index = next;
      } else if (chunk.codeUnitAt(next) == _lineFeed) {
        commit();
        index = next + 1;
      } else {
        // A lone `\r` overwrites the incomplete line, so discard it.
        open = '';
        index = next;
      }
    }

    return LiveTail._(
      lines: List<String>.unmodifiable(completed),
      pending: open,
      maxLines: maxLines,
      pendingCarriageReturn: carry,
    );
  }

  /// What to render: completed lines plus the still-open one, capped.
  ///
  /// Show the open line: a newline-free prompt especially needs a sign of life.
  List<String> get visibleLines {
    if (pending.isEmpty) return List<String>.unmodifiable(lines);
    return List<String>.unmodifiable(_lastLines(<String>[...lines, pending]));
  }

  List<String> _lastLines(List<String> all) =>
      all.length <= maxLines ? all : all.sublist(all.length - maxLines);

  /// Index of the next `\r` or `\n` at or after [from], or `-1`.
  static int _nextTerminator(String text, int from) {
    for (var index = from; index < text.length; index++) {
      final unit = text.codeUnitAt(index);
      if (unit == _lineFeed || unit == _carriageReturn) return index;
    }
    return -1;
  }

  /// [base] plus as much of [addition] as the per-line cap allows.
  static String _capped(String base, String addition) {
    if (addition.isEmpty) return base;
    final room = liveTailMaxLineUnits - base.length;
    if (room <= 0) return base;
    if (addition.length <= room) return '$base$addition';
    var keep = room;
    final last = addition.codeUnitAt(keep - 1);
    if (last >= _highSurrogateStart && last <= _highSurrogateEnd) {
      // Drop the trailing half-character to keep a valid string.
      keep -= 1;
    }
    return '$base${addition.substring(0, keep)}';
  }
}

/// Monospace rendering of a [LiveTail].
///
/// Renders nothing for an empty tail, so a no-progress run (every in-process
/// function tool) leaves no layout hole.
class LiveOutputTailView extends StatelessWidget {
  const LiveOutputTailView({
    required this.tail,
    required this.fontSize,
    super.key,
  });

  final LiveTail tail;
  final double fontSize;

  @override
  Widget build(BuildContext context) {
    final visible = tail.visibleLines;
    if (visible.isEmpty) return const SizedBox.shrink();
    final tokens = context.upeg;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        for (final line in visible)
          Text(
            line,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(
              fontFamily: upegMonoFontFamily,
              fontFamilyFallback: upegMonoFontFamilyFallback,
              fontSize: fontSize,
              color: tokens.fg3,
              height: 1.4,
            ),
          ),
      ],
    );
  }
}
