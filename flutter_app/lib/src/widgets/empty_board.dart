/// Empty-board onboarding card.
///
/// A dashed-border card with an icon, a hint line that calls out the
/// `+ pin` / `⌘K` entry points, a CTA button, and up to three suggested
/// tools that pin on tap. Suggestions come from the FRB
/// `suggestionsForEmptyBoard` helper.
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/pegboard_mutations_provider.dart';
import 'package:upeg/src/state/suggestions_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';

/// Catalog keys — the En/Ko copy lives in the Rust catalog
/// (upeg-pegboard-ui/src/i18n.rs) and renders through `t()`.
const String emptyBoardTitleKey = 'empty.pegboard.title';
const String emptyBoardCtaLabelKey = 'empty.find_tool_button';
const String emptyBoardHintTextKey = 'empty.pegboard.hint';
const String emptyBoardSuggestionsLabelKey = 'empty.suggestion_header';

class EmptyBoard extends ConsumerWidget {
  const EmptyBoard({required this.onOpenPalette, this.boardKey, super.key});

  final VoidCallback onOpenPalette;

  /// Active board key — drives the suggestion list + pin actions.
  /// `null` is allowed so old call sites (tests) keep compiling; the
  /// suggestion list is simply omitted when the key isn't known.
  final BoardKey? boardKey;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = context.upeg;
    final key = boardKey;
    final List<ToolDto> suggestions = key == null
        ? const <ToolDto>[]
        : ref.watch(emptyBoardSuggestionsProvider(key));
    return Center(
      key: const Key('empty-board-card'),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 480),
        child: Container(
          padding: const EdgeInsets.fromLTRB(22, 18, 22, 18),
          decoration: BoxDecoration(
            border: Border.all(color: tokens.line, style: BorderStyle.solid),
            borderRadius: BorderRadius.circular(8),
            // Faux-dashed via lighter line. Flutter doesn't ship dashed
            // borders OOTB; the lighter solid line reads visually close
            // to a 1px dashed border.
            color: tokens.surface,
          ),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  const Text('🧰', style: TextStyle(fontSize: 18)),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Text(
                      t(ref, emptyBoardTitleKey),
                      style: TextStyle(
                        fontFamily: upegMonoFontFamily,
                        fontFamilyFallback: upegMonoFontFamilyFallback,
                        fontSize: 13,
                        fontWeight: FontWeight.w500,
                        color: tokens.fg,
                      ),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 10),
              Text(
                t(ref, emptyBoardHintTextKey),
                style: TextStyle(
                  fontFamily: upegMonoFontFamily,
                  fontFamilyFallback: upegMonoFontFamilyFallback,
                  fontSize: 12,
                  color: tokens.fg3,
                  height: 1.55,
                ),
              ),
              const SizedBox(height: 14),
              Align(
                alignment: Alignment.centerLeft,
                child: InkWell(
                  onTap: onOpenPalette,
                  borderRadius: BorderRadius.circular(UpegSizing.radius1),
                  child: Container(
                    key: const Key('empty-board-cta'),
                    padding: const EdgeInsets.symmetric(
                      horizontal: 10,
                      vertical: 6,
                    ),
                    decoration: BoxDecoration(
                      border: Border.all(color: tokens.line),
                      borderRadius: BorderRadius.circular(UpegSizing.radius1),
                    ),
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Icon(Icons.search, color: tokens.fg2, size: 12),
                        const SizedBox(width: 6),
                        Text(
                          t(ref, emptyBoardCtaLabelKey),
                          style: TextStyle(
                            fontFamily: upegMonoFontFamily,
                            fontFamilyFallback: upegMonoFontFamilyFallback,
                            fontSize: 11,
                            color: tokens.fg2,
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
              if (key != null && suggestions.isNotEmpty) ...[
                const SizedBox(height: 18),
                Text(
                  t(ref, emptyBoardSuggestionsLabelKey).toUpperCase(),
                  style: TextStyle(
                    fontFamily: upegMonoFontFamily,
                    fontFamilyFallback: upegMonoFontFamilyFallback,
                    fontSize: 10,
                    letterSpacing: 0.6,
                    color: tokens.fg3,
                  ),
                ),
                const SizedBox(height: 8),
                for (final tool in suggestions)
                  Padding(
                    padding: const EdgeInsets.only(bottom: 6),
                    child: _SuggestionTile(
                      key: Key('empty-board-suggestion-${tool.id}'),
                      tool: tool,
                      onTap: () => unawaited(
                        ref
                            .read(pegboardMutationsProvider)
                            .pin(key, ToolId.parse(tool.id)),
                      ),
                    ),
                  ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

class _SuggestionTile extends ConsumerWidget {
  const _SuggestionTile({required this.tool, required this.onTap, super.key});

  final ToolDto tool;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = context.upeg;
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(UpegSizing.radius1),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
        decoration: BoxDecoration(
          border: Border.all(color: tokens.lineSoft),
          borderRadius: BorderRadius.circular(UpegSizing.radius1),
        ),
        child: Row(
          children: [
            Container(
              width: 12,
              height: 12,
              decoration: BoxDecoration(
                color: tokens.bg2,
                border: Border.all(color: tokens.line),
                borderRadius: BorderRadius.circular(2),
              ),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(
                    tool.label,
                    style: TextStyle(
                      fontFamily: upegMonoFontFamily,
                      fontFamilyFallback: upegMonoFontFamilyFallback,
                      fontSize: 12,
                      fontWeight: FontWeight.w500,
                      color: tokens.fg,
                    ),
                  ),
                  Text(
                    tool.id,
                    style: TextStyle(
                      fontFamily: upegMonoFontFamily,
                      fontFamilyFallback: upegMonoFontFamilyFallback,
                      fontSize: 10,
                      color: tokens.fg4,
                    ),
                  ),
                ],
              ),
            ),
            const SizedBox(width: 8),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
              decoration: BoxDecoration(
                border: Border.all(color: tokens.accent),
                borderRadius: BorderRadius.circular(UpegSizing.radius1),
              ),
              child: Text(
                t(ref, 'palette.row.pin'),
                style: TextStyle(
                  fontFamily: upegMonoFontFamily,
                  fontFamilyFallback: upegMonoFontFamilyFallback,
                  fontSize: 10,
                  color: tokens.accent,
                  height: 1.0,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
