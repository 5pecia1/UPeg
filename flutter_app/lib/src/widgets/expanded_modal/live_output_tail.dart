/// Live tail of a still-running Tool's output.
///
/// A running Tool can emit far more text than a modal card — let alone a
/// board tile — can hold, and none of it is the answer: the canonical
/// result is. So the tail is deliberately a *tail*: the last few lines,
/// capped, as a sign of life rather than a transcript.
///
/// [LiveTail] is the pure fold (chunks in, capped lines out) and carries
/// no widget dependency, so the capping rule is unit-testable without a
/// pump. [LiveOutputTailView] is the monospace rendering of one.
library;

import 'package:flutter/material.dart';

import 'package:upeg/src/theme/upeg_theme.dart';

/// Lines the expanded modal keeps — enough to watch a build scroll by
/// inside a 640px-tall card.
const int modalLiveTailMaxLines = 8;

/// Lines an inline pin keeps. A board tile is short, and the pin's job is
/// the result, not the log.
const int inlineLiveTailMaxLines = 3;

/// Widget key for the modal's tail block.
const Key modalLiveOutputTailKey = Key('expanded-modal-live-tail');

/// Widget key for an inline pin's tail block.
const Key inlineLiveOutputTailKey = Key('inline-live-tail');

/// How many UTF-16 code units of one open line the tail keeps.
///
/// A tool that prints a megabyte on one line — a base64 blob, a minified
/// bundle — used to make [LiveTail.pending] grow without bound, and every
/// chunk re-copied the whole thing, so the fold went quadratic in the
/// output size. The pane renders one ellipsised row per line, so
/// everything past this cap could never be shown anyway. Mirrors the
/// TUI's `TUI_LIVE_TAIL_MAX_LINE_BYTES`.
const int liveTailMaxLineUnits = 512;

/// Line feed — commits the open line.
const int _lineFeed = 0x0A;

/// Carriage return. Not punctuation: a bare `\r` is "redraw the line I
/// just drew" (the shape progress bars and spinners emit) and `\r\n` is a
/// single line terminator.
const int _carriageReturn = 0x0D;

/// Highest UTF-16 code unit that opens a surrogate pair, and the lowest.
/// A cap that landed between the halves would leave a lone surrogate in
/// the string, which renders as a replacement glyph.
const int _highSurrogateStart = 0xD800;
const int _highSurrogateEnd = 0xDBFF;

/// The last [maxLines] lines of a run's output so far.
///
/// Chunks arrive at whatever boundary the invoker had ready, so a chunk
/// may end mid-line; that remainder is held in [pending] until its
/// terminator arrives. Both halves are bounded — [lines] by [maxLines]
/// and [pending] by [liveTailMaxLineUnits] — so memory stays flat no
/// matter how much a tool prints, with or without newlines.
///
/// [pendingCarriageReturn] is what makes a `\r\n` split across two chunks
/// behave like the one terminator it is: a bare `\r` clears the open line
/// (overwrite semantics), but a `\r` whose `\n` arrives in the next chunk
/// commits it.
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
  /// Scans for terminators rather than walking code units one at a time,
  /// so the cost is linear in the chunk and surrogate pairs (which never
  /// contain `\r` or `\n`) survive the split intact.
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

    // A `\r` left over from the previous chunk: its meaning is decided by
    // the first character of this one.
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
        // Lone `\r`: the tool is overwriting the line it just drew, so
        // the half-drawn text is not output worth keeping.
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
  /// The open line is shown rather than withheld — a tool that prints a
  /// prompt without a newline is exactly the case where a sign of life
  /// matters most.
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
      // Dropping the trailing half-character is the only way to keep the
      // result a valid string.
      keep -= 1;
    }
    return '$base${addition.substring(0, keep)}';
  }
}

/// Monospace rendering of a [LiveTail].
///
/// Renders nothing when the tail is empty so a run that reports no
/// progress (every in-process function tool) leaves no hole in the
/// layout.
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
