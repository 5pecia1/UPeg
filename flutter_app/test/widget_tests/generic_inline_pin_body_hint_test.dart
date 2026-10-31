/// The Inline pin's source hint after the visual-density cleanup: the default
/// source (runs on input) draws no line under the form and is announced by
/// each field instead; only the exceptions keep a visible line.
library;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart' as rust;
import 'package:upeg/src/rust/api/tools.dart' as rust;
import 'package:upeg/src/rust/api/tools/file_input_policy.dart' as rust;
import 'package:upeg/src/rust/api/tools/input_field.dart' as rust;
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/inline/generic_inline_pin_body.dart';

import '../test_helpers/board_canvas_harness.dart' show genericInlineHarness;
import '../test_helpers/field_placeholder.dart';
import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/pin_persistence_overrides.dart';
import '../test_helpers/tool_fixture.dart';

const _description = 'UTF-8 string to encode';

rust.ToolDto _tool({
  rust.SourceDto source = const rust.SourceDto.userInput(),
  bool requiresApproval = false,
  bool withInputs = true,
  String label = 'input',
  String? description = _description,
  rust.InputFieldType otherType = const rust.InputFieldType.text(),
}) => fixtureToolDto(
  id: 'text.hint',
  source: source,
  requiresApproval: requiresApproval,
  inputFields: withInputs
      ? <rust.InputFieldDto>[
          rust.InputFieldDto(
            key: 'text',
            label: label,
            description: description,
            fieldType: const rust.InputFieldType.text(),
            required_: true,
          ),
          rust.InputFieldDto(
            key: 'other',
            label: 'Other',
            fieldType: otherType,
            required_: false,
          ),
        ]
      : const <rust.InputFieldDto>[],
);

Future<void> _pumpBody(WidgetTester tester, rust.ToolDto tool) async {
  await tester.pumpWidget(
    ProviderScope(
      overrides: [...i18nTestOverrides, ...pinPersistenceTestOverrides],
      child: MaterialApp(
        theme: UpegTheme.darkTheme(),
        home: Scaffold(
          body: SizedBox(
            width: 168,
            height: 150,
            child: GenericInlinePinBody(
              tool: tool,
              pinKey: (BoardKey.parse('dev'), PinId.parse(tool.id)),
            ),
          ),
        ),
      ),
    ),
  );
  await tester.pump();
}

String _announced(WidgetTester tester, String fieldKey) =>
    tester.getSemantics(find.byKey(Key('field-$fieldKey'))).hint;

