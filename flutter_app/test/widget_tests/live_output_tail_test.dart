/// Tests for the live output tail: the pure chunk→lines fold, and the
/// in-flight strip the modal and inline pin render from it.
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/pages/expanded_modal_page.dart';
import 'package:upeg/src/rust/api/dispatch_stream.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/state/dispatch_stream_provider.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/expanded_modal/live_output_tail.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';
import 'package:upeg/src/widgets/inline/generic_inline_pin_body.dart';

import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/tool_fixture.dart';

const _stdout = 'stdout';
const _success = CanonicalToolResult(ok: true, outputs: []);

DispatchStreamEventDto _chunk(int seq, String text) =>
    DispatchStreamEventDto.chunk(
      stream: _stdout,
      seq: BigInt.from(seq),
      chunk: text,
    );

final _toollessTool = fixtureToolDto(
  id: 'fixture.streaming',
  label: 'streaming tool',
  inputFields: const <InputFieldDto>[],
);

final _manualInlineTool = fixtureToolDto(
  id: 'fixture.streaming_inline',
  label: 'streaming inline tool',
  source: const SourceDto.manual(),
  inputFields: const <InputFieldDto>[],
);

/// A dispatch whose events the test pushes by hand.
final class _ScriptedDispatch {
  final StreamController<DispatchStreamEventDto> controller =
      StreamController<DispatchStreamEventDto>();
  final List<DispatchRunId> startedRuns = <DispatchRunId>[];

  DispatchStreamFn get fn =>
      ({
        PinKey? pinKey,
        required ToolId toolId,
        required ToolArgs args,
        required bool approve,
        required DispatchRunId runId,
      }) {
        startedRuns.add(runId);
        return controller.stream;
      };

  /// Settle the scripted run before the tree comes down.
  ///
  /// A run left open past teardown would finish against a disposed
  /// `ProviderScope` — the widget releases its running-tool lease when
  /// the stream ends, and that lease has to land somewhere alive.
  Future<void> finish(WidgetTester tester) async {
    controller.add(const DispatchStreamEventDto.done(result: _success));
    await controller.close();
    await tester.pumpAndSettle();
  }
}

/// Records the run ids handed to the cancel bridge.
final class _CancelLog {
  final List<DispatchRunId> cancelled = <DispatchRunId>[];

  bool call({required DispatchRunId runId}) {
    cancelled.add(runId);
    return true;
  }
}

Widget _modalHarness(_ScriptedDispatch dispatch, _CancelLog cancel) {
  return ProviderScope(
    overrides: [
      ...i18nTestOverrides,
      dispatchStreamFnProvider.overrideWithValue(dispatch.fn),
      cancelDispatchFnProvider.overrideWithValue(cancel.call),
    ],
    child: MaterialApp(
      theme: UpegTheme.darkTheme(),
      home: ExpandedModalPage(tool: _toollessTool),
    ),
  );
}

Widget _inlineHarness(_ScriptedDispatch dispatch, _CancelLog cancel) {
  return ProviderScope(
    overrides: [
      ...i18nTestOverrides,
      dispatchStreamFnProvider.overrideWithValue(dispatch.fn),
      cancelDispatchFnProvider.overrideWithValue(cancel.call),
    ],
    child: MaterialApp(
      theme: UpegTheme.darkTheme(),
      home: Scaffold(
        body: SizedBox(
          width: 280,
          height: 240,
          child: GenericInlinePinBody(
            tool: _manualInlineTool,
            pinKey: (BoardKey.parse('dev'), PinId.parse(_manualInlineTool.id)),
          ),
        ),
      ),
    ),
  );
}

