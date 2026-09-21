/// A small, data-only collection view for presentation v1 JSON output.
///
/// It deliberately has no knowledge of toolkit names or dispatch. The modal
/// resolves rows/bindings in Rust, keeps the original JSON output visible,
/// and gives this widget safe scalar cells plus the action callbacks.
library;

import 'package:flutter/material.dart';

import 'package:upeg/src/theme/upeg_theme.dart';

@immutable
final class PresentationTableColumn {
  const PresentationTableColumn({required this.label});
  final String label;
}

@immutable
final class PresentationTableRow {
  const PresentationTableRow({
    required this.key,
    required this.cells,
    required this.rawJson,
  });

  final String key;
  final List<Object?> cells;
  final String rawJson;
}

@immutable
final class PresentationTableAction {
  const PresentationTableAction({
    required this.id,
    required this.label,
    required this.onPressed,
  });

  final String id;
  final String label;
  final ValueChanged<PresentationTableRow> onPressed;
}

class PresentationTable extends StatefulWidget {
  const PresentationTable({
    required this.columns,
    required this.rows,
    required this.actions,
    required this.rowActionsEnabled,
    required this.diagnostics,
    required this.tokens,
    required this.searchHint,
    required this.emptyLabel,
    super.key,
  });

  final List<PresentationTableColumn> columns;
  final List<PresentationTableRow> rows;
  final List<PresentationTableAction> actions;
  final bool rowActionsEnabled;
  final List<String> diagnostics;
  final UpegTokens tokens;
  final String searchHint;
  final String emptyLabel;

  @override
  State<PresentationTable> createState() => _PresentationTableState();
}

class _PresentationTableState extends State<PresentationTable> {
  String _query = '';
  String? _selectedKey;

  @override
  void didUpdateWidget(PresentationTable oldWidget) {
    super.didUpdateWidget(oldWidget);
    final selected = _selectedKey;
    if (selected != null && !widget.rows.any((row) => row.key == selected)) {
      _selectedKey = null;
    }
  }

  @override
  Widget build(BuildContext context) {
    final visibleRows = widget.rows
        .where((row) {
          if (_query.isEmpty) return true;
          final needle = _query.toLowerCase();
          return row.key.toLowerCase().contains(needle) ||
              row.cells.any(
                (cell) => _scalarText(cell).toLowerCase().contains(needle),
              );
        })
        .toList(growable: false);
    final selected = _selectedKey == null
        ? null
        : widget.rows.cast<PresentationTableRow?>().firstWhere(
            (row) => row?.key == _selectedKey,
            orElse: () => null,
          );

    return Container(
      key: const Key('presentation-table'),
      padding: const EdgeInsets.all(10),
      decoration: BoxDecoration(
        border: Border.all(color: widget.tokens.line),
        borderRadius: BorderRadius.circular(UpegSizing.radius2),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          TextField(
            key: const Key('presentation-table-search'),
            onChanged: (value) => setState(() => _query = value),
            decoration: InputDecoration(
              isDense: true,
              prefixIcon: Icon(Icons.search, size: 16),
              hintText: widget.searchHint,
            ),
          ),
          if (widget.diagnostics.isNotEmpty) ...[
            const SizedBox(height: 8),
            for (final diagnostic in widget.diagnostics)
              Text(
                diagnostic,
                key: Key('presentation-table-diagnostic-$diagnostic'),
                style: TextStyle(color: widget.tokens.warn, fontSize: 11),
              ),
          ],
          const SizedBox(height: 8),
          _Header(columns: widget.columns, tokens: widget.tokens),
          for (final row in visibleRows)
            _Row(
              row: row,
              selected: row.key == _selectedKey,
              tokens: widget.tokens,
              onTap: () => setState(() => _selectedKey = row.key),
            ),
          if (visibleRows.isEmpty)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 12),
              child: Text(
                widget.emptyLabel,
                key: const Key('presentation-table-empty'),
                style: TextStyle(color: widget.tokens.fg4, fontSize: 11),
              ),
            ),
          if (selected != null && widget.actions.isNotEmpty) ...[
            const SizedBox(height: 8),
            Wrap(
              spacing: 8,
              children: [
                for (final action in widget.actions)
                  OutlinedButton(
                    key: Key('presentation-table-action-${action.id}'),
                    onPressed: widget.rowActionsEnabled
                        ? () => action.onPressed(selected)
                        : null,
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

class _Header extends StatelessWidget {
  const _Header({required this.columns, required this.tokens});
  final List<PresentationTableColumn> columns;
  final UpegTokens tokens;

  @override
  Widget build(BuildContext context) => Row(
    children: [
      for (final column in columns)
        Expanded(
          child: Text(
            column.label,
            style: TextStyle(
              color: tokens.fg4,
              fontSize: 10,
              fontWeight: FontWeight.w600,
            ),
          ),
        ),
    ],
  );
}

class _Row extends StatelessWidget {
  const _Row({
    required this.row,
    required this.selected,
    required this.tokens,
    required this.onTap,
  });
  final PresentationTableRow row;
  final bool selected;
  final UpegTokens tokens;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) => Material(
    color: selected ? tokens.bg2 : Colors.transparent,
    child: InkWell(
      key: Key('presentation-table-row-${row.key}'),
      onTap: onTap,
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 7),
        child: Row(
          children: [
            for (final cell in row.cells)
              Expanded(
                child: Text(
                  _scalarText(cell),
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(color: tokens.fg, fontSize: 11),
                ),
              ),
          ],
        ),
      ),
    ),
  );
}

String _scalarText(Object? value) => switch (value) {
  null => '—',
  String() || num() || bool() => '$value',
  _ => '—',
};