void main() {
  testWidgets(
    'the_default_source_draws_no_hint_and_announces_runs_on_input_on_each_field',
    (tester) async {
      final semantics = tester.ensureSemantics();
      try {
        await _pumpBody(tester, _tool());

        expect(find.text(inlineUserInputSourceHint), findsNothing);
        expect(_announced(tester, 'text'), inlineUserInputSourceHint);
        expect(_announced(tester, 'other'), inlineUserInputSourceHint);
      } finally {
        semantics.dispose();
      }
    },
  );

  testWidgets('the_placeholder_names_what_to_enter_instead_of_the_key', (
    tester,
  ) async {
    await _pumpBody(tester, _tool());

    expect(drawnPlaceholder(tester, 'text'), '$_description *');
  });

  testWidgets('a_long_placeholder_keeps_its_required_marker_inside_the_field', (
    tester,
  ) async {
    const longText = 'UTF-8 string to encode, one line or many lines';
    for (final tool in <rust.ToolDto>[
      _tool(description: longText), // the description wins over the label
      _tool(label: longText, description: null),
    ]) {
      await _pumpBody(tester, tool);

      final field = find.byKey(const Key('field-text'));
      final text = find.descendant(of: field, matching: find.text(longText));
      final marker = find.descendant(of: field, matching: find.text(' *'));

      // The text gives way (one line, ellipsis); the marker does not.
      expect(
        tester.renderObject<RenderParagraph>(text).didExceedMaxLines,
        isTrue,
      );
      expect(marker, findsOneWidget);
      expect(
        tester.getRect(marker).right,
        lessThanOrEqualTo(tester.getRect(field).right),
      );
      expect(
        tester.getRect(marker).left,
        greaterThanOrEqualTo(tester.getRect(text).right),
      );
    }
  });

  testWidgets('a_gated_tool_neither_draws_nor_announces_runs_on_input', (
    tester,
  ) async {
    final semantics = tester.ensureSemantics();
    try {
      await _pumpBody(tester, _tool(requiresApproval: true));

      expect(find.text(inlineUserInputSourceHint), findsNothing);
      expect(_announced(tester, 'text'), isEmpty);
    } finally {
      semantics.dispose();
    }
  });

  testWidgets('a_timer_source_keeps_its_visible_hint_and_announces_nothing', (
    tester,
  ) async {
    final semantics = tester.ensureSemantics();
    try {
      final interval = BigInt.from(250);
      await _pumpBody(
        tester,
        _tool(source: rust.SourceDto.timer(intervalMs: interval)),
      );

      expect(find.text(inlineTimerSourceHint(interval)), findsOneWidget);
      expect(_announced(tester, 'text'), isEmpty);
    } finally {
      semantics.dispose();
    }
  });

  testWidgets('a_static_source_keeps_its_visible_hint_and_announces_nothing', (
    tester,
  ) async {
    final semantics = tester.ensureSemantics();
    try {
      await _pumpBody(tester, _tool(source: const rust.SourceDto.static_()));

      expect(find.text(inlineStaticSourceHint), findsOneWidget);
      expect(_announced(tester, 'text'), isEmpty);
    } finally {
      semantics.dispose();
    }
  });

  testWidgets('a_tool_without_inputs_draws_no_source_hint_at_all', (
    tester,
  ) async {
    await _pumpBody(
      tester,
      _tool(source: const rust.SourceDto.static_(), withInputs: false),
    );

    expect(find.text(inlineUserInputSourceHint), findsNothing);
    expect(find.text(inlineStaticSourceHint), findsNothing);
  });

  // On the board the pin's own semantics merge what they can: fields without
  // a node of their own would be read as one, with the hint twice.
  for (final (name, otherType) in <(String, rust.InputFieldType)>[
    (
      'a_multi_options_field',
      const rust.InputFieldType.multiOptions(
        options: <rust.ChoiceOptionDto>[
          rust.ChoiceOptionDto(value: 'red', label: 'Red'),
          rust.ChoiceOptionDto(value: 'blue', label: 'Blue'),
        ],
      ),
    ),
    (
      'a_file_field',
      const rust.InputFieldType.file(
        policy: rust.FileInputPolicyDto(
          maxCount: 1,
          extensions: <String>[],
          maxFileBytes: null,
          maxTotalBytes: null,
        ),
      ),
    ),
  ]) {
    testWidgets(
      'on_the_board_${name}_beside_a_text_field_announces_runs_on_input_'
      'once_on_its_own_node',
      (tester) async {
        final semantics = tester.ensureSemantics();
        try {
          final tool = _tool(otherType: otherType);
          await tester.pumpWidget(
            genericInlineHarness(
              tools: <rust.ToolDto>[tool],
              snapshot: rust.LayoutSnapshotDto(
                boardKey: 'dev',
                boardCols: 6,
                placements: <rust.PlacementDto>[
                  rust.PlacementDto(
                    toolId: tool.id,
                    pinId: tool.id,
                    x: 0,
                    y: 0,
                    w: 1,
                    h: 1,
                  ),
                ],
              ),
              dispatch: ({pinKey, required toolId, required args}) async =>
                  const rust.CanonicalToolResult(
                    ok: true,
                    outputs: <rust.CanonicalOutputEntry>[],
                  ),
            ),
          );
          await tester.pump();
          await tester.pump();

          final nodes = [
            for (final key in <String>['text', 'other'])
              tester.getSemantics(find.byKey(Key('field-$key'))),
          ];

          expect(nodes.map((node) => node.id).toSet(), hasLength(nodes.length));
          for (final node in nodes) {
            expect(node.hint, inlineUserInputSourceHint);
          }
        } finally {
          semantics.dispose();
        }
      },
    );
  }
}
