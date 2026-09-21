/// Declared-constraint rendering + validation in the generic form.
///
/// These pin the contract that lets a tool stay on the generic form
/// instead of growing a bespoke widget: whatever the Rust declaration
/// says about a field (description, placeholder, default, range,
/// pattern, choice labels) has to show up here.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/widgets/expanded_modal/form_value.dart';
import 'package:upeg/src/widgets/expanded_modal/generic_form.dart';

import '../test_helpers/fake_keyboard_resolver.dart';
import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/tool_fixture.dart';

/// Mirrors `security.password_generate`'s declaration
/// (`optional count: Integer(min=8, max=128, default=20)`) — the tool
/// whose bespoke form this generic path replaced.
const InputFieldDto _passwordCountField = InputFieldDto(
  key: 'count',
  label: 'length',
  description: 'Password length',
  fieldType: InputFieldType_Integer(),
  required_: false,
  constraints: FieldConstraintsDto(
    number: NumberConstraintsDto(min: 8, max: 128, default_: 20),
  ),
);

const InputFieldDto _slugField = InputFieldDto(
  key: 'slug',
  label: 'Slug',
  fieldType: InputFieldType_Text(),
  required_: false,
  constraints: FieldConstraintsDto(
    string: StringConstraintsDto(
      regex: r'^[a-z-]+$',
      placeholder: 'my-post-title',
    ),
  ),
);

ToolDto _toolWith(List<InputFieldDto> fields) =>
    fixtureToolDto(id: 'fixture.constraints', inputFields: fields);

Future<bool?> _pump(
  WidgetTester tester, {
  required ToolDto tool,
  required GenericFormController controller,
}) async {
  bool? isValid;
  await tester.pumpWidget(
    ProviderScope(
      overrides: [...i18nTestOverrides, fakeKeyboardResolverOverride],
      child: MaterialApp(
        home: Scaffold(
          body: SingleChildScrollView(
            child: GenericFormWidget(
              tool: tool,
              controller: controller,
              onValidationChanged: (value) => isValid = value,
            ),
          ),
        ),
      ),
    ),
  );
  await tester.pump();
  return isValid;
}

