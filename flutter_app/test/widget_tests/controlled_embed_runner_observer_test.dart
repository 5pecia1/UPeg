import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/embed.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/widgets/controlled_embed/debug.dart';
import 'package:upeg/src/widgets/controlled_embed/runner.dart';

import '../shared/controlled_embed_debug_harness.dart';
import '../test_helpers/controlled_embed_runner_harness.dart';

const _bindings = [
  SelectorBindingDto(
    role: BindingRoleDto.input,
    field: 'q',
    selector: '#q',
    triggerAction: ControlledEmbedTriggerActionDto.click,
  ),
  SelectorBindingDto(
    role: BindingRoleDto.trigger,
    field: '',
    selector: '#go',
    triggerAction: ControlledEmbedTriggerActionDto.click,
  ),
  SelectorBindingDto(
    role: BindingRoleDto.output,
    field: 'result',
    selector: '#result',
    triggerAction: ControlledEmbedTriggerActionDto.click,
  ),
];
const _inputs = {'q': 'hello'};
const _beforeProbe =
    '[{"role":"input","field":"q","selector":"#q","matched":true,'
    '"valuePreview":"","textPreview":""}]';
const _afterProbe =
    '[{"role":"input","field":"q","selector":"#q","matched":true,'
    '"valuePreview":"hello","textPreview":""}]';
const _outputPayload = '{"result":"ok"}';

void main() {
  useStubbedExecutionScripts();

  test(
    'the_observation_callbacks_receive_events_and_probes_in_input_trigger_read_order',
    () async {
      final events = <ControlledEmbedDebugEvent>[];
      final probes = <List<ControlledEmbedSelectorProbe>>[];
      final recorder = DebugJavaScriptRecorder(
        [null, null],
        [_beforeProbe, _afterProbe, _outputPayload],
      );

      final result =
          await const ControlledEmbedRunner(settleDelay: Duration.zero).execute(
            runCommandJs: recorder.runCommandJs,
            runResultJs: recorder.runResultJs,
            bindings: _bindings,
            inputs: _inputs,
            onEvent: events.add,
            onProbes: probes.add,
          );

      expect(result.ok, isTrue);
      expect(result.jsonValues, {'result': 'ok'});
      expect(events.map((event) => event.phase), [
        ControlledEmbedDebugPhase.setup,
        ControlledEmbedDebugPhase.probe,
        ControlledEmbedDebugPhase.probe,
        ControlledEmbedDebugPhase.write,
        ControlledEmbedDebugPhase.write,
        ControlledEmbedDebugPhase.trigger,
        ControlledEmbedDebugPhase.trigger,
        ControlledEmbedDebugPhase.probe,
        ControlledEmbedDebugPhase.probe,
        ControlledEmbedDebugPhase.read,
        ControlledEmbedDebugPhase.read,
      ]);
      expect(
        events.map((event) => event.sequence),
        List.generate(events.length, (index) => index),
      );
      expect(
        events.map((event) => event.elapsedMs),
        everyElement(isNonNegative),
      );
      expect(events.first.details, {'bindingCount': 3, 'inputCount': 1});
      expect(events.last.details?['outputs'], {'result': 'ok'});
      expect(events.last.details?['outputPreview'], {'result': 'ok'});
      expect(recorder.commandCalls, ['/*WRITE*/', '/*TRIGGER*/']);
      expect(recorder.resultCalls, hasLength(3));
      expect(recorder.resultCalls.last, '/*READ*/');
      expect(probes, hasLength(2));
      expect(probes.first.single.valuePreview, isEmpty);
      expect(probes.last.single.valuePreview, 'hello');
      expect(probes.last.single.matched, isTrue);
      expect(probes.last.single.selector, '#q');
    },
  );

  for (final failure in [
    (
      label: 'an input write',
      phase: ControlledEmbedDebugPhase.write,
      commands: <Object?>[Exception('write failed')],
      results: <Object?>[_beforeProbe],
      expectedCommands: ['/*WRITE*/'],
      expectedResultCount: 1,
    ),
    (
      label: 'a trigger',
      phase: ControlledEmbedDebugPhase.trigger,
      commands: <Object?>[null, Exception('trigger failed')],
      results: <Object?>[_beforeProbe],
      expectedCommands: ['/*WRITE*/', '/*TRIGGER*/'],
      expectedResultCount: 1,
    ),
    (
      label: 'an output read',
      phase: ControlledEmbedDebugPhase.read,
      commands: <Object?>[null, null],
      results: <Object?>[_beforeProbe, _afterProbe, Exception('read failed')],
      expectedCommands: ['/*WRITE*/', '/*TRIGGER*/'],
      expectedResultCount: 3,
    ),
  ]) {
    test(
      '${failure.label} failure is recorded at its phase and stops further browser operations',
      () async {
        final recorder = DebugJavaScriptRecorder(
          failure.commands,
          failure.results,
        );
        final events = <ControlledEmbedDebugEvent>[];

        final result =
            await const ControlledEmbedRunner(
              settleDelay: Duration.zero,
            ).execute(
              runCommandJs: recorder.runCommandJs,
              runResultJs: recorder.runResultJs,
              bindings: _bindings,
              inputs: _inputs,
              onEvent: events.add,
            );

        expect(result.ok, isFalse);
        expect(result.error?.code, kControlledEmbedExecutionErrorCode);
        expect(events.last.phase, failure.phase);
        expect(events.last.level, ControlledEmbedDebugLevel.error);
        expect(events.last.message, result.errorMessage);
        expect(events.last.details?['code'], result.error?.code);
        expect(recorder.commandCalls, failure.expectedCommands);
        expect(recorder.resultCalls, hasLength(failure.expectedResultCount));
      },
    );
  }

  test(
    'with_no_bindings_to_run_only_the_setup_info_is_logged_and_the_browser_is_untouched',
    () async {
      final recorder = DebugJavaScriptRecorder([], []);
      final events = <ControlledEmbedDebugEvent>[];
      final probes = <List<ControlledEmbedSelectorProbe>>[];

      final result =
          await const ControlledEmbedRunner(settleDelay: Duration.zero).execute(
            runCommandJs: recorder.runCommandJs,
            runResultJs: recorder.runResultJs,
            bindings: [],
            inputs: {},
            onEvent: events.add,
            onProbes: probes.add,
          );

      expect(result.ok, isTrue);
      expect(result.outputs, isEmpty);
      expect(events.single.phase, ControlledEmbedDebugPhase.setup);
      expect(recorder.commandCalls, isEmpty);
      expect(recorder.resultCalls, isEmpty);
      expect(probes, isEmpty);
    },
  );

  test(
    'subscribing_only_to_probe_callbacks_still_returns_results_and_reports_selector_state',
    () async {
      final recorder = DebugJavaScriptRecorder(
        [null, null],
        [_beforeProbe, _afterProbe, _outputPayload],
      );
      final probes = <List<ControlledEmbedSelectorProbe>>[];

      final result =
          await const ControlledEmbedRunner(settleDelay: Duration.zero).execute(
            runCommandJs: recorder.runCommandJs,
            runResultJs: recorder.runResultJs,
            bindings: _bindings,
            inputs: _inputs,
            onProbes: probes.add,
          );

      expect(result.jsonValues, {'result': 'ok'});
      expect(probes.map((probe) => probe.single.valuePreview), ['', 'hello']);
    },
  );
}
