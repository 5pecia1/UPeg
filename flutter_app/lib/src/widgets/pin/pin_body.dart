/// The `.cell-bd` slot: label plus either canonical output rows derived
/// from the tool's output schema or the description fallback.
library;

import 'package:flutter/material.dart';
import 'package:upeg/src/rust/api/tools.dart'
    show CanonicalToolResult, OutputFieldDto;
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/theme/upeg_theme.dart';

/// Trailing marker appended to output previews whose persisted payload
/// was truncated by the store's size cap.
const String _truncatedPreviewSuffix = '…';

class PinBody extends StatelessWidget {
  const PinBody({
    required this.tokens,
    required this.label,
    required this.description,
    required this.outputFields,
    required this.outputResult,
    this.richStatus,
    this.richSummary,
    this.truncated = false,
    super.key,
  });

  final UpegTokens tokens;
  final String? label;
  final String? description;

  /// Ordered fields from the tool's `output_spec.fields`. Drives the
  /// auto-render path: when non-empty, the description fallback is
  /// suppressed and one line per field label is emitted instead.
  final List<OutputFieldDto> outputFields;

  /// Canonical runtime result. Rows use the shared canonical presenter;
  /// null preserves label-only rendering from the output schema.
  final CanonicalToolResult? outputResult;
  final String? richStatus;
  final String? richSummary;

  /// True when the persisted outputs were capped — row previews get a
  /// trailing ellipsis so the cut is honest.
  final bool truncated;

  @override
  Widget build(BuildContext context) {
    final outputRows = _pinOutputRows(
      outputFields: outputFields,
      outputResult: outputResult,
      truncated: truncated,
    );
    return Padding(
      padding: UpegSizing.pinBodyPadding,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          if (richStatus case final status?)
            Text(status, style: TextStyle(color: tokens.fg3, fontSize: 10)),
          if (richSummary case final summary?)
            Text(
              summary,
              key: const Key('pin-rich-summary'),
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                color: tokens.fg,
                fontSize: 11,
                fontWeight: FontWeight.w600,
              ),
            ),
          if (label != null && label!.isNotEmpty)
            Text(
              label!,
              style: TextStyle(
                fontFamily: upegMonoFontFamily,
                fontFamilyFallback: upegMonoFontFamilyFallback,
                fontSize: 12,
                color: tokens.fg2,
                fontWeight: FontWeight.w500,
                height: 1.3,
              ),
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
            ),
          if (outputRows.isNotEmpty &&
              richSummary == null &&
              richStatus == null)
            Expanded(
              child: Padding(
                padding: const EdgeInsets.only(top: 4),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    for (final row in outputRows)
                      _PinOutputRow(
                        key: ValueKey('pin-output-${row.id}'),
                        tokens: tokens,
                        label: row.label,
                        value: row.value,
                      ),
                  ],
                ),
              ),
            )
          else if (richSummary == null &&
              richStatus == null &&
              description != null &&
              description!.isNotEmpty)
            Expanded(
              child: Padding(
                padding: const EdgeInsets.only(top: 4),
                child: Text(
                  description!,
                  style: TextStyle(
                    fontFamily: upegMonoFontFamily,
                    fontFamilyFallback: upegMonoFontFamilyFallback,
                    fontSize: 10.5,
                    color: tokens.fg3,
                    height: 1.35,
                  ),
                  maxLines: 3,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ),
        ],
      ),
    );
  }
}

List<_PinOutputRowData> _pinOutputRows({
  required List<OutputFieldDto> outputFields,
  required CanonicalToolResult? outputResult,
  bool truncated = false,
}) {
  final rows = _pinOutputRowsUntruncated(
    outputFields: outputFields,
    outputResult: outputResult,
  );
  if (!truncated) return rows;
  // The store capped this result's outputs — mark every surviving
  // preview so the cut is visible instead of silently short.
  return <_PinOutputRowData>[
    for (final row in rows)
      row.value == null
          ? row
          : _PinOutputRowData(
              id: row.id,
              label: row.label,
              value: '${row.value}$_truncatedPreviewSuffix',
            ),
  ];
}

List<_PinOutputRowData> _pinOutputRowsUntruncated({
  required List<OutputFieldDto> outputFields,
  required CanonicalToolResult? outputResult,
}) {
  final result = outputResult;
  if (result == null) {
    return <_PinOutputRowData>[
      for (final field in outputFields)
        _PinOutputRowData(id: field.key, label: field.label),
    ];
  }

  final structuredValues = result.structuredValues;
  final outputRows = result.displayRows(outputFields);
  if (outputFields.isEmpty) {
    final primaryText = result.primaryOutputText;
    if (primaryText.isNotEmpty && outputRows.isNotEmpty) {
      final primaryRow = outputRows.firstWhere(
        (row) => row.isPrimary,
        orElse: () => outputRows.first,
      );
      return <_PinOutputRowData>[
        _PinOutputRowData(
          id: primaryRow.id,
          label: primaryRow.label,
          value: primaryText,
        ),
      ];
    }
    return <_PinOutputRowData>[
      for (final row in outputRows)
        _PinOutputRowData(id: row.id, label: row.label, value: row.value),
    ];
  }

  final rowsById = <String, CanonicalOutputDisplayRow>{
    for (final row in outputRows) row.id: row,
  };
  return <_PinOutputRowData>[
    for (final field in outputFields)
      _PinOutputRowData(
        id: field.key,
        label: field.label,
        value: structuredValues.containsKey(field.key)
            ? rowsById[field.key]?.value
            : null,
      ),
  ];
}

class _PinOutputRowData {
  const _PinOutputRowData({required this.id, required this.label, this.value});

  final String id;
  final String label;
  final String? value;
}

class _PinOutputRow extends StatelessWidget {
  const _PinOutputRow({
    required this.tokens,
    required this.label,
    required this.value,
    super.key,
  });

  final UpegTokens tokens;
  final String label;
  final String? value;

  @override
  Widget build(BuildContext context) {
    final labelStyle = TextStyle(
      fontFamily: upegMonoFontFamily,
      fontFamilyFallback: upegMonoFontFamilyFallback,
      fontSize: 10.5,
      color: tokens.fg3,
      height: 1.35,
    );
    final valueStyle = TextStyle(
      fontFamily: upegMonoFontFamily,
      fontFamilyFallback: upegMonoFontFamilyFallback,
      fontSize: 10.5,
      color: tokens.fg,
      height: 1.35,
      fontWeight: FontWeight.w500,
    );
    final v = value;
    if (v == null) {
      return Text(
        label,
        style: labelStyle,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
      );
    }
    return Row(
      children: [
        Text(label, style: labelStyle),
        const SizedBox(width: 6),
        Expanded(
          child: Text(
            v,
            textAlign: TextAlign.right,
            style: valueStyle,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
          ),
        ),
      ],
    );
  }
}
