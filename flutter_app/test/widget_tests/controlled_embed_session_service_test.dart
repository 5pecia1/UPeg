import 'dart:async';

import 'package:fake_async/fake_async.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/features/controlled_embed/session_service.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/embed.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/widgets/controlled_embed/debug.dart';
import 'package:upeg/src/widgets/controlled_embed/runner.dart';
import 'package:upeg/src/widgets/controlled_embed/settings.dart';

import '../shared/fake_controlled_embed_session.dart';
import '../test_helpers/controlled_embed_runner_harness.dart';

const _url = 'https://example.test/controlled';
const _inputs = {'q': 'hello'};
final _toolId = ToolId.parse('test.controlled.session');
final _spec = ControlledEmbedSessionSpec(toolId: _toolId, url: _url);
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

Future<ControlledEmbedSessionOutcome> _run(
  ControlledEmbedSessionService service, {
  ControlledEmbedCancellation? cancellation,
  List<SelectorBindingDto> bindings = _bindings,
}) => service.execute(
  spec: _spec,
  bindings: bindings,
  inputs: _inputs,
  cancellation: cancellation ?? ControlledEmbedCancellation(),
);

void main() {
  useStubbedExecutionScripts();
  late FakeControlledEmbedSessionFactory factory;
  late ControlledEmbedSessionService service;
  late ControlledEmbedResultNormalizer normalizeResult;

  setUp(() {
    executionScriptsBuilder = eventExecutionScripts;
    bindingWaitScriptBuilder = eventBindingWaitScript;
    factory = FakeControlledEmbedSessionFactory();
    normalizeResult = (_, result, _) => result;
    service = ControlledEmbedSessionService(
      createSession: factory.create,
      normalizeResult: (toolId, result, wait) =>
          normalizeResult(toolId, result, wait),
      runner: const ControlledEmbedRunner(settleDelay: Duration.zero),
    );
  });
  tearDown(() => service.close());

  test('원시 읽기가 성공해도 출력 변환이 실패하면 디버그 예약과 기록 지우기 뒤에도 실패를 유지한다', () async {
    const conversionError = CanonicalToolResult(
      ok: false,
      outputs: [],
      error: CanonicalToolError(
        code: 'test-output-conversion',
        message: 'Expected a numeric output.',
      ),
    );
    ToolId? normalizedTool;
    CanonicalToolResult? rawResult;
    normalizeResult = (toolId, result, wait) {
      normalizedTool = toolId;
      rawResult = result;
      expect(wait, isNull);
      return conversionError;
    };
    await service.ensure(_spec);
    factory.sessions.single.readPayload = '{"result":"not-a-number"}';

    final outcome = await _run(service);
    final entry = service.entryFor(_toolId)!;

    expect(normalizedTool, _toolId);
    expect(rawResult?.ok, isTrue);
    expect(rawResult?.jsonValues, {'result': 'not-a-number'});
    expect(outcome.result, same(rawResult));
    expect(outcome.canonicalResult, same(conversionError));
    expect(entry.lastResult, same(conversionError));
    expect(entry.phase, ControlledEmbedSessionPhase.failed);
    expect(entry.events.last.kind, 'output');
    expect(entry.events.last.level, ControlledEmbedDebugLevel.error);
    expect(entry.events.last.message, conversionError.error?.message);

    final debugEntry = await service.ensure(_spec);
    final release = service.reserveDebugger(debugEntry);
    service.clearEvents(_toolId);
    await service.ensure(
      ControlledEmbedSessionSpec(
        toolId: ToolId.parse('test.other.session'),
        url: _url,
      ),
    );
    release();

    expect(debugEntry, same(entry));
    expect(entry.lastResult, same(conversionError));
    expect(entry.lastResult?.outputs, isEmpty);
    expect(entry.phase, ControlledEmbedSessionPhase.failed);
    expect(entry.events, isEmpty);
  });

  test('문자열 원본과 별도로 정규화한 출력 타입과 첫 항목이 아닌 대표 결과를 저장한다', () async {
    const canonical = CanonicalToolResult(
      ok: true,
      primaryOutputId: 'amount',
      outputs: [
        CanonicalOutputEntry(
          id: 'caption',
          kind: 'string',
          value: CanonicalOutputValue.string(value: 'value'),
        ),
        CanonicalOutputEntry(
          id: 'amount',
          kind: 'number',
          value: CanonicalOutputValue.number(value: 42.5),
        ),
      ],
    );
    normalizeResult = (_, _, _) => canonical;
    await service.ensure(_spec);
    factory.sessions.single.readPayload = '{"caption":"value","amount":"42.5"}';

    final outcome = await _run(service);
    final entry = service.entryFor(_toolId)!;
    service.clearEvents(_toolId);
    final release = service.reserveDebugger(await service.ensure(_spec));

    expect(outcome.result.primaryOutputId, 'caption');
    expect(outcome.result.jsonValues, {'caption': 'value', 'amount': '42.5'});
    expect(outcome.canonicalResult, same(canonical));
    expect(entry.lastResult, same(canonical));
    expect(entry.lastResult?.primaryOutputId, 'amount');
    expect(entry.lastResult?.jsonValues, {'caption': 'value', 'amount': 42.5});
    expect(entry.lastResult?.outputs.last.kind, 'number');
    expect(entry.phase, ControlledEmbedSessionPhase.ready);
    release();
  });

  test('일반 실행에서 만든 페이지를 디버거가 예약해도 같은 상태로 다시 실행한다', () async {
    final first = await _run(service);
    final originalEntry = service.entryFor(_toolId)!;
    final debugEntry = await service.ensure(
      ControlledEmbedSessionSpec(toolId: _toolId, url: _url),
    );
    final release = service.reserveDebugger(debugEntry);

    final second = await _run(service);

    expect(identical(originalEntry, debugEntry), isTrue);
    expect(factory.sessions, hasLength(1));
    expect(first.result.jsonValues, {'result': '1'});
    expect(second.result.jsonValues, {'result': '2'});
    expect(debugEntry.displayedInDebugger, isTrue);
    expect(factory.sessions.single.closeCalls, 0);
    expect(() => service.reserveDebugger(debugEntry), throwsStateError);
    release();
    release();
    expect(debugEntry.displayedInDebugger, isFalse);
    expect(identical(service.entryFor(_toolId), originalEntry), isTrue);
  });

  test('같은 도구의 동시 호출은 앞 실행의 결과를 읽은 뒤 다음 입력을 쓴다', () async {
    await service.ensure(_spec);
    final browser = factory.sessions.single;
    final gate = browser.blockNextCommand();
    final first = _run(service);
    await browser.commandStarted.future;

    final second = _run(service);
    await Future<void>.delayed(Duration.zero);
    expect(browser.operations, ['write']);
    expect(
      service.entryFor(_toolId)?.phase,
      ControlledEmbedSessionPhase.running,
    );
    gate.complete();
    final results = await Future.wait([first, second]);

    expect(browser.operations, [
      'write',
      'trigger',
      'read',
      'write',
      'trigger',
      'read',
    ]);
    expect(results.map((outcome) => outcome.result.jsonValues), [
      {'result': '1'},
      {'result': '2'},
    ]);
    expect(service.entryFor(_toolId)?.phase, ControlledEmbedSessionPhase.ready);
    expect(factory.sessions, hasLength(1));
  });

  test('대기열의 호출을 취소하면 앞 실행과 무관하게 취소를 알리고 조작을 시작하지 않는다', () {
    fakeAsync((clock) {
      unawaited(service.ensure(_spec));
      clock.flushMicrotasks();
      final browser = factory.sessions.single;
      final gate = browser.blockNextCommand();
      unawaited(_run(service));
      clock.flushMicrotasks();
      final cancellation = ControlledEmbedCancellation();
      Object? queuedError;
      unawaited(
        _run(
          service,
          cancellation: cancellation,
        ).then<void>((_) {}, onError: (Object error) => queuedError = error),
      );

      cancellation.cancel();
      clock.flushMicrotasks();

      final errorBeforeRelease = queuedError;
      final operationsBeforeRelease = List.of(browser.operations);
      gate.complete();
      clock.elapse(Duration.zero);
      expect(errorBeforeRelease, isA<ControlledEmbedCancelled>());
      expect(operationsBeforeRelease, ['write']);
      expect(browser.operations, ['write', 'trigger', 'read']);
    });
  });

  test('실행 중 취소를 즉시 알리면서 처리 중인 조작이 끝날 때까지 다음 호출을 막는다', () {
    fakeAsync((clock) {
      unawaited(service.ensure(_spec));
      clock.flushMicrotasks();
      final browser = factory.sessions.single;
      final gate = browser.blockNextCommand();
      final cancellation = ControlledEmbedCancellation();
      Object? runningError;
      unawaited(
        _run(
          service,
          cancellation: cancellation,
        ).then<void>((_) {}, onError: (Object error) => runningError = error),
      );
      clock.flushMicrotasks();

      cancellation.cancel();
      unawaited(_run(service));
      clock.flushMicrotasks();

      final errorBeforeRelease = runningError;
      final operationsBeforeRelease = List.of(browser.operations);
      gate.complete();
      clock.elapse(Duration.zero);
      expect(errorBeforeRelease, isA<ControlledEmbedCancelled>());
      expect(operationsBeforeRelease, ['write']);
      expect(browser.operations, ['write', 'write', 'trigger', 'read']);
      expect(browser.triggerCount, 1);
    });
  });

  test('실행 중인 페이지의 설정 변경을 거절하고 기존 실행 상태를 유지한다', () async {
    await service.ensure(_spec);
    final browser = factory.sessions.single;
    final gate = browser.blockNextCommand();
    final operation = _run(service);
    await browser.commandStarted.future;
    final changed = ControlledEmbedSessionSpec(
      toolId: _toolId,
      url: _url,
      settings: const ResolvedBrowserSettings(
        hasUserAgentOverride: true,
        userAgent: 'test-agent',
      ),
    );

    await expectLater(service.ensure(changed), throwsStateError);

    expect(factory.sessions, hasLength(1));
    expect(browser.closeCalls, 0);
    expect(service.entryFor(_toolId)?.spec, _spec);
    gate.complete();
    expect((await operation).result.ok, isTrue);
  });

  test('디버거가 사용 중인 세션은 설정 변경으로 닫히지 않고 예약 해제 후 교체된다', () async {
    final entry = await service.ensure(_spec);
    final originalBrowser = factory.sessions.single;
    final release = service.reserveDebugger(entry);
    final changed = ControlledEmbedSessionSpec(
      toolId: _toolId,
      url: _url,
      settings: const ResolvedBrowserSettings(
        hasUserAgentOverride: false,
        viewportSize: ViewPortPresetSizes.tablet,
      ),
    );

    await expectLater(service.ensure(changed), throwsStateError);
    expect(originalBrowser.closeCalls, 0);
    expect(entry.displayedInDebugger, isTrue);
    release();
    final replacement = await service.ensure(changed);

    expect(factory.sessions, hasLength(2));
    expect(originalBrowser.closeCalls, 1);
    expect(replacement.browser.viewportSize, ViewPortPresetSizes.tablet);
    expect(replacement.phase, ControlledEmbedSessionPhase.ready);
  });

  test('종료는 실행 중 호출과 대기 호출을 깨우고 브라우저를 한 번만 닫는다', () async {
    await service.ensure(_spec);
    final browser = factory.sessions.single;
    browser.blockNextCommand();
    final first = _run(service);
    await browser.commandStarted.future;
    final second = _run(service);
    final firstCancelled = expectLater(
      first,
      throwsA(isA<ControlledEmbedCancelled>()),
    );
    final secondCancelled = expectLater(
      second,
      throwsA(isA<ControlledEmbedCancelled>()),
    );

    await service.close();
    await service.close();
    await Future.wait([firstCancelled, secondCancelled]);

    expect(browser.closeCalls, 1);
    expect(browser.operations, ['write']);
    expect(service.entries, isEmpty);
    await expectLater(service.ensure(_spec), throwsStateError);
    await expectLater(_run(service), throwsStateError);
  });

  test('페이지 준비 중 종료해도 대기 호출이 깨어나고 브라우저는 한 번만 닫힌다', () {
    fakeAsync((clock) {
      factory.nextReadyGate = Completer<void>();
      Object? openingError;
      unawaited(
        service
            .ensure(_spec)
            .then<void>(
              (_) {},
              onError: (Object error) => openingError = error,
            ),
      );
      clock.flushMicrotasks();
      final browser = factory.sessions.single;
      expect(
        service.entryFor(_toolId)?.phase,
        ControlledEmbedSessionPhase.loading,
      );

      unawaited(service.close());
      clock.flushMicrotasks();

      expect(openingError, isStateError);
      expect(browser.closeCalls, 1);
      expect(service.entries, isEmpty);
    });
  });

  test('대기 오류는 구조화된 대상 정보와 같은 실행의 이벤트에 함께 남는다', () async {
    await service.ensure(_spec);
    final browser = factory.sessions.single;
    browser.waitResponses.add(false);
    final bindings = [
      inputBindingWithWait(
        forSelector: '#input-ready',
        condition: BindingWaitConditionDto.visible,
        timeoutMs: BigInt.zero,
      ),
      _bindings.last,
    ];

    final outcome = await _run(service, bindings: bindings);
    final entry = service.entryFor(_toolId)!;

    expect(outcome.result.ok, isFalse);
    expect(outcome.waitTimeout?.roleLabel, 'input');
    expect(outcome.waitTimeout?.selector, '#q');
    expect(outcome.waitTimeout?.forSelector, '#input-ready');
    expect(outcome.waitTimeout?.condition, BindingWaitConditionDto.visible);
    expect(outcome.waitTimeout?.timeoutMs, 0);
    expect(entry.lastResult, same(outcome.result));
    expect(entry.phase, ControlledEmbedSessionPhase.failed);
    final waitEvent = entry.events.singleWhere((event) => event.kind == 'wait');
    expect(waitEvent.details?['forSelector'], '#input-ready');
    expect(waitEvent.details?['condition'], 'visible');
    expect(waitEvent.details?['timeoutMs'], 0);
    expect(entry.events.last.level, ControlledEmbedDebugLevel.error);
    expect(entry.events.last.details?['code'], kControlledEmbedWaitTimeoutCode);
    expect(browser.operations, ['wait:#input-ready']);
  });
}
