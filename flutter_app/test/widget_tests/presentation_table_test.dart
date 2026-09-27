import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/expanded_modal/presentation_table.dart';

Widget _harness({
  required List<PresentationTableRow> rows,
  required bool rowActionsEnabled,
  required List<String> diagnostics,
  required ValueChanged<PresentationTableRow> onAction,
  ValueChanged<PresentationTableRow>? onReadRowNavigate,
}) {
  final tokens = UpegTheme.darkTheme().extension<UpegTokens>()!;
  return MaterialApp(
    theme: UpegTheme.darkTheme(),
    home: Scaffold(
      body: PresentationTable(
        columns: const [
          PresentationTableColumn(label: 'Name'),
          PresentationTableColumn(label: 'Enabled'),
        ],
        onReadRowNavigate: onReadRowNavigate,
        rows: rows,
        actions: [
          PresentationTableAction(
            id: 'open',
            label: 'Open',
            onPressed: onAction,
          ),
        ],
        rowActionsEnabled: rowActionsEnabled,
        diagnostics: diagnostics,
        tokens: tokens,
        searchHint: 'Search results',
        emptyLabel: 'No matching rows',
      ),
    ),
  );
}

void main() {
  const alpha = PresentationTableRow(
    key: 'alpha',
    cells: ['Alpha', true],
    rawJson: '{"name":"Alpha"}',
  );
  const beta = PresentationTableRow(
    key: 'beta',
    cells: ['Beta', false],
    rawJson: '{"name":"Beta"}',
  );

  testWidgets('filters received rows and sends the selected row to an action', (
    tester,
  ) async {
    PresentationTableRow? actionRow;
    await tester.pumpWidget(
      _harness(
        rows: const [alpha, beta],
        rowActionsEnabled: true,
        diagnostics: const [],
        onAction: (row) => actionRow = row,
      ),
    );

    await tester.enterText(
      find.byKey(const Key('presentation-table-search')),
      'beta',
    );
    await tester.pump();

    expect(find.byKey(const Key('presentation-table-row-alpha')), findsNothing);
    await tester.tap(find.byKey(const Key('presentation-table-row-beta')));
    await tester.pump();
    await tester.tap(find.byKey(const Key('presentation-table-action-open')));

    expect(actionRow?.key, 'beta');
    expect(actionRow?.rawJson, '{"name":"Beta"}');
  });

  testWidgets('shows diagnostics and disables actions for invalid row keys', (
    tester,
  ) async {
    var called = false;
    await tester.pumpWidget(
      _harness(
        rows: const [alpha],
        rowActionsEnabled: false,
        diagnostics: const ['row key `alpha` is duplicated'],
        onAction: (_) => called = true,
      ),
    );

    await tester.tap(find.byKey(const Key('presentation-table-row-alpha')));
    await tester.pump();
    final action = tester.widget<OutlinedButton>(
      find.byKey(const Key('presentation-table-action-open')),
    );

    expect(find.text('row key `alpha` is duplicated'), findsOneWidget);
    expect(action.onPressed, isNull);
    expect(called, isFalse);
  });

  testWidgets(
    'forwards a single click only when its caller enables read navigation',
    (tester) async {
      PresentationTableRow? navigated;
      await tester.pumpWidget(
        _harness(
          rows: const [alpha],
          rowActionsEnabled: true,
          diagnostics: const [],
          onAction: (_) {},
          onReadRowNavigate: (row) => navigated = row,
        ),
      );

      await tester.tap(find.byKey(const Key('presentation-table-row-alpha')));

      expect(navigated?.key, 'alpha');
    },
  );
}
