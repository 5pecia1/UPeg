/// RED test for the MultiOptions FilterChip field kind.
///
/// `InputFieldType::MultiOptions { options }` lands in Dart as
/// `InputFieldType_MultiOptions(options: List<String>)`. The form
/// renders one `FilterChip` per option, taps toggle the selection set,
/// and the controller stores a [MultiOptionValue] so the dispatcher
/// receives a JSON array.
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/widgets/expanded_modal/form_value.dart';
import 'package:upeg/src/widgets/expanded_modal/generic_form.dart';

import '../test_helpers/tool_fixture.dart';

void main() {
  group('GenericForm MultiOptions field', () {
    testWidgets('GenericForm은_MultiOptions_필드를_FilterChip_set으로_렌더한다', (
      tester,
    ) async {
      final tool = fixtureToolDto(
        id: 'fixture.tags',
        inputFields: const [
          InputFieldDto(
            key: 'tags',
            label: 'Tags',
            fieldType: InputFieldType_MultiOptions(
              options: <ChoiceOptionDto>[
                ChoiceOptionDto(value: 'opt1', label: 'opt1'),
                ChoiceOptionDto(value: 'opt2', label: 'opt2'),
                ChoiceOptionDto(value: 'opt3', label: 'opt3'),
              ],
            ),
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
      // 옵션 수만큼 FilterChip 이 한 개씩 그려진다.
      expect(find.byType(FilterChip), findsNWidgets(3));
      expect(find.text('opt1'), findsOneWidget);
      expect(find.text('opt2'), findsOneWidget);
      expect(find.text('opt3'), findsOneWidget);
    });

    testWidgets('GenericForm_MultiOptions_탭은_선택목록을_갱신한다', (tester) async {
      final tool = fixtureToolDto(
        id: 'fixture.tags',
        inputFields: const [
          InputFieldDto(
            key: 'tags',
            label: 'Tags',
            fieldType: InputFieldType_MultiOptions(
              options: <ChoiceOptionDto>[
                ChoiceOptionDto(value: 'opt1', label: 'opt1'),
                ChoiceOptionDto(value: 'opt2', label: 'opt2'),
              ],
            ),
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

      await tester.tap(find.text('opt1'));
      await tester.pump();

      final stored = controller.value('tags');
      expect(stored, isA<MultiOptionValue>());
      expect((stored as MultiOptionValue).keys, contains('opt1'));

      // 두 번째 탭은 토글: 선택목록에서 빠진다.
      await tester.tap(find.text('opt1'));
      await tester.pump();
      final afterToggle = controller.value('tags') as MultiOptionValue;
      expect(afterToggle.keys, isNot(contains('opt1')));
    });
  });
}
