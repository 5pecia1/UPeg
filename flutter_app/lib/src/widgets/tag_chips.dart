/// Tag chip row between BoardTabs and BoardCanvas.
///
/// A 34px row that paints `TAGS` then one chip
/// per available tag. Tapping a chip swaps `selectedTagProvider`,
/// which the board canvas + palette pick up on the next rebuild.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/keyboard/filter_bar_navigation.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/tag_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';

/// Fixed height of the tag chip row (px).
const double kTagChipRowHeight = 34;

class TagChipRow extends ConsumerWidget {
  const TagChipRow({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = context.upeg;
    final boardKey = ref.watch(currentBoardKeyProvider);
    final tags = ref.watch(tagOptionsForBoardProvider(boardKey));
    final selected = ref.watch(selectedTagProvider);
    return Focus(
      onKeyEvent: (_, event) => handleFilterBarKey<TagSelection>(
        ref,
        event,
        items: tags,
        currentIndex: tags.indexOf(selected),
        onSelect: (tag) =>
            ref.read(selectedTagProvider.notifier).setSelection(tag),
      ),
      child: Container(
        key: const Key('tag-chip-row'),
        height: kTagChipRowHeight,
        padding: const EdgeInsets.symmetric(horizontal: 12),
        decoration: BoxDecoration(
          color: tokens.bg,
          border: Border(bottom: BorderSide(color: tokens.lineSoft)),
        ),
        child: Row(
          children: [
            Text(
              'TAGS',
              style: TextStyle(
                fontFamily: upegMonoFontFamily,
                fontFamilyFallback: upegMonoFontFamilyFallback,
                fontSize: 10,
                letterSpacing: 0.6,
                color: tokens.fg3,
              ),
            ),
            const SizedBox(width: 10),
            Expanded(
              child: SingleChildScrollView(
                scrollDirection: Axis.horizontal,
                child: Row(
                  children: [
                    for (final tag in tags) ...[
                      _TagChip(
                        key: Key('tag-chip-${tag.frbValue}'),
                        label:
                            '${tag.frbValue} ${ref.watch(countForBoardTagProvider((boardKey, tag)))}',
                        selected: tag == selected,
                        onTap: () => ref
                            .read(selectedTagProvider.notifier)
                            .setSelection(tag),
                      ),
                      const SizedBox(width: 6),
                    ],
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// Size of the check glyph marking the selected chip.
const double _selectedCheckIconSize = 10;

/// Test hook: the non-colour "selected" cue on the active tag chip.
const Key tagChipCheckIconKey = Key('tag-chip-check-icon');

class _TagChip extends StatelessWidget {
  const _TagChip({
    required this.label,
    required this.selected,
    required this.onTap,
    super.key,
  });

  final String label;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    final borderColor = selected ? tokens.accent : tokens.line;
    final fgColor = selected ? tokens.accent : tokens.fg2;
    // `selected:` mirrors the visual state into the semantics tree, and
    // the check icon is the non-colour cue — selection must not be
    // communicated by hue alone (WCAG 1.4.1).
    return Semantics(
      button: true,
      selected: selected,
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(UpegSizing.radius1),
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
          decoration: BoxDecoration(
            border: Border.all(color: borderColor),
            borderRadius: BorderRadius.circular(UpegSizing.radius1),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              if (selected) ...[
                Icon(
                  Icons.check,
                  key: tagChipCheckIconKey,
                  size: _selectedCheckIconSize,
                  color: fgColor,
                ),
                const SizedBox(width: 3),
              ],
              Text(
                label,
                style: TextStyle(
                  fontFamily: upegMonoFontFamily,
                  fontFamilyFallback: upegMonoFontFamilyFallback,
                  fontSize: 11,
                  color: fgColor,
                  height: 1.0,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
