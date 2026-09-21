library;

import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/state/presentation_resolver_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/expanded_modal/presentation_table.dart';

/// Adapts the generated presentation DTO to the small Dart collection view.
/// Resolution remains in Rust; malformed cell JSON stays a non-actionable
/// placeholder instead of becoming executable or formatted content.
class PresentationCollection extends ConsumerWidget {
  const PresentationCollection({
    required this.tool,
    required this.outcome,
    required this.tokens,
    required this.onRowAction,
    required this.onResultAction,
    super.key,
  });

  final ToolDto tool;
  final CanonicalToolResult outcome;
  final UpegTokens tokens;
  final void Function(PresentationActionDto action, PresentationTableRow row)
  onRowAction;
  final void Function(PresentationActionDto action) onResultAction;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final presentation = tool.presentation;
    if (!outcome.ok || presentation == null || presentation.version != 1) {
      return const SizedBox.shrink();
    }
    final resolved = ref.watch(presentationRowsResolverProvider)(
      toolId: tool.id,
      outputsJson: jsonEncode(outcome.jsonValues),
    );
    final rowActions = presentation.actions
        .where((action) => action.scope == ActionScopeDto.row)
        .toList(growable: false);
    final resultActions = presentation.actions
        .where((action) => action.scope == ActionScopeDto.result)
        .toList(growable: false);
    return Padding(
      padding: const EdgeInsets.only(top: 10),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          PresentationTable(
            columns: presentation.columns
                .map((column) => PresentationTableColumn(label: column.label))
                .toList(growable: false),
            rows: resolved.rows
                .map(
                  (row) => PresentationTableRow(
                    key: row.key,
                    rawJson: row.valueJson,
                    cells: row.cellsJson
                        .map(_decodeCell)
                        .toList(growable: false),
                  ),
                )
                .toList(growable: false),
            actions: rowActions
                .map(
                  (action) => PresentationTableAction(
                    id: action.id,
                    label: action.label,
                    onPressed: (row) => onRowAction(action, row),
                  ),
                )
                .toList(growable: false),
            rowActionsEnabled: resolved.rowActionsEnabled,
            diagnostics: resolved.diagnostics,
            tokens: tokens,
            searchHint: t(ref, 'modal.presentation.search'),
            emptyLabel: t(ref, 'modal.presentation.empty'),
          ),
          if (resultActions.isNotEmpty) ...[
            const SizedBox(height: 8),
            Wrap(
              spacing: 8,
              children: [
                for (final action in resultActions)
                  OutlinedButton(
                    key: Key('presentation-result-action-${action.id}'),
                    onPressed: () => onResultAction(action),
                    child: Text(action.label),
                  ),
              ],
            ),
          ],
        ],
      ),
    );
  }
}

Object? _decodeCell(String encoded) {
  try {
    final value = jsonDecode(encoded);
    return switch (value) {
      null || String() || num() || bool() => value,
      _ => null,
    };
  } on FormatException {
    return null;
  }
}
