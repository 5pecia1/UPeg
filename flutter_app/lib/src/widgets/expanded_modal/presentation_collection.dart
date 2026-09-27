library;

import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/state/presentation_resolver_provider.dart';
import 'package:upeg/src/state/tools_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/expanded_modal/presentation_table.dart';
import 'package:upeg/src/widgets/expanded_modal/rich_presentation.dart';

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
    required this.onReadRowNavigate,
    super.key,
  });

  final ToolDto tool;
  final CanonicalToolResult outcome;
  final UpegTokens tokens;
  final void Function(PresentationActionDto action, PresentationTableRow row)
  onRowAction;
  final void Function(PresentationActionDto action) onResultAction;
  final void Function(PresentationActionDto action, PresentationTableRow row)
  onReadRowNavigate;

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
    final view = ref.watch(presentationViewResolverProvider)(
      toolId: tool.id,
      outputsJson: jsonEncode(outcome.jsonValues),
    );
    final availability = {for (final item in view.actions) item.id: item};
    final rowDetails = <String, PresentationDetail>{
      for (final item in view.rowDetails)
        item.key: PresentationDetail(
          fields: item.detail.fields
              .map(
                (field) => PresentationTextValue(
                  label: field.label,
                  value: field.value,
                ),
              )
              .toList(growable: false),
          markdown: item.detail.markdown,
          diff: item.detail.diff,
        ),
    };
    final rowActions = presentation.actions
        .where((action) => action.scope == ActionScopeDto.row)
        .toList(growable: false);
    final resultActions = presentation.actions
        .where((action) => action.scope == ActionScopeDto.result)
        .toList(growable: false);
    final readRowActions = rowActions
        .where((action) {
          final target = ref.read(
            toolByIdProvider(ToolId.parse(action.targetTool)),
          );
          return target?.effect == ToolEffectDto.read;
        })
        .toList(growable: false);
    return Padding(
      padding: const EdgeInsets.only(top: 10),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          PresentationTable(
            columns: presentation.columns
                .map(
                  (column) => PresentationTableColumn(
                    label: column.label,
                    filterable: column.filterable,
                  ),
                )
                .toList(growable: false),
            rows: resolved.rows
                .map(
                  (row) => PresentationTableRow(
                    key: row.key,
                    rawJson: row.valueJson,
                    cellTones: row.cellTones,
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
                    enabled: availability[action.id]?.enabled ?? true,
                    disabledReason: availability[action.id]?.reason,
                  ),
                )
                .toList(growable: false),
            onReadRowNavigate: readRowActions.length == 1
                ? (row) => onReadRowNavigate(readRowActions.single, row)
                : null,
            rowActionsEnabled: resolved.rowActionsEnabled,
            diagnostics: [...resolved.diagnostics, ...view.diagnostics],
            tokens: tokens,
            searchHint: t(ref, 'modal.presentation.search'),
            emptyLabel: view.emptyMessage ?? t(ref, 'modal.presentation.empty'),
            noMatchesLabel: t(ref, 'modal.presentation.empty'),
            rowDetails: rowDetails,
          ),
          if (resultActions.isNotEmpty) ...[
            const SizedBox(height: 8),
            Wrap(
              spacing: 8,
              children: [
                for (final action in resultActions)
                  OutlinedButton(
                    key: Key('presentation-result-action-${action.id}'),
                    onPressed: availability[action.id]?.enabled ?? true
                        ? () => onResultAction(action)
                        : null,
                    child: Text(action.label),
                  ),
                for (final action in resultActions)
                  if (availability[action.id]?.enabled == false &&
                      availability[action.id]?.reason != null)
                    Text(
                      availability[action.id]!.reason!,
                      style: TextStyle(color: tokens.fg2, fontSize: 10),
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
