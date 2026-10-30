import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/tools.dart' show ToolDto;
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/rust/api/tools/file_input_policy.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/expanded_modal/generic_form.dart';

import '../test_helpers/fake_keyboard_resolver.dart';
import '../test_helpers/field_placeholder.dart';
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
    'a_untouched_compact_form_hides_required_errors_while_its_gate_is_closed',
    (tester) async {
      bool? isValid;
      final tool = fixtureToolDto(
        id: 'fixture.compact-required',
        inputFields: const [
          InputFieldDto(
            key: 'value',
            label: 'Value',
            fieldType: InputFieldType_Text(),
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
                controller: GenericFormController(),
                compact: true,
                onValidationChanged: (value) => isValid = value,
              ),
            ),
          ),
        ),
      );
      await tester.pump();

      expect(isValid, isFalse);
      expect(find.text('required'), findsNothing);
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

  testWidgets(
    'the_compact_placeholder_prefers_the_description_and_keeps_the_required_marker',
    (tester) async {
      await _pumpCompactForm(tester, _placeholderTool());

      // A macro-declared input's label is just its key ("input"); the
      // description says what to enter. The required marker survives.
      expect(drawnPlaceholder(tester, 'described'), 'UTF-8 string to encode *');
      expect(drawnPlaceholder(tester, 'optional'), 'Shown when empty');
      expect(drawnPlaceholder(tester, 'plain'), 'Plain *');
    },
  );

  testWidgets('the_compact_placeholder_is_announced_as_one_label', (
    tester,
  ) async {
    final semantics = tester.ensureSemantics();
    try {
      await _pumpCompactForm(tester, _placeholderTool());

      // The marker is drawn as its own piece but read together with the text.
      String announced(String key) =>
          tester.getSemantics(find.byKey(Key('field-$key'))).label;
      expect(announced('described'), 'UTF-8 string to encode *');
      expect(announced('optional'), 'Shown when empty');
      expect(announced('plain'), 'Plain *');
    } finally {
      semantics.dispose();
    }
  });

  testWidgets('a_semantics_hint_is_announced_by_each_field_and_never_drawn', (
    tester,
  ) async {
    final semantics = tester.ensureSemantics();
    try {
      await _pumpCompactForm(
        tester,
        _placeholderTool(),
        semanticsHint: 'announced only',
      );

      expect(find.text('announced only'), findsNothing);
      for (final key in <String>['described', 'optional', 'plain']) {
        expect(
          tester.getSemantics(find.byKey(Key('field-$key'))).hint,
          'announced only',
          reason: key,
        );
      }

      await _pumpCompactForm(tester, _placeholderTool());

      expect(
        tester.getSemantics(find.byKey(const Key('field-plain'))).hint,
        isEmpty,
      );
    } finally {
      semantics.dispose();
    }
  });

  testWidgets('a_semantics_hint_reaches_the_own_node_of_every_field_type', (
    tester,
  ) async {
    // Tall enough that no field is clipped out of the semantics tree.
    await tester.binding.setSurfaceSize(
      const Size(UpegSizing.pinCellWidth, 1600),
    );
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final semantics = tester.ensureSemantics();
    try {
      final fields = _oneFieldOfEveryType();
      await _pumpCompactForm(
        tester,
        fixtureToolDto(
          id: 'fixture.compact-hint-every-type',
          inputFields: fields,
        ),
        semanticsHint: 'announced only',
      );

      expect(tester.takeException(), isNull);
      for (final field in fields) {
        final finder = find.byKey(Key('field-${field.key}'));
        final node = tester.getSemantics(finder);

        expect(node.hint, 'announced only', reason: field.key);
        // `getSemantics` climbs to the nearest ancestor that has a node, so a
        // field without a node of its own would pass the line above by
        // reading its ancestor's hint. That node outgrows the field.
        expect(
          node.rect.height,
          lessThanOrEqualTo(tester.getSize(finder).height),
          reason: '${field.key} has no semantics node of its own',
        );
      }
    } finally {
      semantics.dispose();
    }
  });

  testWidgets(
    'a_long_description_placeholder_ellipsizes_instead_of_growing_the_field',
    (tester) async {
      Future<double> fieldHeight(String? description) async {
        await _pumpCompactForm(
          tester,
          fixtureToolDto(
            id: 'fixture.compact-long-placeholder',
            inputFields: [
              InputFieldDto(
                key: 'text',
                label: 'input',
                description: description,
                fieldType: const InputFieldType_Text(),
                required_: true,
              ),
            ],
          ),
        );
        return tester.getSize(find.byKey(const Key('field-text'))).height;
      }

      final labelOnly = await fieldHeight(null);
      final longDescription = await fieldHeight('Explains at length. ' * 12);

      // A wrapped placeholder would push the Run button out of the pin.
      expect(longDescription, labelOnly);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'a_compact_select_without_a_value_draws_its_placeholder_inside_the_field',
    (tester) async {
      final semantics = tester.ensureSemantics();
      try {
        await _pumpCompactForm(
          tester,
          fixtureToolDto(
            id: 'fixture.compact-select-placeholder',
            inputFields: const [
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
            ],
          ),
        );

        final field = find.byKey(const Key('field-choice'));
        final box = tester.getRect(
          find.descendant(of: field, matching: find.byType(InputDecorator)),
        );
        final placeholder = tester.getRect(
          find.descendant(of: field, matching: find.text('Choice')),
        );

        // A dropdown has no baseline for the decorator's own hint slot to
        // align with: a placeholder drawn there drops onto the underline.
        expect(placeholder.top, greaterThanOrEqualTo(box.top));
        expect(placeholder.bottom, lessThanOrEqualTo(box.bottom));
        expect(drawnPlaceholder(tester, 'choice'), 'Choice *');
        expect(tester.getSemantics(field).label, 'Choice *');
      } finally {
        semantics.dispose();
      }
    },
  );
}

