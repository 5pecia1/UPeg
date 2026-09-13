import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/features/controlled_embed/execution_bridge.dart';
import 'package:upeg/src/features/controlled_embed/session_service.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/embed.dart';
import 'package:upeg/src/rust/api/webview.dart';
import 'package:upeg/src/widgets/controlled_embed/runner.dart';

import '../shared/fake_controlled_embed_session.dart';
import '../shared/fake_webview_execution_api.dart';
import '../test_helpers/controlled_embed_runner_harness.dart';

const _toolId = 'test.webview.bridge';
const _url = 'https://example.test/bridge';
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
    selector: '#run',
    triggerAction: ControlledEmbedTriggerActionDto.click,
  ),
  SelectorBindingDto(
    role: BindingRoleDto.output,
    field: 'result',
    selector: '#result',
    triggerAction: ControlledEmbedTriggerActionDto.click,
  ),
];

WebViewExecutionRequestDto _request(
  int id, {
  List<SelectorBindingDto> bindings = _bindings,
}) => WebViewExecutionRequestDto(
  requestId: BigInt.from(id),
  toolId: _toolId,
  url: _url,
  bindings: bindings,
  settings: const ControlledEmbedSettingsDto(),
  inputs: const [('q', 'hello')],
);

void main() {
  useStubbedExecutionScripts();
  late FakeControlledEmbedSessionFactory factory;
  late ControlledEmbedSessionService service;
  late FakeWebViewExecutionApi api;
  late ControlledEmbedExecutionBridge bridge;

  setUp(() {
    executionScriptsBuilder = eventExecutionScripts;
    bindingWaitScriptBuilder = eventBindingWaitScript;
    factory = FakeControlledEmbedSessionFactory();
    service = ControlledEmbedSessionService(
      createSession: factory.create,
      normalizeResult: (_, result, _) => result,
      runner: const ControlledEmbedRunner(settleDelay: Duration.zero),
    );
    api = FakeWebViewExecutionApi();
    bridge = ControlledEmbedExecutionBridge(sessions: service, api: api);
  });
  tearDown(() async {
    await bridge.close();
    await service.close();
    await api.close();
  });

  test('요청의 입력과 설정으로 실행하고 원시 출력을 같은 요청 번호로 반환한다', () async {
    api.eventsController.add(const WebViewExecutionEventDto.ready());
    await bridge.ready;
    final completed = api.completionController.stream.first;
    api.eventsController.add(
      WebViewExecutionEventDto.execute(request: _request(31)),
    );

    final result = await completed as WebViewExecutionCompletionDto_Success;

    expect(result.outputs, [('result', '1')]);
    expect(api.completions.single.$1, api.providerId);
    expect(api.completions.single.$2, BigInt.from(31));
    expect(factory.sessions.single.operations, ['write', 'trigger', 'read']);
    expect(factory.sessions.single.url, _url);
  });

  test('실행 중 취소는 다음 조작을 막고 취소 완료를 반환한다', () async {
    await service.ensure(
      ControlledEmbedSessionSpec(toolId: ToolId.parse(_toolId), url: _url),
    );
    final browser = factory.sessions.single;
    final gate = browser.blockNextCommand();
    final completed = api.completionController.stream.first;
    api.eventsController.add(
      WebViewExecutionEventDto.execute(request: _request(32)),
    );
    await browser.commandStarted.future;

    api.eventsController.add(
      WebViewExecutionEventDto.cancel(requestId: BigInt.from(32)),
    );
    expect(await completed, isA<WebViewExecutionCompletionDto_Cancelled>());
    gate.complete();
    await Future<void>.delayed(Duration.zero);
    expect(browser.operations, ['write']);
  });

  test('바인딩 대기 실패의 역할과 대상과 제한시간을 타입으로 보존한다', () async {
    await service.ensure(
      ControlledEmbedSessionSpec(toolId: ToolId.parse(_toolId), url: _url),
    );
    factory.sessions.single.waitResponses.add(false);
    final completed = api.completionController.stream.first;
    final wait = BindingWaitDto(
      forSelector: '#ready',
      condition: BindingWaitConditionDto.visible,
      timeoutMs: BigInt.zero,
      settleMs: BigInt.zero,
      onTimeout: BindingWaitOnTimeoutDto.fail,
    );
    api.eventsController.add(
      WebViewExecutionEventDto.execute(
        request: _request(
          33,
          bindings: [
            SelectorBindingDto(
              role: BindingRoleDto.input,
              field: 'q',
              selector: '#q',
              triggerAction: ControlledEmbedTriggerActionDto.click,
              wait: wait,
            ),
            ..._bindings.skip(1),
          ],
        ),
      ),
    );

    final result = await completed as WebViewExecutionCompletionDto_WaitTimeout;
    expect(result.role, BindingRoleDto.input);
    expect(result.selector, '#q');
    expect(result.forSelector, '#ready');
    expect(result.condition, BindingWaitConditionDto.visible);
    expect(result.timeoutMs, BigInt.zero);
    expect(factory.sessions.single.triggerCount, 0);
  });

  test('공급자를 해제하면 대기 호출의 늦은 결과를 보내지 않는다', () async {
    await service.ensure(
      ControlledEmbedSessionSpec(toolId: ToolId.parse(_toolId), url: _url),
    );
    final browser = factory.sessions.single;
    final gate = browser.blockNextCommand();
    api.eventsController.add(
      WebViewExecutionEventDto.execute(request: _request(34)),
    );
    await browser.commandStarted.future;

    await bridge.close();
    gate.complete();
    await Future<void>.delayed(Duration.zero);

    expect(api.unregisterCalls, 1);
    expect(api.completions, isEmpty);
    expect(browser.operations, ['write']);
  });
}
