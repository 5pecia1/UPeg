/// Result block rendered under the expanded modal's form.
///
/// Split out of `expanded_modal_page.dart` to keep that page inside the
/// hand-written Dart file budget. Owns only presentation: the canonical
/// result is already shaped by `CanonicalToolResultView`.
library;

import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/presentation_view.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';
import 'package:upeg/src/widgets/expanded_modal/error_details_block.dart';
import 'package:upeg/src/widgets/expanded_modal/structured_output.dart';
import 'package:upeg/src/widgets/expanded_modal/presentation_collection.dart';
import 'package:upeg/src/widgets/expanded_modal/presentation_table.dart';
import 'package:upeg/src/widgets/expanded_modal/rich_presentation.dart';
import 'package:upeg/src/state/presentation_resolver_provider.dart';

class OutcomeBlock extends ConsumerWidget {
  const OutcomeBlock({
    required this.outcome,
    required this.outputFields,
    required this.tool,
    required this.tokens,
    required this.onRowAction,
    required this.onResultAction,
    required this.onReadRowNavigate,
    super.key,
  });

  final CanonicalToolResult outcome;
  final List<OutputFieldDto> outputFields;
  final ToolDto tool;
  final UpegTokens tokens;
  final void Function(PresentationActionDto action, PresentationTableRow row)
  onRowAction;
  final void Function(PresentationActionDto action) onResultAction;
  final void Function(PresentationActionDto action, PresentationTableRow row)
  onReadRowNavigate;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final headerColor = outcome.ok ? tokens.accent : tokens.warn;
    final headerText = outcome.ok
        ? t(ref, 'modal.outcome.ok')
        : t(ref, 'modal.outcome.error');
    final ClipboardWriter writer = ref.watch(clipboardWriterProvider);
    final outputRows = outcome.displayRows(outputFields);
    final resolvedView = tool.presentation == null || !outcome.ok
        ? null
        : ref.watch(presentationViewResolverProvider)(
            toolId: tool.id,
            outputsJson: jsonEncode(outcome.jsonValues),
          );
    final richView = resolvedView == null
        ? null
        : richPresentationViewFromDto(
            resolvedView,
            rawJson: outcome.canonicalJsonText(),
          );
    final hasRichView =
        resolvedView?.diagnostics.isEmpty == true &&
        (richView?.hasContent ?? false);
    return Container(
      key: const Key('expanded-modal-outcome'),
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: tokens.bg2,
        border: Border.all(color: tokens.line),
        borderRadius: BorderRadius.circular(UpegSizing.radius2),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Text(
                t(ref, 'modal.tag.output'),
                style: TextStyle(
                  fontFamily: upegMonoFontFamily,
                  fontFamilyFallback: upegMonoFontFamilyFallback,
                  fontSize: 10,
                  letterSpacing: 0.4,
                  color: tokens.fg3,
                ),
              ),
              const Spacer(),
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
                decoration: BoxDecoration(
                  color: headerColor,
                  borderRadius: BorderRadius.circular(UpegSizing.radius1),
                ),
                child: Text(
                  headerText,
                  style: TextStyle(
                    fontFamily: upegMonoFontFamily,
                    fontFamilyFallback: upegMonoFontFamilyFallback,
                    fontSize: 10,
                    color: outcome.ok ? tokens.onAccent : tokens.onWarn,
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ),
            ],
          ),
          const SizedBox(height: 10),
          if (outcome.errorMessage != null && outcome.errorMessage!.isNotEmpty)
            _OutputBlock(
              label: t(ref, 'modal.outcome.error'),
              value: outcome.errorMessage!,
              tokens: tokens,
              writer: writer,
            ),
          if (outcome.errorDetailsText case final details?)
            ErrorDetailsBlock(details: details, tokens: tokens),
          if (resolvedView != null && resolvedView.diagnostics.isNotEmpty)
            for (final diagnostic in resolvedView.diagnostics)
              Padding(
                padding: const EdgeInsets.only(bottom: 6),
                child: Text(
                  diagnostic,
                  style: TextStyle(color: tokens.warn, fontSize: 11),
                ),
              ),
          if (hasRichView)
            RichPresentationPanel(
              view: richView!,
              tokens: tokens,
              showRaw: false,
            )
          else
            for (final row in outputRows)
              if (row.field case final field?)
                StructuredOutputBlock(
                  key: ValueKey('expanded-modal-output-${row.id}'),
                  field: field,
                  entry: row.entry,
                  primary: row.isPrimary,
                  tokens: tokens,
                  writer: writer,
                )
              else
                _OutputBlock(
                  key: ValueKey('expanded-modal-output-${row.id}'),
                  label: row.label,
                  value: row.value,
                  primary: row.isPrimary,
                  tokens: tokens,
                  writer: writer,
                ),
          PresentationCollection(
            tool: tool,
            outcome: outcome,
            tokens: tokens,
            onRowAction: onRowAction,
            onResultAction: onResultAction,
            onReadRowNavigate: onReadRowNavigate,
          ),
          if (hasRichView && richView!.rawJson != null)
            RichPresentationRaw(
              raw: richView.rawJson!,
              tokens: tokens,
              label: t(ref, 'modal.presentation.raw_json'),
            ),
          if (!outcome.hasDisplayContent && !hasRichView)
            Text(
              t(ref, 'modal.no_output'),
              style: TextStyle(
                color: tokens.fg4,
                fontStyle: FontStyle.italic,
                fontSize: 11,
              ),
            ),
        ],
      ),
    );
  }
}

