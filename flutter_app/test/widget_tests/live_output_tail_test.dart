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
            pinKey: (BoardKey.parse('dev'), ToolId.parse(_manualInlineTool.id)),
          ),
        ),
      ),
    ),
  );
}

void main() {
  group('LiveTail 접기', () {
    test('완결된_줄만_lines에_쌓이고_나머지는_pending에_남는다', () {
      const tail = LiveTail.empty(maxLines: 4);

      final folded = tail.append('first\nsecond\nthi');

      expect(folded.lines, <String>['first', 'second']);
      expect(folded.pending, 'thi');
      expect(folded.visibleLines, <String>['first', 'second', 'thi']);
    });

    test('여러_chunk에_걸친_한_줄을_이어_붙인다', () {
      const tail = LiveTail.empty(maxLines: 4);

      final folded = tail.append('he').append('llo').append(' world\n');

      expect(folded.lines, <String>['hello world']);
      expect(folded.pending, isEmpty);
    });

    test('maxLines를_넘으면_가장_오래된_줄부터_버린다', () {
      var tail = const LiveTail.empty(maxLines: 3);

      for (var index = 1; index <= 6; index++) {
        tail = tail.append('line $index\n');
      }

      expect(tail.lines, <String>['line 4', 'line 5', 'line 6']);
      expect(tail.visibleLines.length, 3);
    });

    test('씨알엘에프는_한_번의_줄_종료로_처리한다', () {
      const tail = LiveTail.empty(maxLines: 2);

      final folded = tail.append('alpha\r\nbeta\r');

      expect(folded.lines, <String>['alpha']);
      expect(folded.visibleLines, <String>['alpha', 'beta']);
    });

    test('chunk_경계에_걸친_씨알엘에프도_한_줄이다', () {
      const tail = LiveTail.empty(maxLines: 4);

      final folded = tail.append('done\r').append('\nnext');

      expect(folded.lines, <String>['done']);
      expect(folded.pending, 'next');
    });

    test('단독_캐리지리턴은_현재_줄을_덮어쓴다', () {
      // 진행 막대와 spinner가 내는 모양. \r는 글자가 아니라 프로토콜이다.
      const tail = LiveTail.empty(maxLines: 4);

      final folded = tail.append('50%\r80%\r100%\n');

      expect(folded.lines, <String>['100%']);
      expect(folded.pending, isEmpty);
    });

    test('캐리지리턴만_오는_stream도_줄을_늘리지_않는다', () {
      var tail = const LiveTail.empty(maxLines: 4);

      for (var index = 0; index <= 100; index++) {
        tail = tail.append('progress $index\r');
      }

      expect(tail.lines, isEmpty, reason: '종료된 줄이 없다');
      expect(tail.visibleLines, <String>['progress 100']);
    });

    test('개행이_없는_stream의_열린_줄은_상한에서_멈춘다', () {
      var tail = const LiveTail.empty(maxLines: 4);

      for (var index = 0; index < 40; index++) {
        tail = tail.append('x' * 100);
      }

      expect(tail.pending.length, liveTailMaxLineUnits);
      expect(tail.lines, isEmpty);
      expect(tail.visibleLines, <String>[tail.pending]);
    });

    test('상한에서_잘려도_문자열은_온전하다', () {
      // 이모지(surrogate pair)가 상한 경계에 걸쳐도 반쪽만 남지 않는다.
      const emoji = '🙂';
      final head = 'x' * (liveTailMaxLineUnits - 1);
      const tail = LiveTail.empty(maxLines: 2);

      final folded = tail.append('$head$emoji');

      expect(folded.pending.length, liveTailMaxLineUnits - 1);
      expect(folded.pending, head);
      expect(folded.pending.runes.every((rune) => rune == 0x78), isTrue);
    });

    test('빈_chunk는_아무것도_바꾸지_않는다', () {
      const tail = LiveTail.empty(maxLines: 2);

      expect(identical(tail.append(''), tail), isTrue);
      expect(tail.isEmpty, isTrue);
    });
  });

  group('ExpandedModalPage 실시간 출력', () {
    testWidgets('실행_중_chunk는_마지막_N줄만_보여준다', (tester) async {
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

      expect(find.text('line 1'), findsNothing, reason: '가장 오래된 줄은 밀려난다');
      expect(find.text('line 3'), findsOneWidget);
      expect(
        find.text('line ${modalLiveTailMaxLines + 2}'),
        findsOneWidget,
        reason: '가장 최근 줄은 항상 보인다',
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

    testWidgets('Cancel은_실행_중인_run_id로_취소를_요청한다', (tester) async {
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

    testWidgets('Done이_오면_tail은_사라지고_결과가_남는다', (tester) async {
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

  group('inline pin 실시간 출력', () {
    testWidgets('inline도_tail을_그리고_Cancel로_취소한다', (tester) async {
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
