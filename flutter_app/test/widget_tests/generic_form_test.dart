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
    testWidgets('GenericForm은_각_field_타입마다_적절한_위젯을_렌더한다', (tester) async {
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

      // 각 키마다 위젯이 하나씩 그려져 있다.
      expect(find.byKey(const Key('field-text_in')), findsOneWidget);
      expect(find.byKey(const Key('field-multi_in')), findsOneWidget);
      expect(find.byKey(const Key('field-num_in')), findsOneWidget);
      expect(find.byKey(const Key('field-bool_in')), findsOneWidget);
      expect(find.byKey(const Key('field-file_in')), findsOneWidget);
      expect(find.byKey(const Key('field-sel_in')), findsOneWidget);

      // 위젯 종류별 매핑 확인.
      // SwitchListTile 내부에 Switch 가 한 개 존재.
      expect(find.byType(Switch), findsOneWidget);
      // Select 필드는 DropdownButtonFormField 그 자체가 키를 가진다.
      expect(
        find.byWidgetPredicate(
          (w) =>
              w is DropdownButtonFormField &&
              w.key == const Key('field-sel_in'),
        ),
        findsOneWidget,
      );
      // required 필드는 ` *` 접미사가 붙는다.
      expect(find.text('Text *'), findsOneWidget);
    });

    testWidgets('GenericForm은_텍스트_입력을_controller에_반영한다', (tester) async {
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

    testWidgets('GenericForm은_빈_inputFields에서_안내문을_표시한다', (tester) async {
      final tool = _toolWithFields(const []);
      final controller = GenericFormController();
      await tester.pumpWidget(_harness(tool: tool, c: controller));

      expect(find.byKey(const Key('generic-form-empty')), findsOneWidget);
    });

    testWidgets('GenericForm은_화살표키로_field_focus를_이동한다', (tester) async {
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

    testWidgets('GenericForm은_Tab과_ShiftTab으로_field_focus를_이동한다', (
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

    testWidgets('필수 선택 필드는 값을 고르기 전까지 form을 유효하게 만들지 않는다', (tester) async {
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
          overrides: [fakeKeyboardResolverOverride],
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
    });

    testWidgets('Boolean 필드는 기본값 false를 typed snapshot에 포함한다', (tester) async {
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
    });

    testWidgets('Number 필드는 overflow 입력을 typed 숫자로 저장하거나 유효하게 보지 않는다', (
      tester,
    ) async {
      final tool = _toolWithFields(const [
        InputFieldDto(
          key: 'number',
          label: 'Number',
          fieldType: InputFieldType_Number(),
          required_: true,
        ),
      ]);
      final controller = GenericFormController();
      bool? isValid;
      await tester.pumpWidget(
        ProviderScope(
          overrides: [fakeKeyboardResolverOverride],
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
    });
  });
}
