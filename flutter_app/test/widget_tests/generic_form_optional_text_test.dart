/// Regression coverage for the optional-text-field clear bug: clearing an
/// OPTIONAL text-shaped field (text/string, markdown, filePath, url) must
/// remove the key from the controller — not store an empty [TextValue] —
/// so the dispatched args omit it instead of sending `""`. Mirrors
/// `generic_form_optional_number_test.dart`'s coverage of `_numericField`.
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

ToolDto _textShapedTool({
  required bool isRequired,
  required InputFieldType fieldType,
}) {
  return fixtureToolDto(
    id: 'fixture.text',
    label: 'Text fixture',
    inputFields: [
      InputFieldDto(
        key: 'text',
        label: 'Text',
        fieldType: fieldType,
        required_: isRequired,
      ),
    ],
  );
}

Widget _harness({
  required bool isRequired,
  required InputFieldType fieldType,
  required GenericFormController controller,
  required ValueChanged<bool> onValidationChanged,
}) {
  return ProviderScope(
    overrides: [...i18nTestOverrides, fakeKeyboardResolverOverride],
    child: MaterialApp(
      home: Scaffold(
        body: GenericFormWidget(
          tool: _textShapedTool(isRequired: isRequired, fieldType: fieldType),
          controller: controller,
          onValidationChanged: onValidationChanged,
        ),
      ),
    ),
  );
}

void main() {
  testWidgets(
    'an_optional_Text_is_valid_when_cleared_and_omitted_from_the_snapshot',
    (tester) async {
      final controller = GenericFormController();
      bool? isValid;
      await tester.pumpWidget(
        _harness(
          isRequired: false,
          fieldType: const InputFieldType_Text(),
          controller: controller,
          onValidationChanged: (value) => isValid = value,
        ),
      );
      await tester.pump();

      await tester.enterText(find.byKey(const Key('field-text')), 'hello');
      await tester.pump();
      expect(controller.snapshot()['text'], 'hello');

      await tester.enterText(find.byKey(const Key('field-text')), '');
      await tester.pump();

      expect(isValid, isTrue);
      expect(controller.value('text'), isNull);
      expect(controller.snapshot().toJsonObject().containsKey('text'), isFalse);
    },
  );

  testWidgets('a_required_Text_stays_invalid_when_cleared', (tester) async {
    final controller = GenericFormController();
    bool? isValid;
    await tester.pumpWidget(
      _harness(
        isRequired: true,
        fieldType: const InputFieldType_Text(),
        controller: controller,
        onValidationChanged: (value) => isValid = value,
      ),
    );
    await tester.pump();

    await tester.enterText(find.byKey(const Key('field-text')), 'hello');
    await tester.pump();
    expect(isValid, isTrue);

    await tester.enterText(find.byKey(const Key('field-text')), '');
    await tester.pump();

    // Required semantics are unchanged: the raw (empty) text is still
    // stored so the required-empty validator can flag it, and the Run
    // gate stays closed.
    expect(isValid, isFalse);
    expect(controller.value('text'), const TextValue(''));
  });

  testWidgets(
    'an_optional_Markdown_field_is_omitted_from_the_snapshot_when_cleared',
    (tester) async {
      final controller = GenericFormController();
      await tester.pumpWidget(
        _harness(
          isRequired: false,
          fieldType: const InputFieldType_Markdown(),
          controller: controller,
          onValidationChanged: (_) {},
        ),
      );
      await tester.pump();

      await tester.enterText(find.byKey(const Key('field-text')), '# heading');
      await tester.pump();
      expect(controller.value('text'), const TextValue('# heading'));

      await tester.enterText(find.byKey(const Key('field-text')), '');
      await tester.pump();

      expect(controller.value('text'), isNull);
      expect(controller.snapshot().toJsonObject().containsKey('text'), isFalse);
    },
  );

  testWidgets(
    'an_optional_file_path_field_is_omitted_from_the_snapshot_when_cleared',
    (tester) async {
      final controller = GenericFormController();
      await tester.pumpWidget(
        _harness(
          isRequired: false,
          fieldType: const InputFieldType_FilePath(),
          controller: controller,
          onValidationChanged: (_) {},
        ),
      );
      await tester.pump();

      await tester.enterText(find.byKey(const Key('field-text')), '/tmp/a.txt');
      await tester.pump();
      expect(controller.value('text'), const TextValue('/tmp/a.txt'));

      await tester.enterText(find.byKey(const Key('field-text')), '');
      await tester.pump();

      expect(controller.value('text'), isNull);
      expect(controller.snapshot().toJsonObject().containsKey('text'), isFalse);
    },
  );

  testWidgets('an_optional_URL_field_is_omitted_from_the_snapshot_when_cleared_'
      'without_pattern_validation', (tester) async {
    final controller = GenericFormController();
    bool? isValid;
    await tester.pumpWidget(
      _harness(
        isRequired: false,
        fieldType: const InputFieldType_Url(),
        controller: controller,
        onValidationChanged: (value) => isValid = value,
      ),
    );
    await tester.pump();

    await tester.enterText(
      find.byKey(const Key('field-text')),
      'https://example.com',
    );
    await tester.pump();
    expect(controller.value('text'), const TextValue('https://example.com'));

    await tester.enterText(find.byKey(const Key('field-text')), '');
    await tester.pump();

    // Before the fix this stored `TextValue('')`: `_requiredCheck` short
    // circuits to OK for an optional empty value (never reaching the URL
    // pattern check), so the Run gate stayed open while the empty string
    // would still be dispatched and rejected by Rust.
    expect(isValid, isTrue);
    expect(controller.value('text'), isNull);
    expect(controller.snapshot().toJsonObject().containsKey('text'), isFalse);
  });
}
