import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/embed.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/widgets/controlled_embed/runner.dart';
import '../test_helpers/controlled_embed_runner_harness.dart';

void main() {
  useStubbedExecutionScripts();

  group('ControlledEmbedRunner_실행', () {
    test('비_액션_플랜은_runJs를_호출하지_않고_빈_outputs를_반환한다', () async {
      final recorder = RunJsRecorder(const [], const []);
      final result = await const ControlledEmbedRunner(settleDelay: emptySettle)
          .execute(
            runCommandJs: recorder.runCommandJs,
            runResultJs: recorder.runResultJs,
            bindings: const [],
            inputs: const {},
          );
      expect(result.ok, isTrue);
      expect(result.outputs, isEmpty);
      expect(recorder.commandCalls, isEmpty);
      expect(recorder.resultCalls, isEmpty);
    });

    test('write_trigger_read_순서로_호출되고_성공_outputs를_파싱한다', () async {
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

    test('runJs 예외는 canonical structured error로 매핑된다', () async {
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

    test('null_result_platform_예외는_도메인_error로_정규화된다', () async {
      final recorder = RunJsRecorder(<Object?>[], <Object?>[
        ThrownObject(
          ArgumentError(
            'Result of JavaScript execution returned a null value. (https://docs.flutter.dev/platform-channels/advanced/channels.md#javaScript)',
          ),
        ),
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
      final error = result.errorMessage!;
      expect(error, equals(kControlledEmbedNullJavaScriptResultMessage));
      expect(error, isNot(contains('Invalid argument(s)')));
      expect(error, isNot(contains('platform-channels')));
    });

    test('일반_runJs_예외는_기존_메시지를_보존한다', () async {
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

    test('non_object_payload는_ControlledEmbedError를_반환한다', () async {
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

    // 분할 실행 모드 테스트

    test('write_trigger는_returning_result를_요구하지_않고_read만_결과를_읽는다', () async {
      final recorder = RunJsRecorder(
        <Object?>[null, null],
        <Object?>['{"result":"ok"}'],
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
    });

    test('observed_backtick_null_result_예외는_도메인_error로_정규화된다', () async {
      final recorder = RunJsRecorder(<Object?>[], <Object?>[
        ThrownObject(
          ArgumentError(
            '`Result of JavaScript execution returned a null value. (https://docs.flutter.dev)`',
          ),
        ),
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
      expect(
        result.errorMessage,
        equals(kControlledEmbedNullJavaScriptResultMessage),
      );
    });

    test('backtick_null_value_result_예외도_도메인_error로_정규화된다', () async {
      final recorder = RunJsRecorder(<Object?>[], <Object?>[
        ThrownObject(
          ArgumentError(
            '`Result of JavaScript execution returned a null value.`',
          ),
        ),
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
      expect(
        result.errorMessage,
        equals(kControlledEmbedNullJavaScriptResultMessage),
      );
    });

    test('non_backtick_null_result_platform_예외도_도메인_error로_정규화된다', () async {
      final recorder = RunJsRecorder(<Object?>[], <Object?>[
        ThrownObject(
          ArgumentError(
            'RESULT OF JAVASCRIPT EXECUTION RETURNED A NULL VALUE.',
          ),
        ),
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
      expect(
        result.errorMessage,
        equals(kControlledEmbedNullJavaScriptResultMessage),
      );
    });

    test('일반_command_예외는_기존_메시지를_보존한다', () async {
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

  group('대기옵션 (BindingWait)', () {
    test('SelectorBindingDto는_대기옵션을_수용한다', () {
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

    test('stub_wait_builder는_대기옵션이_있는_바인딩에서_wait_스크립트를_생성한다', () {
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
    });

    test('stub_wait_builder는_대기옵션이_없으면_빈_wait_스크립트를_반환한다', () {
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
    });
  });
}