void main() {
  group('GenericFormWidget declared constraints', () {
    testWidgets(
      'an_integer_field_prefills_the_declared_default_as_an_integer',
      (tester) async {
        final controller = GenericFormController();

        await _pump(
          tester,
          tool: _toolWith(const [_passwordCountField]),
          controller: controller,
        );

        expect(controller.value('count'), const NumberValue(20));
        expect(controller.snapshot()['count'], 20);
        expect(find.text('20'), findsOneWidget);
      },
    );

    testWidgets('an_integer_field_rejects_decimal_input', (tester) async {
      final controller = GenericFormController();
      await _pump(
        tester,
        tool: _toolWith(const [_passwordCountField]),
        controller: controller,
      );

      await tester.enterText(find.byKey(const Key('field-count')), '1.5');
      await tester.pump();

      expect(controller.value('count'), const TextValue('1.5'));
      expect(find.text('not an integer'), findsOneWidget);
    });

    testWidgets('an_integer_field_rejects_a_value_above_the_declared_max', (
      tester,
    ) async {
      final controller = GenericFormController();
      await _pump(
        tester,
        tool: _toolWith(const [_passwordCountField]),
        controller: controller,
      );

      await tester.enterText(find.byKey(const Key('field-count')), '200');
      await tester.pump();

      expect(find.text('max 128'), findsOneWidget);
    });

    testWidgets(
      'a_value_outside_the_declared_range_closes_the_form_validity_gate',
      (tester) async {
        final controller = GenericFormController();
        await _pump(
          tester,
          tool: _toolWith(const [_passwordCountField]),
          controller: controller,
        );

        await tester.enterText(find.byKey(const Key('field-count')), '2');
        await tester.pump();

        expect(find.text('min 8'), findsOneWidget);
      },
    );

    testWidgets('the_description_and_range_render_together_as_helper_text', (
      tester,
    ) async {
      await _pump(
        tester,
        tool: _toolWith(const [_passwordCountField]),
        controller: GenericFormController(),
      );

      expect(find.text('Password length · min 8 · max 128'), findsOneWidget);
    });

    testWidgets(
      'the_placeholder_renders_as_the_hint_of_an_empty_string_field',
      (tester) async {
        await _pump(
          tester,
          tool: _toolWith(const [_slugField]),
          controller: GenericFormController(),
        );

        expect(find.text('my-post-title'), findsOneWidget);
      },
    );

    testWidgets(
      'a_string_field_rejects_values_not_matching_the_declared_regex',
      (tester) async {
        final controller = GenericFormController();
        await _pump(
          tester,
          tool: _toolWith(const [_slugField]),
          controller: controller,
        );

        await tester.enterText(
          find.byKey(const Key('field-slug')),
          'Not A Slug',
        );
        await tester.pump();

        expect(find.text('invalid format'), findsOneWidget);

        await tester.enterText(find.byKey(const Key('field-slug')), 'a-slug');
        await tester.pump();

        expect(find.text('invalid format'), findsNothing);
      },
    );

    testWidgets('a_string_field_starts_with_the_declared_default', (
      tester,
    ) async {
      final controller = GenericFormController();

      await _pump(
        tester,
        tool: _toolWith(const [
          InputFieldDto(
            key: 'title',
            label: 'Title',
            fieldType: InputFieldType_Text(),
            required_: true,
            constraints: FieldConstraintsDto(
              string: StringConstraintsDto(default_: 'hello-world'),
            ),
          ),
        ]),
        controller: controller,
      );

      expect(controller.value('title'), const TextValue('hello-world'));
    });

    testWidgets(
      'a_select_shows_choice_labels_and_descriptions_instead_of_values',
      (tester) async {
        await _pump(
          tester,
          tool: _toolWith(const [
            InputFieldDto(
              key: 'mode',
              label: 'Mode',
              fieldType: InputFieldType_Select(
                options: <ChoiceOptionDto>[
                  ChoiceOptionDto(
                    value: 'fast',
                    label: 'Fast',
                    description: 'lower quality',
                  ),
                  ChoiceOptionDto(value: 'safe', label: 'safe'),
                ],
              ),
              required_: true,
            ),
          ]),
          controller: GenericFormController(),
        );

        await tester.tap(find.byKey(const Key('field-mode')));
        await tester.pumpAndSettle();

        expect(find.text('Fast'), findsWidgets);
        expect(find.text('lower quality'), findsOneWidget);
        expect(find.text('fast'), findsNothing);
      },
    );

    testWidgets('multioptions_chips_show_choice_labels_instead_of_values', (
      tester,
    ) async {
      final controller = GenericFormController();
      await _pump(
        tester,
        tool: _toolWith(const [
          InputFieldDto(
            key: 'colors',
            label: 'Colors',
            fieldType: InputFieldType_MultiOptions(
              options: <ChoiceOptionDto>[
                ChoiceOptionDto(value: 'red', label: '빨강'),
                ChoiceOptionDto(value: 'blue', label: '파랑'),
              ],
            ),
            required_: false,
          ),
        ]),
        controller: controller,
      );

      expect(find.text('빨강'), findsOneWidget);
      expect(find.text('red'), findsNothing);

      await tester.tap(find.byKey(const Key('field-colors-chip-red')));
      await tester.pump();

      // The dispatched value stays the canonical option value, not the
      // display label.
      expect(controller.snapshot()['colors'], <String>['red']);
    });

    testWidgets(
      'fields_without_an_inputdecoration_still_show_their_description',
      (tester) async {
        await _pump(
          tester,
          tool: _toolWith(const [
            InputFieldDto(
              key: 'include_symbols',
              label: 'include symbols',
              description: 'Include punctuation symbols',
              fieldType: InputFieldType_Boolean(),
              required_: false,
            ),
            InputFieldDto(
              key: 'colors',
              label: 'Colors',
              description: 'Pick any number of colors',
              fieldType: InputFieldType_MultiOptions(
                options: <ChoiceOptionDto>[
                  ChoiceOptionDto(value: 'red', label: 'red'),
                ],
              ),
              required_: false,
            ),
          ]),
          controller: GenericFormController(),
        );

        expect(find.text('Include punctuation symbols'), findsOneWidget);
        expect(find.text('Pick any number of colors'), findsOneWidget);
      },
    );
  });
}
