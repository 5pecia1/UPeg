import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/embed.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/widgets/controlled_embed/runner.dart';
import '../test_helpers/controlled_embed_runner_harness.dart';

void main() {
  useStubbedExecutionScripts();

  group('Wait execution behavior', () {
    test('Read_runs_after_a_successful_wait_and_returns_outputs', () async {
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

    test(
      'a_fail_policy_wait_timeout_returns_a_canonical_wait_timeout_error',
      () async {
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
      },
    );

    test(
      'a_continue_policy_timeout_proceeds_without_further_waiting',
      () async {
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
      },
    );

    test('a_zero_timeout_performs_only_one_immediate_check', () async {
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

    test(
      'an_immediate_failure_with_zero_timeout_returns_a_wait_timeout_error',
      () async {
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
      },
    );

    test(
      'execution_without_wait_options_preserves_existing_behavior',
      () async {
        // Same as existing test: no wait → normal write/trigger/read.
        final recorder = RunJsRecorder(
          <Object?>[null, null],
          <Object?>['{"intro":"hi"}'],
        );
        final result =
            await const ControlledEmbedRunner(settleDelay: emptySettle).execute(
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
      },
    );
  });

  group('Per-binding wait execution', () {
    test(
      'multiple_bindings_with_the_same_role_each_run_their_own_wait',
      () async {
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
      },
    );

    test('a_continue_binding_timeout_does_not_affect_a_fail_binding', () async {
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

    test(
      'a_short_timeout_fails_after_the_initial_check_without_another_poll',
      () async {
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
      },
    );

    test(
      'per_binding_wait_write_trigger_output_read_order_is_preserved',
      () async {
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
      },
    );

    test(
      'a_fail_timeout_after_a_continue_timeout_stops_execution_without_reading',
      () async {
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
      },
    );

    test(
      'wait_js_exceptions_propagate_as_execution_failed_and_stop_subsequent_operations',
      () async {
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
      },
    );

    test(
      'a_trigger_binding_wait_runs_immediately_before_the_trigger',
      () async {
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
      },
    );

    test('an_output_binding_wait_runs_immediately_before_reading', () async {
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

  group('Settle delay', () {
    test('a_successful_wait_applies_the_settleMs_delay', () async {
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
