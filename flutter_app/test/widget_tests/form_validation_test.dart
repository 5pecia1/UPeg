/// RED test for pure field-validators + Run-button gating.
///
/// Pure validators sit next to [FormValue] so unit tests can exercise
/// them without booting a widget tree; the widget test verifies the
/// Run button is disabled when any validation fails.
library;

import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/widgets/expanded_modal/form_validation.dart';
import 'package:upeg/src/widgets/expanded_modal/form_value.dart';
import 'package:upeg/src/widgets/expanded_modal/generic_form.dart';

import '../test_helpers/tool_fixture.dart';

void main() {
  group('pure validators', () {
    test('validateNumber는_abc_입력시_에러를_반환한다', () {
      final result = validateNumber('abc', required: false);
      expect(result, isA<FieldValidationError>());
    });

    test('validateNumber는_음수_파싱_가능한_값에_ok를_반환한다', () {
      final result = validateNumber('-5', required: false);
      expect(result, isA<FieldValidationOk>());
    });

    test('validateNumber는_required_빈값에_에러를_반환한다', () {
      final result = validateNumber('', required: true);
      expect(result, isA<FieldValidationError>());
    });

    test('validateNumber는_optional_빈값에_ok를_반환한다', () {
      final result = validateNumber('', required: false);
      expect(result, isA<FieldValidationOk>());
    });

    test('Number validation은 non-finite 숫자 표현을 거부한다', () {
      expect(
        validateNumber('1e400', required: false),
        isA<FieldValidationError>(),
      );
      expect(
        validateNumber('NaN', required: false),
        isA<FieldValidationError>(),
      );
      expect(
        validateNumber('Infinity', required: false),
        isA<FieldValidationError>(),
      );
    });

    test('validateUrl는_잘못된_url에_에러를_반환한다', () {
      final result = validateUrl('not a url', required: false);
      expect(result, isA<FieldValidationError>());
    });

    test('validateUrl는_절대_url에_ok를_반환한다', () {
      final result = validateUrl('https://example.com', required: false);
      expect(result, isA<FieldValidationOk>());
    });

    test('validateText는_required_빈값에_에러를_반환한다', () {
      final result = validateText('', required: true);
      expect(result, isA<FieldValidationError>());
    });

    test('validateText는_required_채워진_값에_ok를_반환한다', () {
      final result = validateText('hi', required: true);
      expect(result, isA<FieldValidationOk>());
    });

    test('필수 Select는 OptionValue가 없거나 허용 목록 밖이면 실패한다', () {
      expect(
        validateOptionValue(null, required: true, options: const ['alpha']),
        isA<FieldValidationError>(),
      );
      expect(
        validateOptionValue(
          const OptionValue('beta'),
          required: true,
          options: const ['alpha'],
        ),
        isA<FieldValidationError>(),
      );
      expect(
        validateOptionValue(
          const OptionValue('alpha'),
          required: true,
          options: const ['alpha'],
        ),
        isA<FieldValidationOk>(),
      );
    });

    test('필수 MultiOptions는 하나 이상의 허용된 선택값을 요구한다', () {
      expect(
        validateMultiOptionValue(
          const MultiOptionValue([]),
          required: true,
          options: const ['alpha'],
        ),
        isA<FieldValidationError>(),
      );
      expect(
        validateMultiOptionValue(
          const MultiOptionValue(['alpha']),
          required: true,
          options: const ['alpha'],
        ),
        isA<FieldValidationOk>(),
      );
    });

    test('필수 DateTime과 Boolean은 올바른 typed 값이 있어야 통과한다', () {
      expect(
        validateDateTimeValue(null, required: true),
        isA<FieldValidationError>(),
      );
      expect(
        validateDateTimeValue(
          DateTimeValue(DateTime.utc(2026, 7, 15)),
          required: true,
        ),
        isA<FieldValidationOk>(),
      );
      expect(
        validateBooleanValue(const BooleanValue(false), required: true),
        isA<FieldValidationOk>(),
      );
    });

    test('필수 File은 FileFormValue가 선택된 후에만 통과한다', () {
      expect(
        validateFileValue(null, required: true),
        isA<FieldValidationError>(),
      );
      expect(
        validateFileValue(const TextValue('/tmp/input.bin'), required: true),
        isA<FieldValidationError>(),
      );
      expect(
        validateFileValue(
          FileFormValue(
            CanonicalFileValue(
              name: 'input.bin',
              isDir: false,
              content: CanonicalFileContent.bytes(
                bytes: Uint8List.fromList(const <int>[1, 2, 3]),
              ),
            ),
          ),
          required: true,
        ),
        isA<FieldValidationOk>(),
      );
    });
  });

  group('GenericFormWidget validation gating', () {
    testWidgets('GenericForm은_빈_required_text에서_run_버튼을_비활성화한다', (
      tester,
    ) async {
      final tool = fixtureToolDto(
        id: 'fixture.tool',
        inputFields: const [
          InputFieldDto(
            key: 'msg',
            label: 'Message',
            fieldType: InputFieldType_Text(),
            required_: true,
          ),
        ],
      );
      final controller = GenericFormController();
      var runButtonEnabled = true;
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: GenericFormWidget(
              tool: tool,
              controller: controller,
              onValidationChanged: (allOk) => runButtonEnabled = allOk,
            ),
          ),
        ),
      );
      await tester.pump();
      expect(runButtonEnabled, isFalse);
    });

    testWidgets('GenericForm은_숫자_필드에_문자_입력시_에러_메시지를_표시한다', (tester) async {
      final tool = fixtureToolDto(
        id: 'fixture.tool',
        inputFields: const [
          InputFieldDto(
            key: 'num',
            label: 'Count',
            fieldType: InputFieldType_Number(),
            required_: false,
          ),
        ],
      );
      final controller = GenericFormController();
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: GenericFormWidget(tool: tool, controller: controller),
          ),
        ),
      );
      await tester.enterText(find.byKey(const Key('field-num')), 'abc');
      await tester.pump();
      expect(find.textContaining('number'), findsWidgets);
    });

    testWidgets('MultiOptions와 DateTime 선택 변경은 required 유효성을 다시 계산한다', (
      tester,
    ) async {
      final tool = fixtureToolDto(
        id: 'fixture.typed-validation',
        inputFields: const [
          InputFieldDto(
            key: 'tags',
            label: 'Tags',
            fieldType: InputFieldType_MultiOptions(
              options: <ChoiceOptionDto>[
                ChoiceOptionDto(value: 'alpha', label: 'alpha'),
              ],
            ),
            required_: true,
          ),
          InputFieldDto(
            key: 'when',
            label: 'When',
            fieldType: InputFieldType_DateTime(),
            required_: true,
          ),
          InputFieldDto(
            key: 'enabled',
            label: 'Enabled',
            fieldType: InputFieldType_Boolean(),
            required_: true,
          ),
        ],
      );
      final controller = GenericFormController();
      bool? isValid;
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: GenericFormWidget(
              tool: tool,
              controller: controller,
              onValidationChanged: (value) => isValid = value,
            ),
          ),
        ),
      );
      await tester.pump();

      expect(isValid, isFalse);
      expect(controller.snapshot()['enabled'], isFalse);

      await tester.tap(find.text('alpha'));
      await tester.pump();
      expect(isValid, isFalse);

      await tester.tap(find.byIcon(Icons.calendar_today));
      await tester.pumpAndSettle();
      await tester.tap(find.text('OK'));
      await tester.pumpAndSettle();

      expect(isValid, isTrue);
    });

    testWidgets(
      '필수 MultiOptions 오류는 full과 compact form에서 화면과 semantics에 노출된다',
      (tester) async {
        final semantics = tester.ensureSemantics();
        final tool = fixtureToolDto(
          id: 'fixture.multi-error',
          inputFields: const [
            InputFieldDto(
              key: 'tags',
              label: 'Tags',
              fieldType: InputFieldType_MultiOptions(
                options: <ChoiceOptionDto>[
                  ChoiceOptionDto(value: 'alpha', label: 'alpha'),
                ],
              ),
              required_: true,
            ),
          ],
        );
        try {
          for (final compact in const [false, true]) {
            await tester.pumpWidget(
              MaterialApp(
                home: Scaffold(
                  body: GenericFormWidget(
                    key: ValueKey(compact),
                    tool: tool,
                    controller: GenericFormController(),
                    compact: compact,
                  ),
                ),
              ),
            );
            await tester.pump();

            expect(find.text('required'), findsOneWidget);
            expect(find.bySemanticsLabel('required'), findsOneWidget);
          }
        } finally {
          semantics.dispose();
        }
      },
    );
  });

  group('선언된 제약 검증', () {
    test('validateInteger는_소수_리터럴을_거부한다', () {
      expect(
        validateInteger('1.5', required: true),
        const FieldValidationError('not an integer'),
      );
      expect(validateInteger('2', required: true), const FieldValidationOk());
    });

    test('validateNumber는_소수를_받아들인다', () {
      expect(validateNumber('1.5', required: true), const FieldValidationOk());
    });

    test('숫자_범위를_벗어나면_경계값을_메시지로_돌려준다', () {
      expect(
        validateInteger('2', required: true, min: 8, max: 128),
        const FieldValidationError('min 8'),
      );
      expect(
        validateInteger('200', required: true, min: 8, max: 128),
        const FieldValidationError('max 128'),
      );
      expect(
        validateInteger('20', required: true, min: 8, max: 128),
        const FieldValidationOk(),
      );
    });

    test('경계값_메시지는_정수를_소수점_없이_보여준다', () {
      expect(
        validateNumber('200', required: true, max: 128),
        const FieldValidationError('max 128'),
      );
      expect(
        validateNumber('2', required: true, min: 1.5),
        const FieldValidationOk(),
      );
      expect(
        validateNumber('1', required: true, min: 1.5),
        const FieldValidationError('min 1.5'),
      );
    });

    test('validateText는_regex에_맞지_않는_값을_거부한다', () {
      expect(
        validateText('Not A Slug', required: true, pattern: r'^[a-z-]+$'),
        const FieldValidationError('invalid format'),
      );
      expect(
        validateText('a-slug', required: true, pattern: r'^[a-z-]+$'),
        const FieldValidationOk(),
      );
    });

    test('비어있는_선택_입력은_regex_검증을_건너뛴다', () {
      expect(
        validateText('', required: false, pattern: r'^[a-z-]+$'),
        const FieldValidationOk(),
      );
    });

    test('컴파일할_수_없는_regex는_Rust에_맡기고_통과시킨다', () {
      expect(
        validateText('anything', required: true, pattern: '['),
        const FieldValidationOk(),
      );
    });
  });
}
