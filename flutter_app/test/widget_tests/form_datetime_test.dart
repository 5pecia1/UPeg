/// RED test for the DateTime field kind.
///
/// `InputFieldType.dateTime()` renders as a read-only TextField paired
/// with an `IconButton(Icons.calendar_today)` that pops Flutter's
/// built-in `showDatePicker`. Selecting a date writes a
/// [DateTimeValue] to the controller — the dispatcher then serialises
/// it as ISO-8601 via [FormValue.toJson].
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/widgets/expanded_modal/form_value.dart';
import 'package:upeg/src/widgets/expanded_modal/generic_form.dart';

import '../test_helpers/tool_fixture.dart';

void main() {
  group('GenericForm DateTime field', () {
    testWidgets('GenericForm_renders_a_DateTime_field_as_text_plus_picker', (
      tester,
    ) async {
      final tool = fixtureToolDto(
        id: 'fixture.dt',
        inputFields: const [
          InputFieldDto(
            key: 'when',
            label: 'When',
            fieldType: InputFieldType_DateTime(),
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
      expect(find.byKey(const Key('field-when')), findsOneWidget);
      expect(find.byIcon(Icons.calendar_today), findsOneWidget);
    });

    testWidgets(
      'GenericForm_DateTime_picker_writes_the_selection_to_the_form',
      (tester) async {
        final tool = fixtureToolDto(
          id: 'fixture.dt',
          inputFields: const [
            InputFieldDto(
              key: 'when',
              label: 'When',
              fieldType: InputFieldType_DateTime(),
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
        // Tap calendar icon to open picker.
        await tester.tap(find.byIcon(Icons.calendar_today));
        await tester.pumpAndSettle();
        // showDatePicker renders an OK button — accept the default
        // (today) selection.
        await tester.tap(find.text('OK'));
        await tester.pumpAndSettle();

        final stored = controller.value('when');
        expect(stored, isA<DateTimeValue>());
      },
    );
  });
}
