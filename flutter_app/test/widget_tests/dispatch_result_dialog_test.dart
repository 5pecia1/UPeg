/// Widget tests for [DispatchResultDialog].
///
/// Pure render test — the dialog is a stateless widget over a
/// [CanonicalToolResult], no provider overrides needed.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/widgets/dispatch_result_dialog.dart';

import '../test_helpers/i18n_test_catalog.dart';

Widget _harness(CanonicalToolResult outcome, {ToolId? toolId}) {
  return ProviderScope(
    overrides: [...i18nTestOverrides],
    child: MaterialApp(
      home: Scaffold(
        body: DispatchResultDialog(
          toolId: toolId ?? ToolId.parse('demo.echo'),
          outcome: outcome,
        ),
      ),
    ),
  );
}

void main() {
  group('DispatchResultDialog', () {
    testWidgets('DispatchResultDialog_shows_canonical_output_on_ok', (
      tester,
    ) async {
      const outcome = CanonicalToolResult(
        ok: true,
        primaryOutputId: 'result',
        outputs: [
          CanonicalOutputEntry(
            id: 'result',
            label: 'Result',
            kind: 'string',
            value: CanonicalOutputValue.string(value: 'hello world'),
          ),
        ],
      );
      await tester.pumpWidget(
        _harness(outcome, toolId: ToolId.parse('demo.echo')),
      );
      await tester.pumpAndSettle();

      expect(find.text('demo.echo'), findsOneWidget);
      expect(find.text('ok'), findsOneWidget);
      expect(find.text('hello world'), findsOneWidget);
      expect(find.text('Result:'), findsOneWidget);
      expect(find.text('primary'), findsOneWidget);
    });

    testWidgets('DispatchResultDialog_shows_canonical_json_output', (
      tester,
    ) async {
      const outcome = CanonicalToolResult(
        ok: true,
        primaryOutputId: 'result',
        outputs: [
          CanonicalOutputEntry(
            id: 'result',
            label: null,
            kind: 'json',
            value: CanonicalOutputValue.json(value: '{"result":255}'),
          ),
        ],
      );
      await tester.pumpWidget(
        _harness(outcome, toolId: ToolId.parse('num.hex_to_decimal')),
      );
      await tester.pumpAndSettle();

      expect(find.text('result:'), findsOneWidget);
      expect(find.text('{"result":255}'), findsOneWidget);
    });

    testWidgets('DispatchResultDialog_shows_the_error_message_on_failure', (
      tester,
    ) async {
      const outcome = CanonicalToolResult(
        ok: false,
        outputs: [],
        error: CanonicalToolError(
          code: 'dispatch_failed',
          message: 'tool blew up',
        ),
      );
      await tester.pumpWidget(
        _harness(outcome, toolId: ToolId.parse('demo.broken')),
      );
      await tester.pumpAndSettle();

      expect(find.text('demo.broken'), findsOneWidget);
      expect(find.text('error'), findsOneWidget);
      expect(find.text('tool blew up'), findsOneWidget);
      expect(find.text('error:'), findsOneWidget);
    });

    testWidgets(
      'DispatchResultDialog_shows_a_no_output_hint_on_empty_outcome',
      (tester) async {
        const outcome = CanonicalToolResult(ok: true, outputs: []);
        await tester.pumpWidget(_harness(outcome));
        await tester.pumpAndSettle();

        expect(find.textContaining('no output'), findsOneWidget);
      },
    );
  });
}
