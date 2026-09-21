import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/embed.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/widgets/controlled_embed/runner.dart';
import '../test_helpers/controlled_embed_runner_harness.dart';

void main() {
  useStubbedExecutionScripts();

  group('ControlledEmbedRunner execution', () {
    test(
      'a_plan_without_actions_returns_empty_outputs_without_calling_runJs',
      () async {
        final recorder = RunJsRecorder(const [], const []);
        final result =
            await const ControlledEmbedRunner(settleDelay: emptySettle).execute(
              runCommandJs: recorder.runCommandJs,
              runResultJs: recorder.runResultJs,
              bindings: const [],
              inputs: const {},
            );
        expect(result.ok, isTrue);
        expect(result.outputs, isEmpty);
        expect(recorder.commandCalls, isEmpty);
        expect(recorder.resultCalls, isEmpty);
      },
    );

    test(
      'calls_write_trigger_read_in_order_and_parses_successful_outputs',
      () async {
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

    test('runJs exceptions map to canonical structured errors', () async {
      final recorder = RunJsRecorder(<Object?>[
        Exception('webview crashed'),
      ], <Object?>[]);
      final result = await const ControlledEmbedRunner(settleDelay: emptySettle)
          .execute(
            runCommandJs: recorder.runCommandJs,
            runResultJs: recorder.runResultJs,
            bindings: const [
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
            inputs: const {},
          );
      expect(result.ok, isFalse);
      expect(result.errorMessage, contains('webview crashed'));
    });

    test(
      'a_null_result_platform_exception_is_normalized_to_a_domain_error',
      () async {
        final recorder = RunJsRecorder(<Object?>[], <Object?>[
          ThrownObject(
            ArgumentError(
              'Result of JavaScript execution returned a null value. (https://docs.flutter.dev/platform-channels/advanced/channels.md#javaScript)',
            ),
          ),
        ]);
        final result =
            await const ControlledEmbedRunner(settleDelay: emptySettle).execute(
              runCommandJs: recorder.runCommandJs,
              runResultJs: recorder.runResultJs,
              bindings: const [
                SelectorBindingDto(
                  role: BindingRoleDto.output,
                  field: 'intro',
                  selector: '#intro',
                  triggerAction: ControlledEmbedTriggerActionDto.click,
                ),
              ],
              inputs: const {},
            );
        expect(result.ok, isFalse);
        final error = result.errorMessage!;
        expect(error, equals(kControlledEmbedNullJavaScriptResultMessage));
        expect(error, isNot(contains('Invalid argument(s)')));
        expect(error, isNot(contains('platform-channels')));
      },
    );

    test('ordinary_runJs_exceptions_preserve_the_original_message', () async {
      final recorder = RunJsRecorder(<Object?>[], <Object?>[
        Exception('webview crashed'),
      ]);
      final result = await const ControlledEmbedRunner(settleDelay: emptySettle)
          .execute(
            runCommandJs: recorder.runCommandJs,
            runResultJs: recorder.runResultJs,
            bindings: const [
              SelectorBindingDto(
                role: BindingRoleDto.output,
                field: 'intro',
                selector: '#intro',
                triggerAction: ControlledEmbedTriggerActionDto.click,
              ),
            ],
            inputs: const {},
          );
      expect(result.ok, isFalse);
      expect(result.errorMessage, contains('webview crashed'));
    });

    test('a_non_object_payload_returns_a_ControlledEmbedError', () async {
      final recorder = RunJsRecorder(<Object?>[], <Object?>['"not an object"']);
      final result = await const ControlledEmbedRunner(settleDelay: emptySettle)
          .execute(
            runCommandJs: recorder.runCommandJs,
            runResultJs: recorder.runResultJs,
            bindings: const [
              SelectorBindingDto(
                role: BindingRoleDto.output,
                field: 'intro',
                selector: '#intro',
                triggerAction: ControlledEmbedTriggerActionDto.click,
              ),
            ],
            inputs: const {},
          );
      expect(result.ok, isFalse);
    });

    // Split execution mode tests

    test(
      'write_and_trigger_require_no_returned_result_and_only_read_fetches_results',
      () async {
        final recorder = RunJsRecorder(
          <Object?>[null, null],
          <Object?>['{"result":"ok"}'],
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
                  field: 'result',
                  selector: '#result',
                  triggerAction: ControlledEmbedTriggerActionDto.click,
                ),
              ],
              inputs: const {'q': 'test'},
            );
        expect(recorder.commandCalls, ['/*WRITE*/', '/*TRIGGER*/']);
        expect(recorder.resultCalls, ['/*READ*/']);
        expect(result.ok, isTrue);
        expect(outputTextById(result), {'result': 'ok'});
      },
    );

    test(
      'the_observed_backtick_null_result_exception_is_normalized_to_a_domain_error',
      () async {
        final recorder = RunJsRecorder(<Object?>[], <Object?>[
          ThrownObject(
            ArgumentError(
              '`Result of JavaScript execution returned a null value. (https://docs.flutter.dev)`',
            ),
          ),
        ]);
        final result =
            await const ControlledEmbedRunner(settleDelay: emptySettle).execute(
              runCommandJs: recorder.runCommandJs,
              runResultJs: recorder.runResultJs,
              bindings: const [
                SelectorBindingDto(
                  role: BindingRoleDto.output,
                  field: 'intro',
                  selector: '#intro',
                  triggerAction: ControlledEmbedTriggerActionDto.click,
                ),
              ],
              inputs: const {},
            );
        expect(result.ok, isFalse);
        expect(
          result.errorMessage,
          equals(kControlledEmbedNullJavaScriptResultMessage),
        );
      },
    );

    test(
      'a_backtick_null_value_result_exception_also_normalizes_to_a_domain_error',
      () async {
        final recorder = RunJsRecorder(<Object?>[], <Object?>[
          ThrownObject(
            ArgumentError(
              '`Result of JavaScript execution returned a null value.`',
            ),
          ),
        ]);
        final result =
            await const ControlledEmbedRunner(settleDelay: emptySettle).execute(
              runCommandJs: recorder.runCommandJs,
              runResultJs: recorder.runResultJs,
              bindings: const [
                SelectorBindingDto(
                  role: BindingRoleDto.output,
                  field: 'intro',
                  selector: '#intro',
                  triggerAction: ControlledEmbedTriggerActionDto.click,
                ),
              ],
              inputs: const {},
            );
        expect(result.ok, isFalse);
        expect(
          result.errorMessage,
          equals(kControlledEmbedNullJavaScriptResultMessage),
        );
      },
    );

    test(
      'a_null_result_platform_exception_without_backticks_also_normalizes_to_a_domain_error',
      () async {
        final recorder = RunJsRecorder(<Object?>[], <Object?>[
          ThrownObject(
            ArgumentError(
              'RESULT OF JAVASCRIPT EXECUTION RETURNED A NULL VALUE.',
            ),
          ),
        ]);
        final result =
            await const ControlledEmbedRunner(settleDelay: emptySettle).execute(
              runCommandJs: recorder.runCommandJs,
              runResultJs: recorder.runResultJs,
              bindings: const [
                SelectorBindingDto(
                  role: BindingRoleDto.output,
                  field: 'intro',
                  selector: '#intro',
                  triggerAction: ControlledEmbedTriggerActionDto.click,
                ),
              ],
              inputs: const {},
            );
        expect(result.ok, isFalse);
        expect(
          result.errorMessage,
          equals(kControlledEmbedNullJavaScriptResultMessage),
        );
      },
    );

    test('ordinary_command_exceptions_preserve_the_original_message', () async {
      final recorder = RunJsRecorder(<Object?>[
        Exception('command failed'),
      ], <Object?>[]);
      final result = await const ControlledEmbedRunner(settleDelay: emptySettle)
          .execute(
            runCommandJs: recorder.runCommandJs,
            runResultJs: recorder.runResultJs,
            bindings: const [
              SelectorBindingDto(
                role: BindingRoleDto.trigger,
                field: '',
                selector: 'button',
                triggerAction: ControlledEmbedTriggerActionDto.click,
              ),
            ],
            inputs: const {},
          );
      expect(result.ok, isFalse);
      expect(result.errorMessage, contains('command failed'));
    });
  });

  group('Wait options (BindingWait)', () {
    test('SelectorBindingDto_accepts_wait_options', () {
      final binding = SelectorBindingDto(
        role: BindingRoleDto.input,
        field: 'q',
        selector: '#q',
        triggerAction: ControlledEmbedTriggerActionDto.click,
        wait: BindingWaitDto(
          forSelector: '#ready',
          condition: BindingWaitConditionDto.exists,
          timeoutMs: big5000,
          settleMs: big100,
          onTimeout: BindingWaitOnTimeoutDto.fail,
        ),
      );

      expect(binding.wait, isNotNull);
      expect(binding.wait!.forSelector, '#ready');
      expect(binding.wait!.condition, BindingWaitConditionDto.exists);
      expect(binding.wait!.timeoutMs, big5000);
      expect(binding.wait!.settleMs, big100);
      expect(binding.wait!.onTimeout, BindingWaitOnTimeoutDto.fail);
    });

    test(
      'the_stub_wait_builder_generates_a_wait_script_for_a_binding_with_wait_options',
      () {
        final script = stubBindingWaitScript(
          binding: SelectorBindingDto(
            role: BindingRoleDto.input,
            field: 'q',
            selector: '#q',
            triggerAction: ControlledEmbedTriggerActionDto.click,
            wait: BindingWaitDto(
              forSelector: '#ready',
              condition: BindingWaitConditionDto.exists,
              timeoutMs: big5000,
              settleMs: big0,
              onTimeout: BindingWaitOnTimeoutDto.fail,
            ),
          ),
        );

        expect(script, contains('/*WAIT*/'));
      },
    );

    test(
      'the_stub_wait_builder_returns_an_empty_wait_script_without_wait_options',
      () {
        final script = stubBindingWaitScript(
          binding: SelectorBindingDto(
            role: BindingRoleDto.input,
            field: 'q',
            selector: '#q',
            triggerAction: ControlledEmbedTriggerActionDto.click,
            wait: null,
          ),
        );

        expect(script, isEmpty);
      },
    );
  });
}