RichPresentationView richPresentationViewFromDto(
  PresentationViewDto dto, {
  required String rawJson,
}) => RichPresentationView(
  title: dto.title,
  subtitle: dto.subtitle,
  status: dto.status == null
      ? null
      : PresentationStatus(
          label: dto.status!.label,
          tone: _tone(dto.status!.tone),
        ),
  summary: dto.summary
      .map(
        (field) =>
            PresentationTextValue(label: field.label, value: field.value),
      )
      .toList(growable: false),
  notices: dto.notices
      .map(
        (notice) =>
            PresentationNotice(text: notice.text, tone: _tone(notice.severity)),
      )
      .toList(growable: false),
  detail: dto.detail == null ? null : _detail(dto.detail!),
  // A rich metadata declaration is the explicit signal that raw output may
  // move behind disclosure. Resolver diagnostics keep the legacy result open.
  rawJson: dto.diagnostics.isEmpty ? rawJson : null,
);

PresentationDetail _detail(PresentationDetailDto dto) => PresentationDetail(
  fields: dto.fields
      .map(
        (field) =>
            PresentationTextValue(label: field.label, value: field.value),
      )
      .toList(growable: false),
  markdown: dto.markdown,
  diff: dto.diff,
);

PresentationTone _tone(String value) => switch (value) {
  'success' => PresentationTone.success,
  'warning' => PresentationTone.warning,
  'error' => PresentationTone.error,
  _ => PresentationTone.neutral,
};

class _OutputBlock extends StatelessWidget {
  const _OutputBlock({
    required this.label,
    required this.value,
    required this.tokens,
    required this.writer,
    this.primary = false,
    super.key,
  });

  final String label;
  final String value;
  final UpegTokens tokens;
  final ClipboardWriter writer;
  final bool primary;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              _OutputLabel(label: label, primary: primary, tokens: tokens),
              const Spacer(),
              CopyToClipboardButton(textToCopy: value, writer: writer),
            ],
          ),
          const SizedBox(height: 2),
          SelectableText(
            value,
            style: TextStyle(
              fontFamily: upegMonoFontFamily,
              fontFamilyFallback: upegMonoFontFamilyFallback,
              fontSize: 11,
              color: tokens.fg,
            ),
          ),
        ],
      ),
    );
  }
}

class _OutputLabel extends ConsumerWidget {
  const _OutputLabel({
    required this.label,
    required this.primary,
    required this.tokens,
  });

  final String label;
  final bool primary;
  final UpegTokens tokens;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(
          label,
          style: TextStyle(
            fontFamily: upegMonoFontFamily,
            fontFamilyFallback: upegMonoFontFamilyFallback,
            fontSize: 10,
            color: tokens.fg4,
          ),
        ),
        if (primary) ...[
          const SizedBox(width: 6),
          Text(
            t(ref, 'modal.output.primary'),
            style: TextStyle(
              fontFamily: upegMonoFontFamily,
              fontFamilyFallback: upegMonoFontFamilyFallback,
              fontSize: 9,
              color: tokens.accent,
            ),
          ),
        ],
      ],
    );
  }
}