void main() {
  group('LiveTail fold', () {
    test(
      'only_completed_lines_land_in_lines_and_the_rest_stays_in_pending',
      () {
        const tail = LiveTail.empty(maxLines: 4);

        final folded = tail.append('first\nsecond\nthi');

        expect(folded.lines, <String>['first', 'second']);
        expect(folded.pending, 'thi');
        expect(folded.visibleLines, <String>['first', 'second', 'thi']);
      },
    );

    test('a_line_spanning_multiple_chunks_is_joined', () {
      const tail = LiveTail.empty(maxLines: 4);

      final folded = tail.append('he').append('llo').append(' world\n');

      expect(folded.lines, <String>['hello world']);
      expect(folded.pending, isEmpty);
    });

    test('lines_past_maxlines_drop_the_oldest_first', () {
      var tail = const LiveTail.empty(maxLines: 3);

      for (var index = 1; index <= 6; index++) {
        tail = tail.append('line $index\n');
      }

      expect(tail.lines, <String>['line 4', 'line 5', 'line 6']);
      expect(tail.visibleLines.length, 3);
    });

    test('a_crlf_counts_as_a_single_line_ending', () {
      const tail = LiveTail.empty(maxLines: 2);

      final folded = tail.append('alpha\r\nbeta\r');

      expect(folded.lines, <String>['alpha']);
      expect(folded.visibleLines, <String>['alpha', 'beta']);
    });

    test('a_crlf_spanning_a_chunk_boundary_is_still_one_line', () {
      const tail = LiveTail.empty(maxLines: 4);

      final folded = tail.append('done\r').append('\nnext');

      expect(folded.lines, <String>['done']);
      expect(folded.pending, 'next');
    });

    test('a_lone_carriage_return_overwrites_the_current_line', () {
      // The shape progress bars and spinners emit. \r is a protocol, not
      // a character.
      const tail = LiveTail.empty(maxLines: 4);

      final folded = tail.append('50%\r80%\r100%\n');

      expect(folded.lines, <String>['100%']);
      expect(folded.pending, isEmpty);
    });

    test('a_stream_of_only_carriage_returns_never_grows_lines', () {
      var tail = const LiveTail.empty(maxLines: 4);

      for (var index = 0; index <= 100; index++) {
        tail = tail.append('progress $index\r');
      }

      expect(tail.lines, isEmpty, reason: 'no line was ever terminated');
      expect(tail.visibleLines, <String>['progress 100']);
    });

    test('an_open_line_in_a_newline_free_stream_stops_at_the_cap', () {
      var tail = const LiveTail.empty(maxLines: 4);

      for (var index = 0; index < 40; index++) {
        tail = tail.append('x' * 100);
      }

      expect(tail.pending.length, liveTailMaxLineUnits);
      expect(tail.lines, isEmpty);
      expect(tail.visibleLines, <String>[tail.pending]);
    });

    test('a_string_clipped_at_the_cap_stays_intact', () {
      // An emoji (surrogate pair) straddling the cap boundary never
      // leaves a half behind.
      const emoji = '🙂';
      final head = 'x' * (liveTailMaxLineUnits - 1);
      const tail = LiveTail.empty(maxLines: 2);

      final folded = tail.append('$head$emoji');

      expect(folded.pending.length, liveTailMaxLineUnits - 1);
      expect(folded.pending, head);
      expect(folded.pending.runes.every((rune) => rune == 0x78), isTrue);
    });

    test('an_empty_chunk_changes_nothing', () {
      const tail = LiveTail.empty(maxLines: 2);

      expect(identical(tail.append(''), tail), isTrue);
      expect(tail.isEmpty, isTrue);
    });
  });

  group('ExpandedModalPage live output', () {
    testWidgets('mid_run_chunks_show_only_the_last_n_lines', (tester) async {
      final dispatch = _ScriptedDispatch();
      final cancel = _CancelLog();
      await tester.pumpWidget(_modalHarness(dispatch, cancel));

      await tester.tap(find.byKey(const Key('expanded-modal-run-btn')));
      await tester.pump();

      expect(find.byKey(modalLiveOutputTailKey), findsOneWidget);

      for (var index = 1; index <= modalLiveTailMaxLines + 2; index++) {
        dispatch.controller.add(_chunk(index, 'line $index\n'));
      }
      await tester.pump();

      expect(
        find.text('line 1'),
        findsNothing,
        reason: 'the oldest line is pushed out',
      );
      expect(find.text('line 3'), findsOneWidget);
      expect(
        find.text('line ${modalLiveTailMaxLines + 2}'),
        findsOneWidget,
        reason: 'the newest line always stays visible',
      );
      expect(
        find.descendant(
          of: find.byKey(modalLiveOutputTailKey),
          matching: find.byType(Text),
        ),
        // The N tail lines plus the block's own label and Cancel label.
        findsNWidgets(modalLiveTailMaxLines + 2),
      );

      await dispatch.finish(tester);
    });

    testWidgets('cancel_requests_cancellation_for_the_running_run_id', (
      tester,
    ) async {
      final dispatch = _ScriptedDispatch();
      final cancel = _CancelLog();
      await tester.pumpWidget(_modalHarness(dispatch, cancel));

      await tester.tap(find.byKey(const Key('expanded-modal-run-btn')));
      await tester.pump();

      await tester.tap(find.byKey(const Key('expanded-modal-cancel-btn')));
      await tester.pump();

      expect(cancel.cancelled, dispatch.startedRuns);
      expect(cancel.cancelled, hasLength(1));

      await dispatch.finish(tester);
    });

    testWidgets('a_done_event_dismisses_the_tail_and_leaves_the_result', (
      tester,
    ) async {
      final dispatch = _ScriptedDispatch();
      final cancel = _CancelLog();
      await tester.pumpWidget(_modalHarness(dispatch, cancel));

      await tester.tap(find.byKey(const Key('expanded-modal-run-btn')));
      await tester.pump();
      dispatch.controller.add(_chunk(0, 'working\n'));
      await tester.pump();
      expect(find.text('working'), findsOneWidget);

      dispatch.controller.add(
        const DispatchStreamEventDto.done(result: _success),
      );
      await dispatch.controller.close();
      await tester.pumpAndSettle();

      expect(find.byKey(modalLiveOutputTailKey), findsNothing);
      expect(find.byKey(const Key('expanded-modal-outcome')), findsOneWidget);
    });
  });

  group('inline pin live output', () {
    testWidgets('the_inline_pin_also_draws_the_tail_and_cancels_via_cancel', (
      tester,
    ) async {
      final dispatch = _ScriptedDispatch();
      final cancel = _CancelLog();
      await tester.pumpWidget(_inlineHarness(dispatch, cancel));
      await tester.pump();

      await tester.tap(find.byKey(inlineRunButtonKey));
      await tester.pump();

      for (var index = 1; index <= inlineLiveTailMaxLines + 1; index++) {
        dispatch.controller.add(_chunk(index, 'row $index\n'));
      }
      await tester.pump();

      expect(find.byKey(inlineLiveOutputTailKey), findsOneWidget);
      expect(find.text('row 1'), findsNothing);
      expect(find.text('row ${inlineLiveTailMaxLines + 1}'), findsOneWidget);

      await tester.tap(find.byKey(inlineCancelButtonKey));
      await tester.pump();

      expect(cancel.cancelled, dispatch.startedRuns);

      await dispatch.finish(tester);
    });
  });
}
