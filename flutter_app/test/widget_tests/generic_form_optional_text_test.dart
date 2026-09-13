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
    overrides: [fakeKeyboardResolverOverride],
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
  testWidgets('선택 Text는 입력을 지우면 유효하고 snapshot에서 생략된다', (tester) async {
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
  });

  testWidgets('필수 Text는 입력을 지우면 계속 유효하지 않다', (tester) async {
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

  testWidgets('선택 Markdown 필드는 값을 지우면 snapshot에서 생략된다', (tester) async {
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
  });

  testWidgets('선택 파일 경로 필드는 값을 지우면 snapshot에서 생략된다', (tester) async {
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
  });

  testWidgets('선택 URL 필드는 값을 지우면 패턴 검증 없이 snapshot에서 생략된다', (tester) async {
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
