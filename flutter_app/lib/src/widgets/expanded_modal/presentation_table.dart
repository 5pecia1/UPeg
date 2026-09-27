/// A small, data-only collection view for presentation v1 JSON output.
///
/// It deliberately has no knowledge of toolkit names or dispatch. The modal
/// resolves rows/bindings in Rust, keeps the original JSON output visible,
/// and gives this widget safe scalar cells plus the action callbacks.
library;

import 'package:flutter/material.dart';

import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/expanded_modal/rich_presentation.dart';

@immutable
final class PresentationTableColumn {
  const PresentationTableColumn({required this.label, this.filterable = false});
  final String label;
  final bool filterable;
}

@immutable
final class PresentationTableRow {
  const PresentationTableRow({
    required this.key,
    required this.cells,
    required this.rawJson,
    this.cellTones = const [],
  });

  final String key;
  final List<Object?> cells;
  final String rawJson;
  final List<String?> cellTones;
}

@immutable
final class PresentationTableAction {
  const PresentationTableAction({
    required this.id,
    required this.label,
    required this.onPressed,
    this.enabled = true,
    this.disabledReason,
  });

  final String id;
  final String label;
  final ValueChanged<PresentationTableRow> onPressed;
  final bool enabled;
  final String? disabledReason;
}

class PresentationTable extends StatefulWidget {
  const PresentationTable({
    required this.columns,
    required this.rows,
    required this.actions,
    this.onReadRowNavigate,
    required this.rowActionsEnabled,
    required this.diagnostics,
    required this.tokens,
    required this.searchHint,
    required this.emptyLabel,
    this.noMatchesLabel = 'No matching rows',
    this.rowDetails = const {},
    super.key,
  });

  final List<PresentationTableColumn> columns;
  final List<PresentationTableRow> rows;
  final List<PresentationTableAction> actions;

  /// A declared read-only follow-up may open on one row click. Write actions
  /// deliberately remain behind their explicit action button and Run flow.
  final ValueChanged<PresentationTableRow>? onReadRowNavigate;
  final bool rowActionsEnabled;
  final List<String> diagnostics;
  final UpegTokens tokens;
  final String searchHint;
  final String emptyLabel;
  final String noMatchesLabel;
  final Map<String, PresentationDetail> rowDetails;

  @override
  State<PresentationTable> createState() => _PresentationTableState();
}

