/// Container tests for the popup activation controller
/// (lib/src/popup/popup_activation_controller.dart): inline run
/// recording, the full-surface handoff, the double-dispatch gate, and
/// the F2 copy path.
library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/popup/popup_activation_controller.dart';
import 'package:upeg/src/popup/popup_inline_outcome_provider.dart';
import 'package:upeg/src/rust/api/pin_activation.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/state/last_outcome_provider.dart';
import 'package:upeg/src/state/live_outcome_provider.dart';
import 'package:upeg/src/state/pending_activation_provider.dart';
import 'package:upeg/src/state/pin_activation_provider.dart';
import 'package:upeg/src/state/window_mode_provider.dart';
import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';

class _RecordingClipboardWriter extends ClipboardWriter {
  final List<String> written = [];

  @override
  Future<void> write(String text) async {
    written.add(text);
  }
}

CanonicalToolResult _okResult(String text) => CanonicalToolResult(
  ok: true,
  primaryOutputId: 'out',
  outputs: [
    CanonicalOutputEntry(
      id: 'out',
      kind: 'string',
      value: CanonicalOutputValue.string(value: text),
    ),
  ],
);

const CanonicalToolResult _errorResult = CanonicalToolResult(
  ok: false,
  outputs: [],
  error: CanonicalToolError(code: 'boom', message: 'it broke'),
);

