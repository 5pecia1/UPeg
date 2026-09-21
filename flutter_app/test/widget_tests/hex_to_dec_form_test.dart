/// Widget tests for the `num.hex_to_decimal` bespoke form (Batch K3 / I03,
/// I13 partial).
///
/// Covers:
/// - Live decode (every keystroke shows the decimal).
/// - F1 → onSubmit with typed args (`{'input': '<hex>'}`).
/// - F2 → copy the currently-decoded decimal to clipboard.
/// - Invalid input surfaces an error message and disables F1/F2.
/// - Registry entry is present.
library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';
import 'package:upeg/src/widgets/expanded_modal/bespoke_forms/hex_to_dec_form.dart';
import 'package:upeg/src/widgets/expanded_modal/bespoke_forms/registry.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

import '../test_helpers/tool_fixture.dart';

import '../test_helpers/i18n_test_catalog.dart';

class _RecordingClipboardWriter extends ClipboardWriter {
  final List<String> writes = <String>[];

  @override
  Future<void> write(String text) async {
    writes.add(text);
  }
}

ToolDto _hexTool() => fixtureToolDto(
  id: 'num.hex_to_decimal',
  toolkit: 'convert',
  label: 'hex → dec',
  inputFields: const <InputFieldDto>[
    InputFieldDto(
      key: 'input',
      label: 'input',
      fieldType: InputFieldType_Text(),
      required_: true,
    ),
  ],
);

Widget _harness({required HexToDecForm form}) {
  return ProviderScope(
    overrides: [...i18nTestOverrides],
    child: MaterialApp(home: Scaffold(body: form)),
  );
}

void main() {
  group('HexToDecForm', () {
    testWidgets('hextodecform_shows_the_decimal_immediately_on_input', (
      tester,
    ) async {
      await tester.pumpWidget(
        _harness(
          form: HexToDecForm(tool: _hexTool(), onSubmit: (_) {}),
        ),
      );
      await tester.enterText(find.byType(TextField), 'ff');
      await tester.pump();
      expect(find.text('255'), findsOneWidget);
    });

    testWidgets(
      'hextodecform_prefills_initialinput_into_the_input_and_decodes_immediately',
      (tester) async {
        // When a deep link `upeg://open?...&input=0x2a` carries a raw scalar,
        // the modal must open with that value rather than an empty form.
        await tester.pumpWidget(
          _harness(
            form: HexToDecForm(
              tool: _hexTool(),
              onSubmit: (_) {},
              initialInput: ToolArgs.fromJsonObject(const <String, Object?>{
                'input': '0x2a',
              }),
            ),
          ),
        );
        await tester.pump();
        expect(find.text('0x2a'), findsOneWidget);
        expect(find.text('42'), findsOneWidget);
      },
    );

    testWidgets('hextodecform_decodes_a_0x_prefix_immediately', (tester) async {
      await tester.pumpWidget(
        _harness(
          form: HexToDecForm(tool: _hexTool(), onSubmit: (_) {}),
        ),
      );
      await tester.enterText(find.byType(TextField), '0xCAFE');
      await tester.pump();
      expect(find.text('51966'), findsOneWidget);
    });

    testWidgets('hextodecform_f1_invokes_onsubmit', (tester) async {
      ToolArgs? observedArgs;
      await tester.pumpWidget(
        _harness(
          form: HexToDecForm(
            tool: _hexTool(),
            onSubmit: (args) {
              observedArgs = args;
            },
          ),
        ),
      );
      await tester.enterText(find.byType(TextField), 'ff');
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.f1);
      await tester.pumpAndSettle();
      expect(observedArgs, isNotNull);
      expect(observedArgs!.toJsonObject(), {'input': 'ff'});
    });

    testWidgets('hextodecform_run_button_tap_also_invokes_onsubmit', (
      tester,
    ) async {
      ToolArgs? observedArgs;
      await tester.pumpWidget(
        _harness(
          form: HexToDecForm(
            tool: _hexTool(),
            onSubmit: (args) {
              observedArgs = args;
            },
          ),
        ),
      );
      await tester.enterText(find.byType(TextField), 'ff');
      await tester.pump();
      await tester.tap(find.byKey(const Key('hex-to-dec-run-btn')));
      await tester.pumpAndSettle();
      expect(observedArgs!.toJsonObject(), {'input': 'ff'});
    });

    testWidgets('hextodecform_f2_copies_the_decoded_decimal_to_the_clipboard', (
      tester,
    ) async {
      final writer = _RecordingClipboardWriter();
      await tester.pumpWidget(
        _harness(
          form: HexToDecForm(
            tool: _hexTool(),
            onSubmit: (_) {},
            clipboardWriter: writer,
          ),
        ),
      );
      await tester.enterText(find.byType(TextField), 'ff');
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.f2);
      await tester.pumpAndSettle();
      expect(writer.writes, ['255']);
    });

    testWidgets('hextodecform_shows_an_error_message_for_invalid_input', (
      tester,
    ) async {
      await tester.pumpWidget(
        _harness(
          form: HexToDecForm(tool: _hexTool(), onSubmit: (_) {}),
        ),
      );
      await tester.enterText(find.byType(TextField), 'xyz');
      await tester.pump();
      expect(find.textContaining('not a valid hex'), findsOneWidget);
    });

    testWidgets('hextodecform_ignores_f1_on_empty_input', (tester) async {
      var calls = 0;
      await tester.pumpWidget(
        _harness(
          form: HexToDecForm(
            tool: _hexTool(),
            onSubmit: (_) {
              calls += 1;
            },
          ),
        ),
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.f1);
      await tester.pumpAndSettle();
      expect(calls, 0);
    });

    testWidgets('hextodecform_ignores_f2_on_invalid_input', (tester) async {
      final writer = _RecordingClipboardWriter();
      await tester.pumpWidget(
        _harness(
          form: HexToDecForm(
            tool: _hexTool(),
            onSubmit: (_) {},
            clipboardWriter: writer,
          ),
        ),
      );
      await tester.enterText(find.byType(TextField), 'xyz');
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.f2);
      await tester.pumpAndSettle();
      expect(writer.writes, isEmpty);
    });
  });

  group('bespokeRegistry hex_to_dec', () {
    test('bespokeregistry_builds_hextodecform_for_the_hex_to_dec_id', () {
      final builder = bespokeFor(ToolId.parse('num.hex_to_decimal'));
      expect(builder, isNotNull);
    });
  });
}
