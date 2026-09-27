/// Fixed-slot rendering for a resolved rich tool presentation.
/// Native code resolves pointers and action availability; this widget only
/// arranges already-safe display values, so it remains toolkit agnostic.
library;

import 'package:flutter/material.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/expanded_modal/structured_output.dart';

enum PresentationTone { neutral, success, warning, error }

@immutable
final class PresentationStatus {
  const PresentationStatus({required this.label, required this.tone});
  final String label;
  final PresentationTone tone;
}

@immutable
final class PresentationTextValue {
  const PresentationTextValue({required this.label, required this.value});
  final String label;
  final String value;
}

@immutable
final class PresentationNotice {
  const PresentationNotice({required this.text, required this.tone});
  final String text;
  final PresentationTone tone;
}

@immutable
final class PresentationDetail {
  const PresentationDetail({this.fields = const [], this.markdown, this.diff});
  final List<PresentationTextValue> fields;
  final String? markdown;
  final String? diff;
  bool get isEmpty =>
      fields.isEmpty &&
      (markdown == null || markdown!.isEmpty) &&
      (diff == null || diff!.isEmpty);
}

@immutable
final class RichPresentationView {
  const RichPresentationView({
    this.title,
    this.subtitle,
    this.status,
    this.summary = const [],
    this.notices = const [],
    this.detail,
    this.rawJson,
  });
  final String? title;
  final String? subtitle;
  final PresentationStatus? status;
  final List<PresentationTextValue> summary;
  final List<PresentationNotice> notices;
  final PresentationDetail? detail;

  /// Present only when native rich metadata resolved successfully.
  final String? rawJson;
  bool get hasContent =>
      title != null ||
      subtitle != null ||
      status != null ||
      summary.isNotEmpty ||
      notices.isNotEmpty ||
      !(detail?.isEmpty ?? true);
}

class RichPresentationPanel extends StatelessWidget {
  const RichPresentationPanel({
    required this.view,
    required this.tokens,
    this.showRaw = true,
    this.rawLabel = 'Raw JSON',
    super.key,
  });
  final RichPresentationView view;
  final UpegTokens tokens;
  final bool showRaw;
  final String rawLabel;

  @override
  Widget build(BuildContext context) {
    if (!view.hasContent) return const SizedBox.shrink();
    final detail = view.detail;
    return Column(
      key: const Key('rich-presentation'),
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (view.title != null || view.status != null)
          Row(
            children: [
              if (view.title != null)
                Expanded(
                  child: Text(
                    view.title!,
                    style: TextStyle(
                      color: tokens.fg,
                      fontSize: 16,
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                ),
              if (view.status case final status?)
                _ToneBadge(status: status, tokens: tokens),
            ],
          ),
        if (view.subtitle case final subtitle?)
          Padding(
            padding: const EdgeInsets.only(top: 3),
            child: Text(
              subtitle,
              style: TextStyle(color: tokens.fg3, fontSize: 11),
            ),
          ),
        if (view.summary.isNotEmpty) ...[
          const SizedBox(height: 10),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: [
              for (final item in view.summary)
                _SummaryCard(item: item, tokens: tokens),
            ],
          ),
        ],
        if (view.notices.isNotEmpty) ...[
          const SizedBox(height: 10),
          for (final notice in view.notices)
            Padding(
              padding: const EdgeInsets.only(bottom: 6),
              child: _Notice(notice: notice, tokens: tokens),
            ),
        ],
        if (detail != null && !detail.isEmpty) ...[
          const SizedBox(height: 12),
          _Detail(detail: detail, tokens: tokens),
        ],
        if (showRaw && view.rawJson != null)
          RichPresentationRaw(
            raw: view.rawJson!,
            tokens: tokens,
            label: rawLabel,
          ),
      ],
    );
  }
}

class RichPresentationRaw extends StatelessWidget {
  const RichPresentationRaw({
    required this.raw,
    required this.tokens,
    this.label = 'Raw JSON',
    super.key,
  });
  final String raw;
  final UpegTokens tokens;
  final String label;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(top: 8),
    child: Material(
      type: MaterialType.transparency,
      child: ExpansionTile(
        key: const Key('rich-presentation-raw'),
        tilePadding: EdgeInsets.zero,
        title: Text(label, style: TextStyle(color: tokens.fg3, fontSize: 11)),
        children: [
          SelectableText(
            raw,
            style: TextStyle(
              fontFamily: upegMonoFontFamily,
              fontFamilyFallback: upegMonoFontFamilyFallback,
              fontSize: 10,
              color: tokens.fg2,
            ),
          ),
        ],
      ),
    ),
  );
}