ProviderContainer _container({
  required PinActivationDto Function(ToolId) verdict,
  required Future<CanonicalToolResult> Function(ToolId) dispatch,
  List<String>? dispatched,
}) {
  final container = ProviderContainer(
    overrides: [
      pinActivationProvider.overrideWith(
        (ref) =>
            ({required toolId, required argsJson}) => verdict(toolId),
      ),
      liveDispatchToolFnProvider.overrideWith(
        (ref) => ({required toolId, required args}) {
          dispatched?.add(toolId.value);
          return dispatch(toolId);
        },
      ),
      windowModeProvider.overrideWith(
        () => WindowModeNotifier(initial: WindowMode.popup),
      ),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

void main() {
  group('PopupActivationController', () {
    test('즉시_dispatch_도구는_popup_안에서_실행되어_결과와_lastOutcome에_기록된다', () async {
      final toolId = ToolId.parse('id.uuid_v7');
      final outcome = _okResult('0198-uuid');
      final container = _container(
        verdict: (id) => PinActivationDto.dispatchImmediate(toolId: id.value),
        dispatch: (_) async => outcome,
      );

      await container.read(popupActivationControllerProvider).activate(toolId);

      // Popup-local inline cache holds the outcome, marked as latest.
      final inline = container.read(popupInlineOutcomeProvider);
      expect(inline.byTool[toolId], outcome);
      expect(inline.lastRun, toolId);
      // Shared board cache got the same record (return-to-board parity),
      // marked session-fresh (not restored-from-store).
      final shared = container.read(lastOutcomeProvider)[toolId];
      expect(shared, isA<FreshOutcome>());
      expect(shared?.result, outcome);
      // The popup stayed open: no mode flip, no pending handoff.
      expect(container.read(windowModeProvider), WindowMode.popup);
      expect(container.read(pendingActivationProvider), isNull);
    });

    test('인라인_실행_실패는_popup_결과에만_기록되고_lastOutcome은_비어있다', () async {
      final toolId = ToolId.parse('id.uuid_v7');
      final container = _container(
        verdict: (id) => PinActivationDto.dispatchImmediate(toolId: id.value),
        dispatch: (_) async => _errorResult,
      );

      await container.read(popupActivationControllerProvider).activate(toolId);

      expect(
        container.read(popupInlineOutcomeProvider).byTool[toolId],
        (_errorResult),
      );
      expect(container.read(lastOutcomeProvider), isEmpty);
      expect(container.read(windowModeProvider), WindowMode.popup);
    });

    test('폼_필요_도구는_full_전환과_pending_큐잉으로_넘긴다', () async {
      final toolId = ToolId.parse('num.hex_to_decimal');
      final dispatched = <String>[];
      final container = _container(
        verdict: (id) => PinActivationDto.openModal(toolId: id.value),
        dispatch: (_) async => _errorResult,
        dispatched: dispatched,
      );

      await container.read(popupActivationControllerProvider).activate(toolId);

      expect(container.read(windowModeProvider), WindowMode.full);
      expect(container.read(pendingActivationProvider), toolId);
      expect(dispatched, isEmpty, reason: '폼 필요 도구는 popup에서 dispatch하지 않는다');
    });

    test('embed_도구도_full_전환과_pending_큐잉으로_넘긴다', () async {
      final toolId = ToolId.parse('web.docs');
      final container = _container(
        verdict: (id) => PinActivationDto.openEmbed(toolId: id.value),
        dispatch: (_) async => _errorResult,
      );

      await container.read(popupActivationControllerProvider).activate(toolId);

      expect(container.read(windowModeProvider), WindowMode.full);
      expect(container.read(pendingActivationProvider), toolId);
    });

    test('실행_중_재활성화는_중복_dispatch를_막는다', () async {
      final toolId = ToolId.parse('id.uuid_v7');
      final gate = Completer<CanonicalToolResult>();
      final dispatched = <String>[];
      final container = _container(
        verdict: (id) => PinActivationDto.dispatchImmediate(toolId: id.value),
        dispatch: (_) => gate.future,
        dispatched: dispatched,
      );

      final controller = container.read(popupActivationControllerProvider);
      final first = controller.activate(toolId);
      final second = controller.activate(toolId);
      gate.complete(_okResult('once'));
      await Future.wait([first, second]);

      expect(dispatched, ['id.uuid_v7']);
    });

    test('결과_표시_상태에서_같은_툴을_다시_실행하면_결과가_갱신된다', () async {
      final toolId = ToolId.parse('id.uuid_v7');
      var run = 0;
      final container = _container(
        verdict: (id) => PinActivationDto.dispatchImmediate(toolId: id.value),
        dispatch: (_) async => _okResult('run-${++run}'),
      );

      final controller = container.read(popupActivationControllerProvider);
      await controller.activate(toolId);
      await controller.activate(toolId);

      final outcome = container.read(popupInlineOutcomeProvider).byTool[toolId];
      expect(outcome, isNotNull);
      expect(outcome!.primaryOutputText, 'run-2');
      expect(run, 2);
    });

    test('F2_복사는_최근_인라인_결과의_출력을_클립보드에_쓴다', () async {
      final toolId = ToolId.parse('id.uuid_v7');
      final writer = _RecordingClipboardWriter();
      final container = ProviderContainer(
        overrides: [
          pinActivationProvider.overrideWith(
            (ref) =>
                ({required toolId, required argsJson}) =>
                    PinActivationDto.dispatchImmediate(toolId: toolId.value),
          ),
          liveDispatchToolFnProvider.overrideWith(
            (ref) =>
                ({required toolId, required args}) async =>
                    _okResult('copied-text'),
          ),
          windowModeProvider.overrideWith(
            () => WindowModeNotifier(initial: WindowMode.popup),
          ),
          clipboardWriterProvider.overrideWithValue(writer),
        ],
      );
      addTearDown(container.dispose);
      final controller = container.read(popupActivationControllerProvider);

      // No result yet — F2 stays unhandled.
      expect(controller.copyLatestResult(), isFalse);

      await controller.activate(toolId);
      expect(controller.copyLatestResult(), isTrue);
      await Future<void>.delayed(Duration.zero);
      expect(writer.written, ['copied-text']);
    });
  });

  group('popupCopyTextFor', () {
    test('성공_결과는_primary_출력을_복사한다', () {
      expect(popupCopyTextFor(_okResult('abc')), 'abc');
    });

    test('실패_결과는_에러_메시지를_복사한다', () {
      expect(popupCopyTextFor(_errorResult), 'it broke');
    });

    test('출력_없는_성공_결과는_복사할_내용이_없다', () {
      const empty = CanonicalToolResult(ok: true, outputs: []);
      expect(popupCopyTextFor(empty), isNull);
    });
  });
}