List<InputFieldDto> _oneFieldOfEveryType() => [
  for (final (key, type) in <(String, InputFieldType)>[
    ('text', const InputFieldType_Text()),
    ('multiline', const InputFieldType_Multiline()),
    ('number', const InputFieldType_Number()),
    ('integer', const InputFieldType_Integer()),
    ('boolean', const InputFieldType_Boolean()),
    (
      'file',
      const InputFieldType_File(
        policy: FileInputPolicyDto(
          extensions: <String>[],
          maxCount: 1,
          maxFileBytes: null,
          maxTotalBytes: null,
        ),
      ),
    ),
    (
      'select',
      const InputFieldType_Select(
        options: <ChoiceOptionDto>[
          ChoiceOptionDto(value: 'alpha', label: 'alpha'),
          ChoiceOptionDto(value: 'beta', label: 'beta'),
        ],
      ),
    ),
    (
      'multi',
      const InputFieldType_MultiOptions(
        options: <ChoiceOptionDto>[
          ChoiceOptionDto(value: 'one', label: 'one'),
          ChoiceOptionDto(value: 'two', label: 'two'),
        ],
      ),
    ),
    ('datetime', const InputFieldType_DateTime()),
    ('markdown', const InputFieldType_Markdown()),
    ('filepath', const InputFieldType_FilePath()),
    ('url', const InputFieldType_Url()),
  ])
    InputFieldDto(key: key, label: key, fieldType: type, required_: false),
];

ToolDto _placeholderTool() => fixtureToolDto(
  id: 'fixture.compact-placeholder',
  inputFields: const [
    InputFieldDto(
      key: 'described',
      label: 'input',
      description: 'UTF-8 string to encode',
      fieldType: InputFieldType_Text(),
      required_: true,
    ),
    InputFieldDto(
      key: 'optional',
      label: 'Note',
      description: 'Shown when empty',
      fieldType: InputFieldType_Text(),
      required_: false,
    ),
    InputFieldDto(
      key: 'plain',
      label: 'Plain',
      fieldType: InputFieldType_Text(),
      required_: true,
    ),
  ],
);

Future<void> _pumpCompactForm(
  WidgetTester tester,
  ToolDto tool, {
  String? semanticsHint,
}) async {
  await tester.pumpWidget(
    ProviderScope(
      overrides: [fakeKeyboardResolverOverride, ...i18nTestOverrides],
      child: MaterialApp(
        theme: UpegTheme.darkTheme(),
        home: Scaffold(
          body: GenericFormWidget(
            tool: tool,
            controller: GenericFormController(),
            compact: true,
            semanticsHint: semanticsHint,
          ),
        ),
      ),
    ),
  );
  await tester.pump();
}
