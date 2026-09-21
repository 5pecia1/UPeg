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

  test(
    'an_output_conversion_failure_after_a_successful_raw_read_stays_failed_past_debug_reservation_and_event_clearing',
    () async {
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
    },
  );

  test(
    'stores_the_normalized_output_type_and_a_representative_non_first_result_separately_from_the_string_source',
    () async {
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
      factory.sessions.single.readPayload =
          '{"caption":"value","amount":"42.5"}';

      final outcome = await _run(service);
      final entry = service.entryFor(_toolId)!;
      service.clearEvents(_toolId);
      final release = service.reserveDebugger(await service.ensure(_spec));

      expect(outcome.result.primaryOutputId, 'caption');
      expect(outcome.result.jsonValues, {'caption': 'value', 'amount': '42.5'});
      expect(outcome.canonicalResult, same(canonical));
      expect(entry.lastResult, same(canonical));
      expect(entry.lastResult?.primaryOutputId, 'amount');
      expect(entry.lastResult?.jsonValues, {
        'caption': 'value',
        'amount': 42.5,
      });
      expect(entry.lastResult?.outputs.last.kind, 'number');
      expect(entry.phase, ControlledEmbedSessionPhase.ready);
      release();
    },
  );

  test(
    'a_page_created_by_a_normal_run_reruns_in_the_same_state_when_the_debugger_reserves_it',
    () async {
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
    },
  );

  test(
    'concurrent_calls_for_the_same_tool_read_the_prior_result_before_writing_the_next_input',
    () async {
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
      expect(
        service.entryFor(_toolId)?.phase,
        ControlledEmbedSessionPhase.ready,
      );
      expect(factory.sessions, hasLength(1));
    },
  );

  test(
    'cancelling_a_queued_call_reports_the_cancellation_without_starting_operations_regardless_of_the_prior_run',
    () {
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
    },
  );

  test(
    'cancelling_mid_run_reports_immediately_while_blocking_the_next_call_until_the_in_flight_operation_finishes',
    () {
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
    },
  );

  test(
    'a_settings_change_on_a_running_page_is_rejected_and_the_existing_run_state_is_kept',
    () async {
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
    },
  );

  test(
    'a_session_in_use_by_the_debugger_is_not_closed_by_a_settings_change_and_is_replaced_after_the_reservation_is_released',
    () async {
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
    },
  );

  test(
    'close_wakes_running_and_waiting_calls_and_closes_the_browser_only_once',
    () async {
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
    },
  );

  test(
    'closing_while_the_page_prepares_still_wakes_waiting_calls_and_closes_the_browser_only_once',
    () {
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
    },
  );

  test(
    'a_wait_error_is_recorded_in_the_same_run_events_with_the_structured_target_info',
    () async {
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
      final waitEvent = entry.events.singleWhere(
        (event) => event.kind == 'wait',
      );
      expect(waitEvent.details?['forSelector'], '#input-ready');
      expect(waitEvent.details?['condition'], 'visible');
      expect(waitEvent.details?['timeoutMs'], 0);
      expect(entry.events.last.level, ControlledEmbedDebugLevel.error);
      expect(
        entry.events.last.details?['code'],
        kControlledEmbedWaitTimeoutCode,
      );
      expect(browser.operations, ['wait:#input-ready']);
    },
  );
}
