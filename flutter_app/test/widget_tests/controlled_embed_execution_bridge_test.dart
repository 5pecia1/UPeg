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
final _bridgeSpec = ControlledEmbedSessionSpec(
  toolId: ToolId.parse(_toolId),
  url: _url,
);
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
  String? boardKey,
  String? pinId,
}) => WebViewExecutionRequestDto(
  requestId: BigInt.from(id),
  toolId: _toolId,
  boardKey: boardKey,
  pinId: pinId,
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

  test(
    'runs_with_the_request_inputs_and_settings_and_returns_raw_outputs_under_the_same_request_id',
    () async {
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
    },
  );

  test(
    'should keep distinct browser sessions for two pins of one tool',
    () async {
      api.eventsController.add(const WebViewExecutionEventDto.ready());
      await bridge.ready;
      final completed = api.completionController.stream.take(2).toList();
      api.eventsController.add(
        WebViewExecutionEventDto.execute(
          request: _request(41, boardKey: 'board-a', pinId: 'pin-one'),
        ),
      );
      api.eventsController.add(
        WebViewExecutionEventDto.execute(
          request: _request(42, boardKey: 'board-a', pinId: 'pin-two'),
        ),
      );

      await completed;

      expect(factory.sessions, hasLength(2));
      expect(
        service.entryForPin((
          BoardKey.parse('board-a'),
          PinId.parse('pin-one'),
        )),
        isNotNull,
      );
      expect(
        service.entryForPin((
          BoardKey.parse('board-a'),
          PinId.parse('pin-two'),
        )),
        isNotNull,
      );
      expect(service.entryForTool(ToolId.parse(_toolId)), isNull);
    },
  );

  test(
    'should keep the tool session for a request without pin context',
    () async {
      api.eventsController.add(const WebViewExecutionEventDto.ready());
      await bridge.ready;
      final completed = api.completionController.stream.first;
      api.eventsController.add(
        WebViewExecutionEventDto.execute(request: _request(43)),
      );

      await completed;

      expect(factory.sessions, hasLength(1));
      expect(service.entryForTool(ToolId.parse(_toolId)), isNotNull);
    },
  );

  test(
    'should reject incomplete pin context before opening a browser',
    () async {
      api.eventsController.add(const WebViewExecutionEventDto.ready());
      await bridge.ready;
      final completed = api.completionController.stream.first;
      api.eventsController.add(
        WebViewExecutionEventDto.execute(
          request: _request(44, boardKey: 'board-a'),
        ),
      );

      expect(await completed, isA<WebViewExecutionCompletionDto_Failed>());
      expect(factory.sessions, isEmpty);
    },
  );

  test(
    'cancelling_mid_run_blocks_the_next_operation_and_returns_a_cancel_completion',
    () async {
      await service.ensure(_bridgeSpec);
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
    },
  );

  test(
    'a_binding_wait_failure_preserves_role_target_and_timeout_as_typed_fields',
    () async {
      await service.ensure(_bridgeSpec);
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

      final result =
          await completed as WebViewExecutionCompletionDto_WaitTimeout;
      expect(result.role, BindingRoleDto.input);
      expect(result.selector, '#q');
      expect(result.forSelector, '#ready');
      expect(result.condition, BindingWaitConditionDto.visible);
      expect(result.timeoutMs, BigInt.zero);
      expect(factory.sessions.single.triggerCount, 0);
    },
  );

  test(
    'disposing_the_provider_drops_the_late_result_of_an_in_flight_call',
    () async {
      await service.ensure(_bridgeSpec);
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
    },
  );
}
