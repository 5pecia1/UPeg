/// RED test for [`ExpandedModalPage`] consuming an `initialInput` map.
///
/// `LaunchIntentApplier` parses the deep-link `?input=…` JSON object
/// and threads it into the modal route's arguments. The page seeds
/// each [InputFieldDto] from the map before the first frame so the
/// user sees their pre-fill instead of an empty form.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/pages/expanded_modal_page.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';
import 'package:upeg/src/state/dispatch_stream_provider.dart';

import '../test_helpers/tool_fixture.dart';

import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/dispatch_stream_fixture.dart';

final _toolWithValue = fixtureToolDto(
  id: 'fixture.echo',
  toolkit: 'fixture',
  label: 'echo',
  inputFields: const [
    InputFieldDto(
      key: 'value',
      label: 'value',
      fieldType: InputFieldType_Text(),
      required_: true,
    ),
  ],
);

Widget _harness({ToolArgs? initialInput, ToolDto? tool}) {
  final t = tool ?? _toolWithValue;
  return ProviderScope(
    overrides: [
      ...i18nTestOverrides,
      dispatchStreamFnProvider.overrideWithValue(
        stubDispatchStream(
          ({required toolId, required args, required approve}) async =>
              const CanonicalToolResult(ok: true, outputs: []),
        ),
      ),
    ],
    child: MaterialApp(
      theme: UpegTheme.darkTheme(),
      home: ExpandedModalPage(tool: t, initialInput: initialInput),
    ),
  );
}

void main() {
  group('ExpandedModalPage initialInput prefill', () {
    testWidgets(
      'ExpandedModalPage_prefills_the_text_field_when_initialInput_is_present',
      (tester) async {
        await tester.pumpWidget(
          _harness(
            initialInput: ToolArgs.fromJsonObject(const <String, Object?>{
              'value': 'ff',
            }),
          ),
        );
        await tester.pump();
        expect(find.text('ff'), findsOneWidget);
      },
    );

    testWidgets(
      'ExpandedModalPage_leaves_the_field_empty_without_initialInput',
      (tester) async {
        await tester.pumpWidget(_harness());
        await tester.pump();
        // Empty form: no input text is visible.
        expect(find.text('ff'), findsNothing);
      },
    );

    testWidgets(
      'URL_and_Markdown_initialInput_show_as_the_first_screen_field_values',
      (tester) async {
        final tool = fixtureToolDto(
          id: 'fixture.prefill',
          inputFields: const [
            InputFieldDto(
              key: 'url',
              label: 'URL',
              fieldType: InputFieldType_Url(),
              required_: false,
            ),
            InputFieldDto(
              key: 'markdown',
              label: 'Markdown',
              fieldType: InputFieldType_Markdown(),
              required_: false,
            ),
          ],
        );

        await tester.pumpWidget(
          _harness(
            tool: tool,
            initialInput: ToolArgs.fromJsonObject(const <String, Object?>{
              'url': 'https://example.com',
              'markdown': '# seeded',
            }),
          ),
        );
        await tester.pump();

        expect(find.text('https://example.com'), findsOneWidget);
        expect(find.text('# seeded'), findsOneWidget);
      },
    );

    testWidgets('Select_and_DateTime_initialInput_show_as_typed_field_values', (
      tester,
    ) async {
      final tool = fixtureToolDto(
        id: 'fixture.typed-prefill',
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
          InputFieldDto(
            key: 'when',
            label: 'When',
            fieldType: InputFieldType_DateTime(),
            required_: true,
          ),
        ],
      );

      await tester.pumpWidget(
        _harness(
          tool: tool,
          initialInput: ToolArgs.fromJsonObject(const <String, Object?>{
            'choice': 'alpha',
            'when': '2026-07-15T12:30:00Z',
          }),
        ),
      );
      await tester.pump();

      expect(find.text('alpha'), findsOneWidget);
      expect(find.text('2026-07-15T12:30:00.000Z'), findsOneWidget);
    });
  });
}
