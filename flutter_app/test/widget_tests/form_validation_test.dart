/// RED test for pure field-validators + Run-button gating.
///
/// Pure validators sit next to [FormValue] so unit tests can exercise
/// them without booting a widget tree; the widget test verifies the
/// Run button is disabled when any validation fails.
library;

import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/widgets/expanded_modal/form_validation.dart';
import 'package:upeg/src/widgets/expanded_modal/form_value.dart';
import 'package:upeg/src/widgets/expanded_modal/generic_form.dart';

import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/tool_fixture.dart';

void main() {
  group('pure validators', () {
    test('validateNumber_returns_an_error_for_abc_input', () {
      final result = validateNumber('abc', required: false);
      expect(result, isA<FieldValidationError>());
    });

    test('validateNumber_returns_ok_for_a_parseable_negative_number', () {
      final result = validateNumber('-5', required: false);
      expect(result, isA<FieldValidationOk>());
    });

    test('validateNumber_returns_an_error_for_an_empty_required_value', () {
      final result = validateNumber('', required: true);
      expect(result, isA<FieldValidationError>());
    });

    test('validateNumber_returns_ok_for_an_empty_optional_value', () {
      final result = validateNumber('', required: false);
      expect(result, isA<FieldValidationOk>());
    });

    test('Number validation rejects non-finite numeric representations', () {
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

    test('validateUrl_returns_an_error_for_an_invalid_url', () {
      final result = validateUrl('not a url', required: false);
      expect(result, isA<FieldValidationError>());
    });

    test('validateUrl_returns_ok_for_an_absolute_url', () {
      final result = validateUrl('https://example.com', required: false);
      expect(result, isA<FieldValidationOk>());
    });

    test('validateText_returns_an_error_for_an_empty_required_value', () {
      final result = validateText('', required: true);
      expect(result, isA<FieldValidationError>());
    });

    test('validateText_returns_ok_for_a_nonempty_required_value', () {
      final result = validateText('hi', required: true);
      expect(result, isA<FieldValidationOk>());
    });

    test(
      'a required Select fails when OptionValue is missing or not allowed',
      () {
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
      },
    );

    test('required MultiOptions need at least one allowed selection', () {
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

    test(
      'required DateTime and Boolean fields pass only with valid typed values',
      () {
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
      },
    );

    test('a required File passes only after selecting a FileFormValue', () {
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
    testWidgets('GenericForm_disables_the_run_button_for_empty_required_text', (
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
        ProviderScope(
          overrides: [...i18nTestOverrides],
          child: MaterialApp(
            home: Scaffold(
              body: GenericFormWidget(
                tool: tool,
                controller: controller,
                onValidationChanged: (allOk) => runButtonEnabled = allOk,
              ),
            ),
          ),
        ),
      );
      await tester.pump();
      expect(runButtonEnabled, isFalse);
    });

    testWidgets(
      'GenericForm_shows_an_error_when_letters_are_entered_in_a_number_field',
      (tester) async {
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
          ProviderScope(
            overrides: [...i18nTestOverrides],
            child: MaterialApp(
              home: Scaffold(
                body: GenericFormWidget(tool: tool, controller: controller),
              ),
            ),
          ),
        );
        await tester.enterText(find.byKey(const Key('field-num')), 'abc');
        await tester.pump();
        expect(find.textContaining('number'), findsWidgets);
      },
    );

    testWidgets(
      'changing MultiOptions and DateTime selections recalculates required validity',
      (tester) async {
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
          ProviderScope(
            overrides: [...i18nTestOverrides],
            child: MaterialApp(
              home: Scaffold(
                body: GenericFormWidget(
                  tool: tool,
                  controller: controller,
                  onValidationChanged: (value) => isValid = value,
                ),
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
      },
    );

    testWidgets(
      'required MultiOptions errors are visible and exposed to semantics in full and compact forms',
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
              ProviderScope(
                overrides: [...i18nTestOverrides],
                child: MaterialApp(
                  home: Scaffold(
                    body: GenericFormWidget(
                      key: ValueKey(compact),
                      tool: tool,
                      controller: GenericFormController(),
                      compact: compact,
                    ),
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

  group('Declared constraint validation', () {
    test('validateInteger_rejects_fractional_literals', () {
      expect(
        validateInteger('1.5', required: true),
        const FieldValidationError('modal.validation.not_an_integer'),
      );
      expect(validateInteger('2', required: true), const FieldValidationOk());
    });

    test('validateNumber_accepts_fractional_values', () {
      expect(validateNumber('1.5', required: true), const FieldValidationOk());
    });

    test('out_of_range_numbers_return_a_message_with_the_boundary_value', () {
      expect(
        validateInteger('2', required: true, min: 8, max: 128),
        const FieldValidationError(
          'modal.validation.min',
          messageArgs: {'value': '8'},
        ),
      );
      expect(
        validateInteger('200', required: true, min: 8, max: 128),
        const FieldValidationError(
          'modal.validation.max',
          messageArgs: {'value': '128'},
        ),
      );
      expect(
        validateInteger('20', required: true, min: 8, max: 128),
        const FieldValidationOk(),
      );
    });

    test('boundary_messages_show_integers_without_a_decimal_point', () {
      expect(
        validateNumber('200', required: true, max: 128),
        const FieldValidationError(
          'modal.validation.max',
          messageArgs: {'value': '128'},
        ),
      );
      expect(
        validateNumber('2', required: true, min: 1.5),
        const FieldValidationOk(),
      );
      expect(
        validateNumber('1', required: true, min: 1.5),
        const FieldValidationError(
          'modal.validation.min',
          messageArgs: {'value': '1.5'},
        ),
      );
    });

    test('validateText_rejects_values_that_do_not_match_the_regex', () {
      expect(
        validateText('Not A Slug', required: true, pattern: r'^[a-z-]+$'),
        const FieldValidationError('modal.validation.invalid_format'),
      );
      expect(
        validateText('a-slug', required: true, pattern: r'^[a-z-]+$'),
        const FieldValidationOk(),
      );
    });

    test('empty_optional_input_bypasses_regex_validation', () {
      expect(
        validateText('', required: false, pattern: r'^[a-z-]+$'),
        const FieldValidationOk(),
      );
    });

    test('uncompilable_regex_is_left_to_Rust_and_passes_local_validation', () {
      expect(
        validateText('anything', required: true, pattern: '['),
        const FieldValidationOk(),
      );
    });
  });
}
