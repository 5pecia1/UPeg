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
    test(
      'immediate_dispatch_tool_runs_inside_popup_and_records_result_and_lastOutcome',
      () async {
        final toolId = ToolId.parse('id.uuid_v7');
        final outcome = _okResult('0198-uuid');
        final container = _container(
          verdict: (id) => PinActivationDto.dispatchImmediate(toolId: id.value),
          dispatch: (_) async => outcome,
        );

        await container
            .read(popupActivationControllerProvider)
            .activate(toolId);

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
      },
    );

    test(
      'inline_run_failure_is_recorded_in_popup_result_only_and_lastOutcome_stays_empty',
      () async {
        final toolId = ToolId.parse('id.uuid_v7');
        final container = _container(
          verdict: (id) => PinActivationDto.dispatchImmediate(toolId: id.value),
          dispatch: (_) async => _errorResult,
        );

        await container
            .read(popupActivationControllerProvider)
            .activate(toolId);

        expect(
          container.read(popupInlineOutcomeProvider).byTool[toolId],
          (_errorResult),
        );
        expect(container.read(lastOutcomeProvider), isEmpty);
        expect(container.read(windowModeProvider), WindowMode.popup);
      },
    );

    test(
      'form_required_tool_hands_off_via_full_switch_and_pending_queue',
      () async {
        final toolId = ToolId.parse('num.hex_to_decimal');
        final dispatched = <String>[];
        final container = _container(
          verdict: (id) => PinActivationDto.openModal(toolId: id.value),
          dispatch: (_) async => _errorResult,
          dispatched: dispatched,
        );

        await container
            .read(popupActivationControllerProvider)
            .activate(toolId);

        expect(container.read(windowModeProvider), WindowMode.full);
        expect(container.read(pendingActivationProvider), toolId);
        expect(
          dispatched,
          isEmpty,
          reason: 'form-required tools are not dispatched from the popup',
        );
      },
    );

    test(
      'embed_tool_also_hands_off_via_full_switch_and_pending_queue',
      () async {
        final toolId = ToolId.parse('web.docs');
        final container = _container(
          verdict: (id) => PinActivationDto.openEmbed(toolId: id.value),
          dispatch: (_) async => _errorResult,
        );

        await container
            .read(popupActivationControllerProvider)
            .activate(toolId);

        expect(container.read(windowModeProvider), WindowMode.full);
        expect(container.read(pendingActivationProvider), toolId);
      },
    );

    test('reactivation_while_running_blocks_duplicate_dispatch', () async {
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

    test(
      'rerunning_same_tool_while_showing_result_refreshes_the_result',
      () async {
        final toolId = ToolId.parse('id.uuid_v7');
        var run = 0;
        final container = _container(
          verdict: (id) => PinActivationDto.dispatchImmediate(toolId: id.value),
          dispatch: (_) async => _okResult('run-${++run}'),
        );

        final controller = container.read(popupActivationControllerProvider);
        await controller.activate(toolId);
        await controller.activate(toolId);

        final outcome = container
            .read(popupInlineOutcomeProvider)
            .byTool[toolId];
        expect(outcome, isNotNull);
        expect(outcome!.primaryOutputText, 'run-2');
        expect(run, 2);
      },
    );

    test('F2_copy_writes_latest_inline_result_output_to_clipboard', () async {
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
    test('successful_result_copies_primary_output', () {
      expect(popupCopyTextFor(_okResult('abc')), 'abc');
    });

    test('failed_result_copies_error_message', () {
      expect(popupCopyTextFor(_errorResult), 'it broke');
    });

    test('successful_result_without_outputs_has_nothing_to_copy', () {
      const empty = CanonicalToolResult(ok: true, outputs: []);
      expect(popupCopyTextFor(empty), isNull);
    });
  });
}