class _SummaryCard extends StatelessWidget {
  const _SummaryCard({required this.item, required this.tokens});
  final PresentationTextValue item;
  final UpegTokens tokens;
  @override
  Widget build(BuildContext context) => Container(
    constraints: const BoxConstraints(minWidth: 112),
    padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
    decoration: BoxDecoration(
      color: tokens.bg2,
      border: Border.all(color: tokens.line),
      borderRadius: BorderRadius.circular(UpegSizing.radius1),
    ),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(item.label, style: TextStyle(color: tokens.fg4, fontSize: 10)),
        const SizedBox(height: 2),
        Text(
          item.value,
          style: TextStyle(
            color: tokens.fg,
            fontSize: 13,
            fontWeight: FontWeight.w600,
          ),
        ),
      ],
    ),
  );
}

class _ToneBadge extends StatelessWidget {
  const _ToneBadge({required this.status, required this.tokens});
  final PresentationStatus status;
  final UpegTokens tokens;
  @override
  Widget build(BuildContext context) {
    final color = switch (status.tone) {
      PresentationTone.success => tokens.accent,
      PresentationTone.warning || PresentationTone.error => tokens.warn,
      PresentationTone.neutral => tokens.surface2,
    };
    return Container(
      key: const Key('rich-presentation-status'),
      padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 4),
      decoration: BoxDecoration(
        color: color,
        borderRadius: BorderRadius.circular(UpegSizing.radius1),
      ),
      child: Text(
        status.label,
        style: TextStyle(
          color: status.tone == PresentationTone.neutral
              ? tokens.fg2
              : UpegTokens.onFill(color),
          fontSize: 10,
          fontWeight: FontWeight.w600,
        ),
      ),
    );
  }
}

class _Notice extends StatelessWidget {
  const _Notice({required this.notice, required this.tokens});
  final PresentationNotice notice;
  final UpegTokens tokens;
  @override
  Widget build(BuildContext context) {
    final color = notice.tone == PresentationTone.neutral
        ? tokens.line
        : notice.tone == PresentationTone.success
        ? tokens.accent
        : tokens.warn;
    return Container(
      key: const Key('rich-presentation-notice'),
      padding: const EdgeInsets.all(8),
      decoration: BoxDecoration(
        border: Border(left: BorderSide(color: color, width: 3)),
        color: tokens.bg2,
      ),
      child: Text(
        notice.text,
        style: TextStyle(color: tokens.fg2, fontSize: 11),
      ),
    );
  }
}

class _Detail extends StatelessWidget {
  const _Detail({required this.detail, required this.tokens});
  final PresentationDetail detail;
  final UpegTokens tokens;
  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      for (final field in detail.fields)
        Padding(
          padding: const EdgeInsets.only(bottom: 5),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              SizedBox(
                width: 112,
                child: Text(
                  field.label,
                  style: TextStyle(color: tokens.fg4, fontSize: 10),
                ),
              ),
              Expanded(
                child: SelectableText(
                  field.value,
                  style: TextStyle(color: tokens.fg, fontSize: 11),
                ),
              ),
            ],
          ),
        ),
      if (detail.markdown case final markdown?)
        Padding(
          padding: const EdgeInsets.only(top: 4),
          child: MarkdownOutput(markdown: markdown, tokens: tokens),
        ),
      if (detail.diff case final diff?)
        Padding(
          padding: const EdgeInsets.only(top: 8),
          child: UnifiedDiffOutput(diff: diff, tokens: tokens),
        ),
    ],
  );
}

class UnifiedDiffOutput extends StatelessWidget {
  const UnifiedDiffOutput({
    required this.diff,
    required this.tokens,
    super.key,
  });
  final String diff;
  final UpegTokens tokens;
  @override
  Widget build(BuildContext context) => Container(
    key: const Key('rich-presentation-diff'),
    padding: const EdgeInsets.all(8),
    decoration: BoxDecoration(
      color: tokens.bg,
      border: Border.all(color: tokens.line),
      borderRadius: BorderRadius.circular(UpegSizing.radius1),
    ),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        for (final line in diff.split('\n'))
          Text(
            line,
            style: TextStyle(
              fontFamily: upegMonoFontFamily,
              fontFamilyFallback: upegMonoFontFamilyFallback,
              fontSize: 10,
              color: line.startsWith('+') && !line.startsWith('+++')
                  ? tokens.accent
                  : line.startsWith('-') && !line.startsWith('---')
                  ? tokens.warn
                  : line.startsWith('@@')
                  ? tokens.fg3
                  : tokens.fg2,
            ),
          ),
      ],
    ),
  );
}
