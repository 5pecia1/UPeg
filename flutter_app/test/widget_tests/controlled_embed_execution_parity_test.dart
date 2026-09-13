import 'package:fake_async/fake_async.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/embed.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/widgets/controlled_embed/debug.dart';
import 'package:upeg/src/widgets/controlled_embed/runner.dart';

import '../shared/controlled_embed_debug_harness.dart';
import '../test_helpers/controlled_embed_runner_harness.dart';

const _inputs = {'q': 'hello'};
const _inputSettle = Duration(milliseconds: 20);
const _triggerSettle = Duration(milliseconds: 40);

List<SelectorBindingDto> _bindings({
  BindingWaitOnTimeoutDto inputOnTimeout = BindingWaitOnTimeoutDto.fail,
  Duration inputSettle = Duration.zero,
}) => [
  for (final (role, field, selector) in [
    (BindingRoleDto.input, 'q', '#q'),
    (BindingRoleDto.trigger, '', '#go'),
    (BindingRoleDto.output, 'result', '#result'),
  ])
    SelectorBindingDto(
      role: role,
      field: field,
      selector: selector,
      triggerAction: ControlledEmbedTriggerActionDto.click,
      wait: BindingWaitDto(
        forSelector: '$selector-ready',
        condition: BindingWaitConditionDto.visible,
        timeoutMs: BigInt.zero,
        settleMs: role == BindingRoleDto.input
            ? BigInt.from(inputSettle.inMilliseconds)
            : BigInt.zero,
        onTimeout: role == BindingRoleDto.input
            ? inputOnTimeout
            : BindingWaitOnTimeoutDto.fail,
      ),
    ),
];

Future<CanonicalToolResult> _runDebug(
  FakeControlledEmbedBrowser browser,
  List<SelectorBindingDto> bindings, {
  required List<ControlledEmbedDebugEvent> events,
  Duration settleDelay = Duration.zero,
}) => ControlledEmbedRunner(settleDelay: settleDelay).execute(
  runCommandJs: browser.runCommandJs,
  runResultJs: browser.runResultJs,
  bindings: bindings,
  inputs: _inputs,
  onEvent: events.add,
  onProbes: (_) {},
);