class _PresentationTableState extends State<PresentationTable> {
  String _query = '';
  String? _selectedKey;
  int? _filterColumn;
  String? _filterValue;
  int? _sortColumn;
  bool _sortAscending = true;

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
    final filterColumn = _filterColumn;
    final filterOptions = filterColumn == null
        ? <String>[]
        : widget.rows
              .map((row) => _scalarText(row.cells[filterColumn]))
              .toSet()
              .toList();
    filterOptions.sort();
    final visibleRows = widget.rows
        .where((row) {
          if (filterColumn != null &&
              _filterValue != null &&
              _scalarText(row.cells[filterColumn]) != _filterValue) {
            return false;
          }
          if (_query.isEmpty) return true;
          final needle = _query.toLowerCase();
          return row.key.toLowerCase().contains(needle) ||
              row.cells.any(
                (cell) => _scalarText(cell).toLowerCase().contains(needle),
              );
        })
        .toList(growable: false);
    if (_sortColumn case final index?) {
      visibleRows.sort((a, b) {
        final compared = _scalarText(
          a.cells[index],
        ).compareTo(_scalarText(b.cells[index]));
        return _sortAscending ? compared : -compared;
      });
    }
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
          if (widget.rows.isNotEmpty)
            TextField(
              key: const Key('presentation-table-search'),
              onChanged: (value) => setState(() => _query = value),
              decoration: InputDecoration(
                isDense: true,
                prefixIcon: Icon(Icons.search, size: 16),
                hintText: widget.searchHint,
              ),
            ),
          if (widget.rows.isNotEmpty &&
              widget.columns.any((column) => column.filterable)) ...[
            const SizedBox(height: 6),
            Row(
              children: [
                DropdownButton<int?>(
                  key: const Key('presentation-table-filter-column'),
                  value: _filterColumn,
                  hint: const Text('Filter'),
                  items: [
                    const DropdownMenuItem<int?>(
                      value: null,
                      child: Text('All'),
                    ),
                    for (var index = 0; index < widget.columns.length; index++)
                      if (widget.columns[index].filterable)
                        DropdownMenuItem<int?>(
                          value: index,
                          child: Text(widget.columns[index].label),
                        ),
                  ],
                  onChanged: (index) => setState(() {
                    _filterColumn = index;
                    _filterValue = null;
                  }),
                ),
                if (filterColumn != null) ...[
                  const SizedBox(width: 8),
                  DropdownButton<String?>(
                    key: const Key('presentation-table-filter-value'),
                    value: _filterValue,
                    hint: const Text('Value'),
                    items: [
                      const DropdownMenuItem<String?>(
                        value: null,
                        child: Text('All'),
                      ),
                      for (final option in filterOptions)
                        DropdownMenuItem<String?>(
                          value: option,
                          child: Text(option),
                        ),
                    ],
                    onChanged: (value) => setState(() => _filterValue = value),
                  ),
                ],
              ],
            ),
          ],
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
          LayoutBuilder(
            builder: (context, constraints) {
              final list = Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  if (widget.rows.isNotEmpty)
                    _Header(
                      columns: widget.columns,
                      tokens: widget.tokens,
                      sortColumn: _sortColumn,
                      sortAscending: _sortAscending,
                      onSort: (index) => setState(() {
                        if (_sortColumn == index) {
                          _sortAscending = !_sortAscending;
                        } else {
                          _sortColumn = index;
                          _sortAscending = true;
                        }
                      }),
                    ),
                  for (final row in visibleRows)
                    _Row(
                      row: row,
                      selected: row.key == _selectedKey,
                      tokens: widget.tokens,
                      onTap: () {
                        setState(() => _selectedKey = row.key);
                        widget.onReadRowNavigate?.call(row);
                      },
                    ),
                  if (visibleRows.isEmpty)
                    Padding(
                      padding: const EdgeInsets.symmetric(vertical: 12),
                      child: Text(
                        widget.rows.isEmpty
                            ? widget.emptyLabel
                            : widget.noMatchesLabel,
                        key: const Key('presentation-table-empty'),
                        style: TextStyle(
                          color: widget.tokens.fg4,
                          fontSize: 11,
                        ),
                      ),
                    ),
                ],
              );
              final detail = selected == null
                  ? null
                  : widget.rowDetails[selected.key];
              if (detail == null) return list;
              final detailPanel = RichPresentationPanel(
                view: RichPresentationView(detail: detail),
                tokens: widget.tokens,
              );
              if (constraints.maxWidth < 700) {
                return Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [list, const SizedBox(height: 10), detailPanel],
                );
              }
              return Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Expanded(flex: 3, child: list),
                  const SizedBox(width: 16),
                  Expanded(flex: 2, child: detailPanel),
                ],
              );
            },
          ),
          if (selected != null && widget.actions.isNotEmpty) ...[
            const SizedBox(height: 8),
            Wrap(
              spacing: 8,
              children: [
                for (final action in widget.actions)
                  OutlinedButton(
                    key: Key('presentation-table-action-${action.id}'),
                    onPressed: widget.rowActionsEnabled && action.enabled
                        ? () => action.onPressed(selected)
                        : null,
                    child: Text(action.label),
                  ),
                for (final action in widget.actions)
                  if (!action.enabled && action.disabledReason != null)
                    Text(
                      action.disabledReason!,
                      style: TextStyle(color: widget.tokens.fg2, fontSize: 10),
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
  const _Header({
    required this.columns,
    required this.tokens,
    required this.sortColumn,
    required this.sortAscending,
    required this.onSort,
  });
  final List<PresentationTableColumn> columns;
  final UpegTokens tokens;
  final int? sortColumn;
  final bool sortAscending;
  final ValueChanged<int> onSort;

  @override
  Widget build(BuildContext context) => Row(
    children: [
      for (var index = 0; index < columns.length; index++)
        Expanded(
          child: InkWell(
            key: Key('presentation-table-sort-$index'),
            onTap: () => onSort(index),
            child: Text(
              '${columns[index].label}${sortColumn == index ? (sortAscending ? ' ↑' : ' ↓') : ''}',
              style: TextStyle(
                color: tokens.fg4,
                fontSize: 10,
                fontWeight: FontWeight.w600,
              ),
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
            for (var index = 0; index < row.cells.length; index++)
              Expanded(
                child: _Cell(
                  value: row.cells[index],
                  tone: index < row.cellTones.length
                      ? row.cellTones[index]
                      : null,
                  tokens: tokens,
                ),
              ),
          ],
        ),
      ),
    ),
  );
}

class _Cell extends StatelessWidget {
  const _Cell({required this.value, required this.tone, required this.tokens});
  final Object? value;
  final String? tone;
  final UpegTokens tokens;

  @override
  Widget build(BuildContext context) {
    final text = Text(
      _scalarText(value),
      overflow: TextOverflow.ellipsis,
      style: TextStyle(color: tokens.fg, fontSize: 11),
    );
    if (tone == null) return text;
    final color = switch (tone) {
      'success' => tokens.accent,
      'warning' || 'error' => tokens.warn,
      _ => tokens.surface2,
    };
    return Align(
      alignment: Alignment.centerLeft,
      child: Container(
        key: Key('presentation-table-badge-$tone'),
        padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 3),
        decoration: BoxDecoration(
          color: color,
          borderRadius: BorderRadius.circular(UpegSizing.radius1),
        ),
        child: text,
      ),
    );
  }
}

String _scalarText(Object? value) => switch (value) {
  null => '—',
  String() || num() || bool() => '$value',
  _ => '—',
};
