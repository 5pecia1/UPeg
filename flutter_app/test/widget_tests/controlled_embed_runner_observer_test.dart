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

  test('관측 콜백은 실제 입력과 트리거와 읽기 순서대로 이벤트와 프로브를 받는다', () async {
    final events = <ControlledEmbedDebugEvent>[];
    final probes = <List<ControlledEmbedSelectorProbe>>[];
    final recorder = DebugJavaScriptRecorder(
      [null, null],
      [_beforeProbe, _afterProbe, _outputPayload],
    );

    final result = await const ControlledEmbedRunner(settleDelay: Duration.zero)
        .execute(
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
    expect(events.map((event) => event.elapsedMs), everyElement(isNonNegative));
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
  });

  for (final failure in [
    (
      label: '입력 쓰기',
      phase: ControlledEmbedDebugPhase.write,
      commands: <Object?>[Exception('write failed')],
      results: <Object?>[_beforeProbe],
      expectedCommands: ['/*WRITE*/'],
      expectedResultCount: 1,
    ),
    (
      label: '트리거',
      phase: ControlledEmbedDebugPhase.trigger,
      commands: <Object?>[null, Exception('trigger failed')],
      results: <Object?>[_beforeProbe],
      expectedCommands: ['/*WRITE*/', '/*TRIGGER*/'],
      expectedResultCount: 1,
    ),
    (
      label: '출력 읽기',
      phase: ControlledEmbedDebugPhase.read,
      commands: <Object?>[null, null],
      results: <Object?>[_beforeProbe, _afterProbe, Exception('read failed')],
      expectedCommands: ['/*WRITE*/', '/*TRIGGER*/'],
      expectedResultCount: 3,
    ),
  ]) {
    test('${failure.label} 실패는 해당 단계에 기록하고 후속 브라우저 조작을 중단한다', () async {
      final recorder = DebugJavaScriptRecorder(
        failure.commands,
        failure.results,
      );
      final events = <ControlledEmbedDebugEvent>[];

      final result =
          await const ControlledEmbedRunner(settleDelay: Duration.zero).execute(
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
    });
  }

  test('실행할 바인딩이 없으면 준비 정보만 기록하고 브라우저를 조작하지 않는다', () async {
    final recorder = DebugJavaScriptRecorder([], []);
    final events = <ControlledEmbedDebugEvent>[];
    final probes = <List<ControlledEmbedSelectorProbe>>[];

    final result = await const ControlledEmbedRunner(settleDelay: Duration.zero)
        .execute(
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
  });

  test('프로브 콜백만 구독해도 결과는 유지하고 선택자 상태를 전달한다', () async {
    final recorder = DebugJavaScriptRecorder(
      [null, null],
      [_beforeProbe, _afterProbe, _outputPayload],
    );
    final probes = <List<ControlledEmbedSelectorProbe>>[];

    final result = await const ControlledEmbedRunner(settleDelay: Duration.zero)
        .execute(
          runCommandJs: recorder.runCommandJs,
          runResultJs: recorder.runResultJs,
          bindings: _bindings,
          inputs: _inputs,
          onProbes: probes.add,
        );

    expect(result.jsonValues, {'result': 'ok'});
    expect(probes.map((probe) => probe.single.valuePreview), ['', 'hello']);
  });
}