void main() {
  useStubbedExecutionScripts();
  setUp(() {
    executionScriptsBuilder = eventExecutionScripts;
    bindingWaitScriptBuilder = eventBindingWaitScript;
  });

  test('일반 실행과 디버그는 각 바인딩을 기다린 뒤 같은 순서로 조작하고 결과를 읽는다', () async {
    const payload = '{"second":"42","first":"done"}';
    final normalBrowser = FakeControlledEmbedBrowser(
      waitResponses: [true, true, true],
      readPayload: payload,
    );
    final debugBrowser = FakeControlledEmbedBrowser(
      waitResponses: [true, true, true],
      readPayload: payload,
    );
    final events = <ControlledEmbedDebugEvent>[];
    final bindings = _bindings();

    final normal = await const ControlledEmbedRunner(settleDelay: Duration.zero)
        .execute(
          runCommandJs: normalBrowser.runCommandJs,
          runResultJs: normalBrowser.runResultJs,
          bindings: bindings,
          inputs: _inputs,
        );
    final debug = await _runDebug(debugBrowser, bindings, events: events);

    expect(normal.ok, isTrue);
    expect(debug.ok, isTrue);
    expect(normalBrowser.operations, [
      'wait:#q-ready',
      'write',
      'wait:#go-ready',
      'trigger',
      'wait:#result-ready',
      'read',
    ]);
    expect(debugBrowser.operations, normalBrowser.operations);
    expect(debug.jsonValues, normal.jsonValues);
    expect(
      debug.outputs.map((output) => output.kind),
      normal.outputs.map((output) => output.kind),
    );
    expect(debug.primaryOutputId, normal.primaryOutputId);
    expect(
      events.where((event) => event.kind == 'wait').map((event) => event.phase),
      [
        ControlledEmbedDebugPhase.write,
        ControlledEmbedDebugPhase.write,
        ControlledEmbedDebugPhase.trigger,
        ControlledEmbedDebugPhase.trigger,
        ControlledEmbedDebugPhase.read,
        ControlledEmbedDebugPhase.read,
      ],
    );
    expect(
      events.map((event) => event.sequence),
      List.generate(events.length, (index) => index),
    );
  });

  test('대기 시간이 초과되면 디버그도 조작을 중단하고 같은 오류와 대기 대상을 반환한다', () async {
    final normalBrowser = FakeControlledEmbedBrowser(waitResponses: [false]);
    final debugBrowser = FakeControlledEmbedBrowser(waitResponses: [false]);
    final events = <ControlledEmbedDebugEvent>[];
    final bindings = _bindings();
    ControlledEmbedWaitTimeoutError? timeout;

    final normal = await const ControlledEmbedRunner(settleDelay: Duration.zero)
        .execute(
          runCommandJs: normalBrowser.runCommandJs,
          runResultJs: normalBrowser.runResultJs,
          bindings: bindings,
          inputs: _inputs,
          onWaitTimeout: (error) => timeout = error,
        );
    final debug = await _runDebug(debugBrowser, bindings, events: events);

    expect(debug.ok, isFalse);
    expect(debug.error?.code, kControlledEmbedWaitTimeoutCode);
    expect(debug.error?.code, normal.error?.code);
    expect(debug.errorMessage, normal.errorMessage);
    expect(debugBrowser.operations, ['wait:#q-ready']);
    expect(debugBrowser.operations, normalBrowser.operations);
    expect(timeout?.selector, '#q');
    expect(timeout?.forSelector, '#q-ready');
    expect(timeout?.roleLabel, 'input');
    expect(timeout?.condition, BindingWaitConditionDto.visible);
    expect(timeout?.timeoutMs, 0);
    expect(events.last.phase, ControlledEmbedDebugPhase.write);
    expect(events.last.level, ControlledEmbedDebugLevel.error);
    expect(events.last.details?['code'], kControlledEmbedWaitTimeoutCode);
  });

  test('시간 초과 뒤 계속하기 정책은 일반 실행과 디버그에 동일하게 적용된다', () async {
    final normalBrowser = FakeControlledEmbedBrowser(
      waitResponses: [false, true, true],
    );
    final debugBrowser = FakeControlledEmbedBrowser(
      waitResponses: [false, true, true],
    );
    final events = <ControlledEmbedDebugEvent>[];
    final bindings = _bindings(
      inputOnTimeout: BindingWaitOnTimeoutDto.continue_,
    );

    final normal = await const ControlledEmbedRunner(settleDelay: Duration.zero)
        .execute(
          runCommandJs: normalBrowser.runCommandJs,
          runResultJs: normalBrowser.runResultJs,
          bindings: bindings,
          inputs: _inputs,
        );
    final debug = await _runDebug(debugBrowser, bindings, events: events);

    expect(normal.ok, isTrue);
    expect(debug.ok, isTrue);
    expect(debugBrowser.operations, normalBrowser.operations);
    expect(debug.jsonValues, normal.jsonValues);
    final warnings = events.where(
      (event) => event.level == ControlledEmbedDebugLevel.warning,
    );
    expect(warnings.single.kind, 'wait');
    expect(warnings.single.details?['forSelector'], '#q-ready');
  });

  test('잘못된 출력은 디버그에서도 일반 실행과 같은 파싱 오류로 반환된다', () async {
    final normalBrowser = FakeControlledEmbedBrowser(
      waitResponses: [true, true, true],
      readPayload: 'invalid JSON',
    );
    final debugBrowser = FakeControlledEmbedBrowser(
      waitResponses: [true, true, true],
      readPayload: 'invalid JSON',
    );
    final events = <ControlledEmbedDebugEvent>[];
    final bindings = _bindings();

    final normal = await const ControlledEmbedRunner(settleDelay: Duration.zero)
        .execute(
          runCommandJs: normalBrowser.runCommandJs,
          runResultJs: normalBrowser.runResultJs,
          bindings: bindings,
          inputs: _inputs,
        );
    final debug = await _runDebug(debugBrowser, bindings, events: events);

    expect(debug.ok, isFalse);
    expect(debug.error?.code, kControlledEmbedReadPayloadErrorCode);
    expect(debug.error?.code, normal.error?.code);
    expect(debug.errorMessage, normal.errorMessage);
    expect(events.last.phase, ControlledEmbedDebugPhase.read);
    expect(events.last.level, ControlledEmbedDebugLevel.error);
  });

  test('선택자 프로브가 실패해도 실제 조작과 출력은 정상 실행과 동일하다', () async {
    final normalBrowser = FakeControlledEmbedBrowser(
      waitResponses: [true, true, true],
    );
    final debugBrowser = FakeControlledEmbedBrowser(
      waitResponses: [true, true, true],
      probeFailure: Exception('probe unavailable'),
    );
    final events = <ControlledEmbedDebugEvent>[];
    final bindings = _bindings();

    final normal = await const ControlledEmbedRunner(settleDelay: Duration.zero)
        .execute(
          runCommandJs: normalBrowser.runCommandJs,
          runResultJs: normalBrowser.runResultJs,
          bindings: bindings,
          inputs: _inputs,
        );
    final debug = await _runDebug(debugBrowser, bindings, events: events);

    expect(debug.ok, isTrue);
    expect(debugBrowser.operations, normalBrowser.operations);
    expect(debug.jsonValues, normal.jsonValues);
    expect(
      events.where(
        (event) =>
            event.phase == ControlledEmbedDebugPhase.probe &&
            event.level == ControlledEmbedDebugLevel.error,
      ),
      hasLength(2),
    );
  });

  test('디버그도 바인딩 안정화와 트리거 후 대기를 모두 마친 뒤 출력을 읽는다', () {
    fakeAsync((clock) {
      final normalBrowser = FakeControlledEmbedBrowser(
        waitResponses: [true, true, true],
      );
      final debugBrowser = FakeControlledEmbedBrowser(
        waitResponses: [true, true, true],
      );
      final bindings = _bindings(inputSettle: _inputSettle);
      CanonicalToolResult? normal;
      CanonicalToolResult? debug;

      const ControlledEmbedRunner(settleDelay: _triggerSettle)
          .execute(
            runCommandJs: normalBrowser.runCommandJs,
            runResultJs: normalBrowser.runResultJs,
            bindings: bindings,
            inputs: _inputs,
          )
          .then((result) => normal = result);
      _runDebug(
        debugBrowser,
        bindings,
        events: [],
        settleDelay: _triggerSettle,
      ).then((result) => debug = result);

      clock.flushMicrotasks();
      expect(debugBrowser.operations, ['wait:#q-ready']);
      expect(debugBrowser.operations, normalBrowser.operations);
      clock.elapse(_inputSettle);
      expect(debugBrowser.operations.last, 'trigger');
      expect(debug, isNull);
      expect(normal, isNull);
      clock.elapse(_triggerSettle);
      expect(debugBrowser.operations.last, 'read');
      expect(debugBrowser.operations, normalBrowser.operations);
      expect(debug?.ok, isTrue);
      expect(normal?.ok, isTrue);
    });
  });
}
