import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/rust/api/tools/file_input_policy.dart';
import 'package:upeg/src/widgets/expanded_modal/form_value.dart';
import 'package:upeg/src/widgets/expanded_modal/generic_form.dart';

import '../test_helpers/fake_keyboard_resolver.dart';
import '../test_helpers/tool_fixture.dart';

import '../test_helpers/i18n_test_catalog.dart';

ToolDto _toolWithFields(List<InputFieldDto> fields) {
  return fixtureToolDto(
    id: 'fixture.tool',
    label: 'Fixture',
    inputFields: fields,
  );
}

Widget _harness({required ToolDto tool, required GenericFormController c}) {
  return ProviderScope(
    overrides: [...i18nTestOverrides, fakeKeyboardResolverOverride],
    child: MaterialApp(
      home: Scaffold(
        body: SingleChildScrollView(
          child: GenericFormWidget(tool: tool, controller: c),
        ),
      ),
    ),
  );
}

bool _editableHasFocus(WidgetTester tester, String key) {
  final editable = find.descendant(
    of: find.byKey(Key('field-$key')),
    matching: find.byType(EditableText),
  );
  final widget = tester.widget<EditableText>(editable);
  return widget.focusNode.hasPrimaryFocus;
}

void main() {
  group('GenericFormWidget', () {
    testWidgets('the_generic_form_renders_the_right_widget_for_each_field_type', (
      tester,
    ) async {
      final tool = _toolWithFields(const [
        InputFieldDto(
          key: 'text_in',
          label: 'Text',
          fieldType: InputFieldType_Text(),
          required_: true,
        ),
        InputFieldDto(
          key: 'multi_in',
          label: 'Multi',
          fieldType: InputFieldType_Multiline(),
          required_: false,
        ),
        InputFieldDto(
          key: 'num_in',
          label: 'Number',
          fieldType: InputFieldType_Number(),
          required_: false,
        ),
        InputFieldDto(
          key: 'bool_in',
          label: 'Boolean',
          fieldType: InputFieldType_Boolean(),
          required_: false,
        ),
        InputFieldDto(
          key: 'file_in',
          label: 'File',
          fieldType: InputFieldType_File(
            policy: FileInputPolicyDto(
              extensions: <String>[],
              maxCount: 1,
              maxFileBytes: null,
              maxTotalBytes: null,
            ),
          ),
          required_: false,
        ),
        InputFieldDto(
          key: 'sel_in',
          label: 'Select',
          fieldType: InputFieldType_Select(
            options: <ChoiceOptionDto>[
              ChoiceOptionDto(value: 'a', label: 'a'),
              ChoiceOptionDto(value: 'b', label: 'b'),
            ],
          ),
          required_: false,
        ),
      ]);
      final controller = GenericFormController();
      await tester.pumpWidget(_harness(tool: tool, c: controller));

      // One widget is rendered per key.
      expect(find.byKey(const Key('field-text_in')), findsOneWidget);
      expect(find.byKey(const Key('field-multi_in')), findsOneWidget);
      expect(find.byKey(const Key('field-num_in')), findsOneWidget);
      expect(find.byKey(const Key('field-bool_in')), findsOneWidget);
      expect(find.byKey(const Key('field-file_in')), findsOneWidget);
      expect(find.byKey(const Key('field-sel_in')), findsOneWidget);

      // Verify the per-field-type widget mapping.
      // Exactly one Switch lives inside the SwitchListTile.
      expect(find.byType(Switch), findsOneWidget);
      // For the Select field the DropdownButtonFormField itself carries the key.
      expect(
        find.byWidgetPredicate(
          (w) =>
              w is DropdownButtonFormField &&
              w.key == const Key('field-sel_in'),
        ),
        findsOneWidget,
      );
      // Required fields get a ` *` suffix.
      expect(find.text('Text *'), findsOneWidget);
    });

    testWidgets('the_generic_form_mirrors_text_input_into_the_controller', (
      tester,
    ) async {
      final tool = _toolWithFields(const [
        InputFieldDto(
          key: 'message',
          label: 'Message',
          fieldType: InputFieldType_Text(),
          required_: false,
        ),
      ]);
      final controller = GenericFormController();
      await tester.pumpWidget(_harness(tool: tool, c: controller));

      await tester.enterText(find.byKey(const Key('field-message')), 'hello');
      await tester.pump();

      expect(controller.snapshot()['message'], equals('hello'));
    });

    testWidgets('the_generic_form_shows_a_notice_for_empty_inputfields', (
      tester,
    ) async {
      final tool = _toolWithFields(const []);
      final controller = GenericFormController();
      await tester.pumpWidget(_harness(tool: tool, c: controller));

      expect(find.byKey(const Key('generic-form-empty')), findsOneWidget);
    });

    testWidgets('the_generic_form_moves_field_focus_with_the_arrow_keys', (
      tester,
    ) async {
      final tool = _toolWithFields(const [
        InputFieldDto(
          key: 'first',
          label: 'First',
          fieldType: InputFieldType_Text(),
          required_: false,
        ),
        InputFieldDto(
          key: 'second',
          label: 'Second',
          fieldType: InputFieldType_Number(),
          required_: false,
        ),
        InputFieldDto(
          key: 'third',
          label: 'Third',
          fieldType: InputFieldType_Url(),
          required_: false,
        ),
      ]);
      final controller = GenericFormController();
      await tester.pumpWidget(_harness(tool: tool, c: controller));

      await tester.showKeyboard(find.byKey(const Key('field-first')));
      expect(_editableHasFocus(tester, 'first'), isTrue);

      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      await tester.pump();
      expect(_editableHasFocus(tester, 'second'), isTrue);

      await tester.sendKeyEvent(LogicalKeyboardKey.arrowUp);
      await tester.pump();
      expect(_editableHasFocus(tester, 'first'), isTrue);
    });

    testWidgets('the_generic_form_moves_field_focus_with_tab_and_shift_tab', (
      tester,
    ) async {
      final tool = _toolWithFields(const [
        InputFieldDto(
          key: 'first',
          label: 'First',
          fieldType: InputFieldType_Text(),
          required_: false,
        ),
        InputFieldDto(
          key: 'second',
          label: 'Second',
          fieldType: InputFieldType_Number(),
          required_: false,
        ),
      ]);
      final controller = GenericFormController();
      await tester.pumpWidget(_harness(tool: tool, c: controller));

      await tester.showKeyboard(find.byKey(const Key('field-first')));
      expect(_editableHasFocus(tester, 'first'), isTrue);

      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
      expect(_editableHasFocus(tester, 'second'), isTrue);

      await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
      await tester.pump();
      expect(_editableHasFocus(tester, 'first'), isTrue);
    });

    testWidgets(
      'a_required_select_keeps_the_form_invalid_until_a_value_is_chosen',
      (tester) async {
        final tool = _toolWithFields(const [
          InputFieldDto(
            key: 'choice',
            label: 'Choice',
            fieldType: InputFieldType_Select(
              options: <ChoiceOptionDto>[
                ChoiceOptionDto(value: 'alpha', label: 'alpha'),
                ChoiceOptionDto(value: 'beta', label: 'beta'),
              ],
            ),
            required_: true,
          ),
        ]);
        final controller = GenericFormController();
        bool? isValid;

        await tester.pumpWidget(
          ProviderScope(
            overrides: [...i18nTestOverrides, fakeKeyboardResolverOverride],
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

        await tester.tap(find.byKey(const Key('field-choice')));
        await tester.pumpAndSettle();
        await tester.tap(find.text('alpha').last);
        await tester.pumpAndSettle();

        expect(isValid, isTrue);
        expect(controller.value('choice'), const OptionValue('alpha'));
      },
    );

    testWidgets(
      'required_errors_wait_for_a_form_edit_while_the_validity_gate_stays_closed',
      (tester) async {
        final tool = _toolWithFields(const [
          InputFieldDto(
            key: 'first',
            label: 'First',
            fieldType: InputFieldType_Text(),
            required_: true,
          ),
          InputFieldDto(
            key: 'second',
            label: 'Second',
            fieldType: InputFieldType_Text(),
            required_: true,
          ),
        ]);
        final controller = GenericFormController();
        bool? isValid;

        await tester.pumpWidget(
          ProviderScope(
            overrides: [...i18nTestOverrides, fakeKeyboardResolverOverride],
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
        expect(find.text('required'), findsNothing);

        await tester.enterText(find.byKey(const Key('field-first')), 'value');
        await tester.pump();

        expect(isValid, isFalse);
        expect(find.text('required'), findsOneWidget);

        await tester.enterText(find.byKey(const Key('field-first')), '');
        await tester.pump();

        expect(find.text('required'), findsNWidgets(2));
      },
    );

    testWidgets('editing_one_form_exposes_required_errors_only_in_that_form', (
      tester,
    ) async {
      final firstController = GenericFormController();
      final secondController = GenericFormController();
      bool? firstValid;
      bool? secondValid;
      final firstTool = _toolWithFields(const [
        InputFieldDto(
          key: 'required',
          label: 'Required',
          fieldType: InputFieldType_Text(),
          required_: true,
        ),
        InputFieldDto(
          key: 'optional',
          label: 'Optional',
          fieldType: InputFieldType_Text(),
          required_: false,
          constraints: FieldConstraintsDto(
            string: StringConstraintsDto(default_: 'seeded'),
          ),
        ),
      ]);
      final secondTool = fixtureToolDto(
        id: 'fixture.second-tool',
        inputFields: const [
          InputFieldDto(
            key: 'required',
            label: 'Required',
            fieldType: InputFieldType_Text(),
            required_: true,
          ),
        ],
      );

      await tester.pumpWidget(
        ProviderScope(
          overrides: [...i18nTestOverrides, fakeKeyboardResolverOverride],
          child: MaterialApp(
            home: Scaffold(
              body: Column(
                children: [
                  GenericFormWidget(
                    tool: firstTool,
                    controller: firstController,
                    onValidationChanged: (value) => firstValid = value,
                  ),
                  GenericFormWidget(
                    tool: secondTool,
                    controller: secondController,
                    onValidationChanged: (value) => secondValid = value,
                  ),
                ],
              ),
            ),
          ),
        ),
      );
      await tester.pump();

      expect(firstController.value('optional'), const TextValue('seeded'));
      expect(firstValid, isFalse);
      expect(secondValid, isFalse);
      expect(find.text('required'), findsNothing);

      await tester.enterText(
        find.byKey(const Key('field-optional')),
        'changed',
      );
      await tester.pump();

      expect(find.text('required'), findsOneWidget);
    });

    testWidgets(
      'a_boolean_field_includes_its_default_false_in_the_typed_snapshot',
      (tester) async {
        final tool = _toolWithFields(const [
          InputFieldDto(
            key: 'enabled',
            label: 'Enabled',
            fieldType: InputFieldType_Boolean(),
            required_: true,
          ),
        ]);
        final controller = GenericFormController();

        await tester.pumpWidget(_harness(tool: tool, c: controller));
        await tester.pump();

        expect(controller.value('enabled'), const BooleanValue(false));
        expect(controller.snapshot()['enabled'], isFalse);
      },
    );

    testWidgets(
      'a_number_field_keeps_overflow_input_as_raw_text_and_stays_invalid',
      (tester) async {
        final tool = _toolWithFields(const [
          InputFieldDto(
            key: 'number',
            label: 'Number',
            fieldType: InputFieldType_Number(),
            required_: true,
          ),
          InputFieldDto(
            key: 'other',
            label: 'Other',
            fieldType: InputFieldType_Text(),
            required_: true,
          ),
        ]);
        final controller = GenericFormController();
        bool? isValid;
        await tester.pumpWidget(
          ProviderScope(
            overrides: [...i18nTestOverrides, fakeKeyboardResolverOverride],
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

        await tester.enterText(find.byKey(const Key('field-number')), '1e400');
        await tester.pump();

        expect(controller.value('number'), const TextValue('1e400'));
        expect(isValid, isFalse);
        expect(find.text('not a number'), findsOneWidget);
        expect(find.text('required'), findsOneWidget);
      },
    );
  });
}
