import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/rust/api/tools/file_input_policy.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/expanded_modal/generic_form.dart';

import '../test_helpers/fake_keyboard_resolver.dart';
import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/color_contrast.dart';
import '../test_helpers/tool_fixture.dart';

const Size _u1Viewport = Size(
  UpegSizing.pinCellWidth,
  UpegSizing.pinCellHeight,
);

void main() {
  testWidgets(
    'the_compact_form_renders_every_non_text_field_at_u1_width_without_errors',
    (tester) async {
      await tester.binding.setSurfaceSize(_u1Viewport);
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final tool = fixtureToolDto(
        id: 'fixture.compact',
        inputFields: const [
          InputFieldDto(
            key: 'file',
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
            key: 'path',
            label: 'Path',
            fieldType: InputFieldType_FilePath(),
            required_: false,
          ),
          InputFieldDto(
            key: 'choice',
            label: 'Choice',
            fieldType: InputFieldType_Select(
              options: <ChoiceOptionDto>[
                ChoiceOptionDto(value: 'alpha', label: 'alpha'),
                ChoiceOptionDto(value: 'beta', label: 'beta'),
              ],
            ),
            required_: false,
          ),
          InputFieldDto(
            key: 'when',
            label: 'When',
            fieldType: InputFieldType_DateTime(),
            required_: false,
          ),
          InputFieldDto(
            key: 'enabled',
            label: 'Enabled',
            fieldType: InputFieldType_Boolean(),
            required_: false,
          ),
          InputFieldDto(
            key: 'tags',
            label: 'Tags',
            fieldType: InputFieldType_MultiOptions(
              options: <ChoiceOptionDto>[
                ChoiceOptionDto(value: 'one', label: 'one'),
                ChoiceOptionDto(value: 'two', label: 'two'),
              ],
            ),
            required_: false,
          ),
        ],
      );

      await tester.pumpWidget(
        ProviderScope(
          overrides: [fakeKeyboardResolverOverride, ...i18nTestOverrides],
          child: MaterialApp(
            theme: UpegTheme.darkTheme(),
            home: Scaffold(
              body: SingleChildScrollView(
                child: GenericFormWidget(
                  tool: tool,
                  controller: GenericFormController(),
                  compact: true,
                ),
              ),
            ),
          ),
        ),
      );
      await tester.pump();

      expect(tester.takeException(), isNull);
      expect(find.byType(SwitchListTile), findsNothing);
      expect(find.byType(Switch), findsOneWidget);
      expect(find.byType(FilterChip), findsNWidgets(2));
    },
  );

  testWidgets(
    'a_compact_boolean_exposes_its_label_state_and_action_as_one_semantics_control',
    (tester) async {
      final semantics = tester.ensureSemantics();
      final controller = GenericFormController();
      final tool = fixtureToolDto(
        id: 'fixture.compact-boolean',
        inputFields: const [
          InputFieldDto(
            key: 'enabled',
            label: 'Enabled',
            fieldType: InputFieldType_Boolean(),
            required_: true,
          ),
        ],
      );
      try {
        await tester.pumpWidget(
          ProviderScope(
            overrides: [fakeKeyboardResolverOverride, ...i18nTestOverrides],
            child: MaterialApp(
              theme: UpegTheme.darkTheme(),
              home: Scaffold(
                body: GenericFormWidget(
                  tool: tool,
                  controller: controller,
                  compact: true,
                ),
              ),
            ),
          ),
        );
        await tester.pump();

        final control = find.bySemanticsLabel('Enabled *');
        expect(control, findsOneWidget);
        expect(
          tester.getSemantics(control),
          matchesSemantics(
            label: 'Enabled *',
            hasToggledState: true,
            isToggled: false,
            hasEnabledState: true,
            isEnabled: true,
            isFocusable: true,
            hasTapAction: true,
          ),
        );
        expect(
          tester.getSemantics(control).rect.size,
          tester.getSize(find.byKey(const Key('field-enabled'))),
        );

        await tester.tap(find.byType(Switch));
        await tester.pump();

        expect(controller.snapshot()['enabled'], isTrue);
        expect(
          tester.getSemantics(control),
          matchesSemantics(
            label: 'Enabled *',
            hasToggledState: true,
            isToggled: true,
            hasEnabledState: true,
            isEnabled: true,
            isFocusable: true,
            hasTapAction: true,
          ),
        );
      } finally {
        semantics.dispose();
      }
    },
  );

  testWidgets(
    'tapping_a_compact_boolean_label_padding_and_switch_toggles_once_each',
    (tester) async {
      var changeCount = 0;
      final controller = GenericFormController(
        onChanged: () => changeCount += 1,
      );
      final tool = fixtureToolDto(
        id: 'fixture.compact-boolean-tap',
        inputFields: const [
          InputFieldDto(
            key: 'enabled',
            label: 'Enabled',
            fieldType: InputFieldType_Boolean(),
            required_: true,
          ),
        ],
      );
      await tester.pumpWidget(
        ProviderScope(
          overrides: [fakeKeyboardResolverOverride, ...i18nTestOverrides],
          child: MaterialApp(
            theme: UpegTheme.darkTheme(),
            home: Scaffold(
              body: GenericFormWidget(
                tool: tool,
                controller: controller,
                compact: true,
              ),
            ),
          ),
        ),
      );
      await tester.pump();

      await tester.tap(find.text('Enabled *'));
      await tester.pump();
      expect(controller.snapshot()['enabled'], isTrue);
      expect(changeCount, 1);

      await tester.tap(find.byType(Switch));
      await tester.pump();
      expect(controller.snapshot()['enabled'], isFalse);
      expect(changeCount, 2);

      final fieldRect = tester.getRect(find.byKey(const Key('field-enabled')));
      final switchRect = tester.getRect(find.byType(Switch));
      await tester.tapAt(
        Offset(switchRect.left - UpegSizing.radius2, fieldRect.center.dy),
      );
      await tester.pump();
      expect(controller.snapshot()['enabled'], isTrue);
      expect(changeCount, 3);
    },
  );

  testWidgets(
    'a_compact_multioptions_selected_label_contrasts_with_its_real_background',
    (tester) async {
      final tool = fixtureToolDto(
        id: 'fixture.compact-multi-options-contrast',
        inputFields: const [
          InputFieldDto(
            key: 'colors',
            label: '색상',
            fieldType: InputFieldType_MultiOptions(
              options: <ChoiceOptionDto>[
                ChoiceOptionDto(value: '빨강', label: '빨강'),
                ChoiceOptionDto(value: '파랑', label: '파랑'),
              ],
            ),
            required_: false,
          ),
        ],
      );
      await tester.pumpWidget(
        ProviderScope(
          overrides: [fakeKeyboardResolverOverride, ...i18nTestOverrides],
          child: MaterialApp(
            theme: UpegTheme.lightTheme(),
            home: Scaffold(
              body: GenericFormWidget(
                tool: tool,
                controller: GenericFormController(),
                compact: true,
              ),
            ),
          ),
        ),
      );
      await tester.tap(find.byKey(const Key('field-colors-chip-빨강')));
      await tester.pump();

      final chip = tester.widget<FilterChip>(
        find.byKey(const Key('field-colors-chip-빨강')),
      );
      final label = chip.label as Text;
      final foreground = label.style!.color!;
      final background = chip.selectedColor!;

      expect(chip.selected, isTrue);
      expect(
        colorContrastRatio(foreground, background),
        greaterThanOrEqualTo(minimumNormalTextContrastRatio),
      );
    },
  );
}
