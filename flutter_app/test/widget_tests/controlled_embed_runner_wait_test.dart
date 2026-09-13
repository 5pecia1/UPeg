import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/embed.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/widgets/controlled_embed/runner.dart';
import '../test_helpers/controlled_embed_runner_harness.dart';

void main() {
  useStubbedExecutionScripts();

  group('대기_실행_행동 (Wait Execution)', () {
    test('대기_성공_후_Read가_실행되고_outputs를_반환한다', () async {
      // Wait returns false first, then true (polling succeeds).
      final recorder = WaitJsRecorder(
        commandResponses: <Object?>[null],
        // write phase: false→true (2 polls succeed), trigger: none, read: false checked
        waitResponses: [false, true, true],
        resultResponses: <Object?>['{"result":"done"}'],
      );
      final runner = ControlledEmbedRunner(settleDelay: emptySettle);
      final result = await runner.execute(
        runCommandJs: recorder.runCommandJs,
        runResultJs: recorder.runResultJs,
        runWaitJs: recorder.runWaitJs,
        bindings: [
          inputBindingWithWait(timeoutMs: big5000),
          SelectorBindingDto(
            role: BindingRoleDto.trigger,
            field: '',
            selector: 'button',
            triggerAction: ControlledEmbedTriggerActionDto.click,
          ),
          SelectorBindingDto(
            role: BindingRoleDto.output,
            field: 'result',
            selector: '#result',
            triggerAction: ControlledEmbedTriggerActionDto.click,
          ),
        ],
        inputs: {'q': 'test'}, // Per-binding flow requires matching input
      );

      expect(result.ok, isTrue);
      expect(outputTextById(result), {'result': 'done'});
      expect(recorder.waitCalls, isNotEmpty);
      // Stub generates combined wait scripts; verify at least 1 call
      expect(recorder.waitCalls.length, greaterThanOrEqualTo(1));
      expect(recorder.commandCalls, ['/*WRITE*/', '/*TRIGGER*/']);
      expect(recorder.resultCalls, ['/*READ*/']);
    });

    test('대기_fail_timeout_이후_canonical_wait_timeout_error를_반환한다', () async {
      // Wait always returns false, timeout after 100ms.
      final recorder = WaitJsRecorder(
        commandResponses: <Object?>[],
        waitResponses: [false, false, false],
        resultResponses: <Object?>[],
      );
      final runner = ControlledEmbedRunner(settleDelay: emptySettle);
      final result = await runner.execute(
        runCommandJs: recorder.runCommandJs,
        runResultJs: recorder.runResultJs,
        runWaitJs: recorder.runWaitJs,
        bindings: [
          inputBindingWithWait(
            timeoutMs: big100,
            onTimeout: BindingWaitOnTimeoutDto.fail,
          ),
        ],
        inputs: {'q': 'test'}, // Per-binding flow requires matching input
      );

      expect(result.ok, isFalse);
      expect(result.error, isNotNull);
      expect(result.error!.code, kControlledEmbedWaitTimeoutCode);
      expect(result.errorMessage, contains('timed out'));
      expect(result.errorMessage, contains('input'));
    });

    test('대기_continue_timeout은_wait_없이_기존_실행을_진행한다', () async {
      // Wait always returns false, but on_timeout=continue.
      // Use timeout_ms=0 for immediate single check in unit test.
      final recorder = WaitJsRecorder(
        commandResponses: <Object?>[null],
        waitResponses: [false],
        resultResponses: <Object?>['{"out":"goes on"}'],
      );
      final runner = ControlledEmbedRunner(settleDelay: emptySettle);
      final result = await runner.execute(
        runCommandJs: recorder.runCommandJs,
        runResultJs: recorder.runResultJs,
        runWaitJs: recorder.runWaitJs,
        bindings: [
          inputBindingWithWait(
            timeoutMs: big0,
            onTimeout: BindingWaitOnTimeoutDto.continue_,
          ),
          SelectorBindingDto(
            role: BindingRoleDto.trigger,
            field: '',
            selector: 'button',
            triggerAction: ControlledEmbedTriggerActionDto.click,
          ),
          SelectorBindingDto(
            role: BindingRoleDto.output,
            field: 'out',
            selector: '#out',
            triggerAction: ControlledEmbedTriggerActionDto.click,
          ),
        ],
        inputs: {'q': 'test'}, // Per-binding flow requires matching input
      );

      expect(result.ok, isTrue);
      expect(outputTextById(result), {'out': 'goes on'});
    });

    test('timeout_0은_single_immediate_체크만_수행한다', () async {
      final recorder = WaitJsRecorder(
        commandResponses: <Object?>[null],
        // With timeout_ms=0, only one wait check is performed.
        waitResponses: [true],
        resultResponses: <Object?>['{"x":"immediate"}'],
      );
      final runner = ControlledEmbedRunner(settleDelay: emptySettle);
      final result = await runner.execute(
        runCommandJs: recorder.runCommandJs,
        runResultJs: recorder.runResultJs,
        runWaitJs: recorder.runWaitJs,
        bindings: [
          inputBindingWithWait(
            timeoutMs: big0,
            onTimeout: BindingWaitOnTimeoutDto.fail,
          ),
          SelectorBindingDto(
            role: BindingRoleDto.trigger,
            field: '',
            selector: 'button',
            triggerAction: ControlledEmbedTriggerActionDto.click,
          ),
          SelectorBindingDto(
            role: BindingRoleDto.output,
            field: 'x',
            selector: '#x',
            triggerAction: ControlledEmbedTriggerActionDto.click,
          ),
        ],
        inputs: {'q': 'test'}, // Per-binding flow requires matching input
      );

      expect(result.ok, isTrue);
      expect(outputTextById(result), {'x': 'immediate'});
      // Only 1 check, no polling with timeout=0
      expect(recorder.waitCalls, hasLength(1));
    });

    test('timeout_0에서_바로_실패하면_wait_timeout_error를_반환한다', () async {
      final recorder = WaitJsRecorder(
        commandResponses: <Object?>[],
        waitResponses: [false], // Immediate not-ready
        resultResponses: <Object?>[],
      );
      final runner = ControlledEmbedRunner(settleDelay: emptySettle);
      final result = await runner.execute(
        runCommandJs: recorder.runCommandJs,
        runResultJs: recorder.runResultJs,
        runWaitJs: recorder.runWaitJs,
        bindings: [
          inputBindingWithWait(
            timeoutMs: big0,
            onTimeout: BindingWaitOnTimeoutDto.fail,
          ),
        ],
        inputs: {'q': 'test'}, // Per-binding flow requires matching input
      );

      expect(result.ok, isFalse);
      expect(result.error!.code, kControlledEmbedWaitTimeoutCode);
    });

    test('대기_없음_하위호환_기존_동작은_변경되지_않는다', () async {
      // Same as existing test: no wait → normal write/trigger/read.
      final recorder = RunJsRecorder(
        <Object?>[null, null],
        <Object?>['{"intro":"hi"}'],
      );
      final result = await const ControlledEmbedRunner(settleDelay: emptySettle)
          .execute(
            runCommandJs: recorder.runCommandJs,
            runResultJs: recorder.runResultJs,
            bindings: const [
              SelectorBindingDto(
                role: BindingRoleDto.input,
                field: 'q',
                selector: '#q',
                triggerAction: ControlledEmbedTriggerActionDto.click,
              ),
              SelectorBindingDto(
                role: BindingRoleDto.trigger,
                field: '',
                selector: 'button',
                triggerAction: ControlledEmbedTriggerActionDto.click,
              ),
              SelectorBindingDto(
                role: BindingRoleDto.output,
                field: 'intro',
                selector: '#intro',
                triggerAction: ControlledEmbedTriggerActionDto.click,
              ),
            ],
            inputs: const {'q': 'hello'},
          );
      expect(recorder.commandCalls, ['/*WRITE*/', '/*TRIGGER*/']);
      expect(recorder.resultCalls, ['/*READ*/']);
      expect(result.ok, isTrue);
      expect(outputTextById(result), {'intro': 'hi'});
    });
  });

  group('per-binding wait execution (바인딩별 개별 대기)', () {
    test('동일_role의_여러_바인딩이_각각_자신의_대기를_실행한다', () async {
      // Two input bindings, each with own wait. Wait responses are per-call.
      // First binding's wait: false->true (2 calls)
      // Second binding's wait: true (1 call, immediate)
      final recorder = WaitJsRecorder(
        commandResponses: <Object?>[null, null], // two input writes
        waitResponses: [false, true, true],
        resultResponses: <Object?>['{"result":"ok"}'],
      );
      final runner = ControlledEmbedRunner(settleDelay: emptySettle);
      final result = await runner.execute(
        runCommandJs: recorder.runCommandJs,
        runResultJs: recorder.runResultJs,
        runWaitJs: recorder.runWaitJs,
        bindings: [
          SelectorBindingDto(
            role: BindingRoleDto.input,
            field: 'q1',
            selector: '#q1',
            triggerAction: ControlledEmbedTriggerActionDto.click,
            wait: BindingWaitDto(
              forSelector: '#ready1',
              condition: BindingWaitConditionDto.exists,
              timeoutMs: big100,
              settleMs: big0,
              onTimeout: BindingWaitOnTimeoutDto.fail,
            ),
          ),
          SelectorBindingDto(
            role: BindingRoleDto.input,
            field: 'q2',
            selector: '#q2',
            triggerAction: ControlledEmbedTriggerActionDto.click,
            wait: BindingWaitDto(
              forSelector: '#ready2',
              condition: BindingWaitConditionDto.exists,
              timeoutMs: big100,
              settleMs: big0,
              onTimeout: BindingWaitOnTimeoutDto.fail,
            ),
          ),
          SelectorBindingDto(
            role: BindingRoleDto.trigger,
            field: '',
            selector: 'button',
            triggerAction: ControlledEmbedTriggerActionDto.click,
          ),
          SelectorBindingDto(
            role: BindingRoleDto.output,
            field: 'result',
            selector: '#result',
            triggerAction: ControlledEmbedTriggerActionDto.click,
          ),
        ],
        inputs: {'q1': 'hello', 'q2': 'world'},
      );

      expect(result.ok, isTrue);
      expect(outputTextById(result), {'result': 'ok'});
      // 3 wait calls total: 2 for first binding (retry), 1 for second
      expect(recorder.waitCalls, hasLength(3));
    });

    test('continue_바인딩의_timeout이_fail_바인딩에_영향을_주지_않는다', () async {
      // Two input bindings: first has continue (will timeout), second has fail (will succeed)
      // First binding timeout should NOT throw — it continues.
      // Second binding wait succeeds immediately.
      final recorder = WaitJsRecorder(
        commandResponses: <Object?>[null, null], // two input writes
        waitResponses: [
          false,
          true,
        ], // first times out (timeout=0), second succeeds
        resultResponses: <Object?>['{"out":"continued"}'],
      );
      final runner = ControlledEmbedRunner(settleDelay: emptySettle);
      final result = await runner.execute(
        runCommandJs: recorder.runCommandJs,
        runResultJs: recorder.runResultJs,
        runWaitJs: recorder.runWaitJs,
        bindings: [
          // First binding: on_timeout=continue, will NOT throw even though it times out
          SelectorBindingDto(
            role: BindingRoleDto.input,
            field: 'a',
            selector: '#a',
            triggerAction: ControlledEmbedTriggerActionDto.click,
            wait: BindingWaitDto(
              forSelector: '#never-ready',
              condition: BindingWaitConditionDto.exists,
              timeoutMs: big0, // immediate check
              settleMs: big0,
              onTimeout: BindingWaitOnTimeoutDto.continue_,
            ),
          ),
          // Second binding: on_timeout=fail, but it succeeds so no issue
          SelectorBindingDto(
            role: BindingRoleDto.input,
            field: 'b',
            selector: '#b',
            triggerAction: ControlledEmbedTriggerActionDto.click,
            wait: BindingWaitDto(
              forSelector: '#ready',
              condition: BindingWaitConditionDto.exists,
              timeoutMs: big100,
              settleMs: big0,
              onTimeout: BindingWaitOnTimeoutDto.fail,
            ),
          ),
          SelectorBindingDto(
            role: BindingRoleDto.output,
            field: 'out',
            selector: '#out',
            triggerAction: ControlledEmbedTriggerActionDto.click,
          ),
        ],
        inputs: {'a': '1', 'b': '2'},
      );

      // Should succeed — the continue binding's timeout did not throw.
      expect(result.ok, isTrue);
      expect(outputTextById(result), {'out': 'continued'});
    });

    test('짧은_timeout은_initial_check_후_추가_poll없이_fail한다', () async {
      executionScriptsBuilder = eventExecutionScripts;
      bindingWaitScriptBuilder = eventBindingWaitScript;
      final recorder = EventJsRecorder(waitResponses: [false]);
      final runner = ControlledEmbedRunner(
        settleDelay: emptySettle,
        waitPollInterval: Duration(milliseconds: big50.toInt()),
      );

      final result = await runner.execute(
        runCommandJs: recorder.runCommandJs,
        runResultJs: recorder.runResultJs,
        runWaitJs: recorder.runWaitJs,
        bindings: [
          SelectorBindingDto(
            role: BindingRoleDto.input,
            field: 'q',
            selector: '#q',
            triggerAction: ControlledEmbedTriggerActionDto.click,
            wait: BindingWaitDto(
              forSelector: '#ready',
              condition: BindingWaitConditionDto.exists,
              timeoutMs: big1,
              settleMs: big0,
              onTimeout: BindingWaitOnTimeoutDto.fail,
            ),
          ),
        ],
        inputs: {'q': 'hello'},
      );

      expect(result.ok, isFalse);
      expect(result.error!.code, kControlledEmbedWaitTimeoutCode);
      expect(recorder.events, ['wait:#ready']);
    });

    test('바인딩별_wait_write_trigger_output_read_순서를_보존한다', () async {
      executionScriptsBuilder = eventExecutionScripts;
      bindingWaitScriptBuilder = eventBindingWaitScript;
      final recorder = EventJsRecorder(
        waitResponses: [true, true, true],
        resultResponses: ['{"out":"done"}'],
      );
      final runner = ControlledEmbedRunner(settleDelay: emptySettle);

      final result = await runner.execute(
        runCommandJs: recorder.runCommandJs,
        runResultJs: recorder.runResultJs,
        runWaitJs: recorder.runWaitJs,
        bindings: [
          SelectorBindingDto(
            role: BindingRoleDto.input,
            field: 'q',
            selector: '#q',
            triggerAction: ControlledEmbedTriggerActionDto.click,
            wait: BindingWaitDto(
              forSelector: '#input-ready',
              condition: BindingWaitConditionDto.exists,
              timeoutMs: big100,
              settleMs: big0,
              onTimeout: BindingWaitOnTimeoutDto.fail,
            ),
          ),
          SelectorBindingDto(
            role: BindingRoleDto.trigger,
            field: '',
            selector: '#go',
            triggerAction: ControlledEmbedTriggerActionDto.click,
            wait: BindingWaitDto(
              forSelector: '#trigger-ready',
              condition: BindingWaitConditionDto.visible,
              timeoutMs: big100,
              settleMs: big0,
              onTimeout: BindingWaitOnTimeoutDto.fail,
            ),
          ),
          SelectorBindingDto(
            role: BindingRoleDto.output,
            field: 'out',
            selector: '#out',
            triggerAction: ControlledEmbedTriggerActionDto.click,
            wait: BindingWaitDto(
              forSelector: '#output-ready',
              condition: BindingWaitConditionDto.exists,
              timeoutMs: big100,
              settleMs: big0,
              onTimeout: BindingWaitOnTimeoutDto.fail,
            ),
          ),
        ],
        inputs: {'q': 'hello'},
      );

      expect(result.ok, isTrue);
      expect(outputTextById(result), {'out': 'done'});
      expect(recorder.events, [
        'wait:#input-ready',
        'command:write',
        'wait:#trigger-ready',
        'command:trigger',
        'wait:#output-ready',
        'result:read',
      ]);
    });

    test('continue_timeout_후_다음_fail_timeout은_실패하고_read를_실행하지_않는다', () async {
      executionScriptsBuilder = eventExecutionScripts;
      bindingWaitScriptBuilder = eventBindingWaitScript;
      final recorder = EventJsRecorder(waitResponses: [false, false]);
      final runner = ControlledEmbedRunner(settleDelay: emptySettle);

      final result = await runner.execute(
        runCommandJs: recorder.runCommandJs,
        runResultJs: recorder.runResultJs,
        runWaitJs: recorder.runWaitJs,
        bindings: [
          SelectorBindingDto(
            role: BindingRoleDto.input,
            field: 'a',
            selector: '#a',
            triggerAction: ControlledEmbedTriggerActionDto.click,
            wait: BindingWaitDto(
              forSelector: '#continue-ready',
              condition: BindingWaitConditionDto.exists,
              timeoutMs: big0,
              settleMs: big0,
              onTimeout: BindingWaitOnTimeoutDto.continue_,
            ),
          ),
          SelectorBindingDto(
            role: BindingRoleDto.input,
            field: 'b',
            selector: '#b',
            triggerAction: ControlledEmbedTriggerActionDto.click,
            wait: BindingWaitDto(
              forSelector: '#fail-ready',
              condition: BindingWaitConditionDto.exists,
              timeoutMs: big0,
              settleMs: big0,
              onTimeout: BindingWaitOnTimeoutDto.fail,
            ),
          ),
          SelectorBindingDto(
            role: BindingRoleDto.output,
            field: 'out',
            selector: '#out',
            triggerAction: ControlledEmbedTriggerActionDto.click,
          ),
        ],
        inputs: {'a': 'continue', 'b': 'fail'},
      );

      expect(result.ok, isFalse);
      expect(result.error!.code, kControlledEmbedWaitTimeoutCode);
      expect(result.errorMessage, contains('#fail-ready'));
      expect(recorder.events, [
        'wait:#continue-ready',
        'command:write',
        'wait:#fail-ready',
      ]);
    });

    test('wait_js_예외는_execution_failed로_전파하고_후속작업을_중단한다', () async {
      executionScriptsBuilder = eventExecutionScripts;
      bindingWaitScriptBuilder = eventBindingWaitScript;
      final recorder = EventJsRecorder(
        waitResponses: [Exception('wait exploded')],
        resultResponses: ['{"out":"unreachable"}'],
      );
      final runner = ControlledEmbedRunner(settleDelay: emptySettle);

      final result = await runner.execute(
        runCommandJs: recorder.runCommandJs,
        runResultJs: recorder.runResultJs,
        runWaitJs: recorder.runWaitJs,
        bindings: [
          SelectorBindingDto(
            role: BindingRoleDto.input,
            field: 'q',
            selector: '#q',
            triggerAction: ControlledEmbedTriggerActionDto.click,
            wait: BindingWaitDto(
              forSelector: '#ready',
              condition: BindingWaitConditionDto.exists,
              timeoutMs: big100,
              settleMs: big0,
              onTimeout: BindingWaitOnTimeoutDto.fail,
            ),
          ),
          SelectorBindingDto(
            role: BindingRoleDto.output,
            field: 'out',
            selector: '#out',
            triggerAction: ControlledEmbedTriggerActionDto.click,
          ),
        ],
        inputs: {'q': 'hello'},
      );

      expect(result.ok, isFalse);
      expect(result.error!.code, kControlledEmbedExecutionErrorCode);
      expect(result.errorMessage, contains('wait exploded'));
      expect(recorder.events, ['wait:#ready']);
    });

    test('trigger_바인딩의_대기가_트리거_직전에_실행된다', () async {
      final recorder = WaitJsRecorder(
        commandResponses: <Object?>[null, null], // input write + trigger
        waitResponses: [true], // triggers wait succeeds immediately
        resultResponses: <Object?>['{"out":"done"}'],
      );
      final runner = ControlledEmbedRunner(settleDelay: emptySettle);
      final result = await runner.execute(
        runCommandJs: recorder.runCommandJs,
        runResultJs: recorder.runResultJs,
        runWaitJs: recorder.runWaitJs,
        bindings: [
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
            wait: BindingWaitDto(
              forSelector: '#go-ready',
              condition: BindingWaitConditionDto.exists,
              timeoutMs: big100,
              settleMs: big0,
              onTimeout: BindingWaitOnTimeoutDto.fail,
            ),
          ),
          SelectorBindingDto(
            role: BindingRoleDto.output,
            field: 'out',
            selector: '#out',
            triggerAction: ControlledEmbedTriggerActionDto.click,
          ),
        ],
        inputs: {'q': 'test'},
      );

      expect(result.ok, isTrue);
      expect(recorder.waitCalls, hasLength(1));
      expect(recorder.commandCalls, hasLength(2)); // input write + trigger
    });

    test('output_바인딩의_대기가_리드_직전에_실행된다', () async {
      final recorder = WaitJsRecorder(
        commandResponses: <Object?>[null],
        waitResponses: [true], // output wait succeeds
        resultResponses: <Object?>['{"out":"read"}'],
      );
      final runner = ControlledEmbedRunner(settleDelay: emptySettle);
      final result = await runner.execute(
        runCommandJs: recorder.runCommandJs,
        runResultJs: recorder.runResultJs,
        runWaitJs: recorder.runWaitJs,
        bindings: [
          SelectorBindingDto(
            role: BindingRoleDto.trigger,
            field: '',
            selector: '#go',
            triggerAction: ControlledEmbedTriggerActionDto.click,
          ),
          SelectorBindingDto(
            role: BindingRoleDto.output,
            field: 'out',
            selector: '#out',
            triggerAction: ControlledEmbedTriggerActionDto.click,
            wait: BindingWaitDto(
              forSelector: '#out-ready',
              condition: BindingWaitConditionDto.visible,
              timeoutMs: big100,
              settleMs: big0,
              onTimeout: BindingWaitOnTimeoutDto.fail,
            ),
          ),
        ],
        inputs: const {},
      );

      expect(result.ok, isTrue);
      expect(recorder.waitCalls, hasLength(1));
    });
  });

  group('settle_대기 (Settle Delay)', () {
    test('대기_성공_후_settleMs만큼_delay를_적용한다', () async {
      final recorder = WaitJsRecorder(
        commandResponses: <Object?>[null],
        waitResponses: [true], // Ready immediately
        resultResponses: <Object?>['{"val":"settled"}'],
      );
      final stopwatch = Stopwatch();
      final runner = ControlledEmbedRunner(settleDelay: emptySettle);

      stopwatch.start();
      final result = await runner.execute(
        runCommandJs: recorder.runCommandJs,
        runResultJs: recorder.runResultJs,
        runWaitJs: recorder.runWaitJs,
        bindings: [
          inputBindingWithWait(timeoutMs: big5000, settleMs: BigInt.from(150)),
          SelectorBindingDto(
            role: BindingRoleDto.output,
            field: 'val',
            selector: '#val',
            triggerAction: ControlledEmbedTriggerActionDto.click,
          ),
        ],
        inputs: {
          'q': 'test',
        }, // Required: per-binding flow skips bindings without input values
      );
      stopwatch.stop();

      expect(result.ok, isTrue);
      // Should have spent at least ~150ms in settle delay.
      expect(
        stopwatch.elapsedMilliseconds,
        greaterThanOrEqualTo(140),
        reason: 'settleMs=150 should add at least 150ms delay',
      );
    });
  });
}
