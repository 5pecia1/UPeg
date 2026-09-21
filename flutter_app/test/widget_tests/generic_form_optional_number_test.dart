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

ToolDto _numberTool({required bool isRequired}) {
  return fixtureToolDto(
    id: 'fixture.number',
    label: 'Number fixture',
    inputFields: [
      InputFieldDto(
        key: 'number',
        label: 'Number',
        fieldType: const InputFieldType_Number(),
        required_: isRequired,
      ),
    ],
  );
}

Widget _harness({
  required bool isRequired,
  required GenericFormController controller,
  required ValueChanged<bool> onValidationChanged,
}) {
  return ProviderScope(
    overrides: [...i18nTestOverrides, fakeKeyboardResolverOverride],
    child: MaterialApp(
      home: Scaffold(
        body: GenericFormWidget(
          tool: _numberTool(isRequired: isRequired),
          controller: controller,
          onValidationChanged: onValidationChanged,
        ),
      ),
    ),
  );
}

void main() {
  testWidgets(
    'an_optional_number_is_valid_when_cleared_and_omitted_from_the_snapshot',
    (tester) async {
      final controller = GenericFormController();
      bool? isValid;
      await tester.pumpWidget(
        _harness(
          isRequired: false,
          controller: controller,
          onValidationChanged: (value) => isValid = value,
        ),
      );
      await tester.pump();

      await tester.enterText(find.byKey(const Key('field-number')), '42');
      await tester.pump();
      expect(controller.snapshot()['number'], 42);

      await tester.enterText(find.byKey(const Key('field-number')), '');
      await tester.pump();

      expect(isValid, isTrue);
      expect(controller.value('number'), isNull);
      expect(
        controller.snapshot().toJsonObject().containsKey('number'),
        isFalse,
      );
    },
  );

  testWidgets(
    'an_optional_number_is_valid_with_whitespace_only_and_omitted_from_the_snapshot',
    (tester) async {
      final controller = GenericFormController();
      bool? isValid;
      await tester.pumpWidget(
        _harness(
          isRequired: false,
          controller: controller,
          onValidationChanged: (value) => isValid = value,
        ),
      );
      await tester.pump();

      await tester.enterText(find.byKey(const Key('field-number')), '42');
      await tester.pump();
      await tester.enterText(find.byKey(const Key('field-number')), '   ');
      await tester.pump();

      expect(isValid, isTrue);
      expect(controller.value('number'), isNull);
      expect(
        controller.snapshot().toJsonObject().containsKey('number'),
        isFalse,
      );
    },
  );

  testWidgets('a_required_number_stays_invalid_when_cleared', (tester) async {
    final controller = GenericFormController();
    bool? isValid;
    await tester.pumpWidget(
      _harness(
        isRequired: true,
        controller: controller,
        onValidationChanged: (value) => isValid = value,
      ),
    );
    await tester.pump();

    await tester.enterText(find.byKey(const Key('field-number')), '42');
    await tester.pump();
    expect(isValid, isTrue);

    await tester.enterText(find.byKey(const Key('field-number')), '');
    await tester.pump();

    expect(isValid, isFalse);
    expect(controller.value('number'), const TextValue(''));
  });
}
