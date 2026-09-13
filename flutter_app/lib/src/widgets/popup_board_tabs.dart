/// Compact board-tab strip for the popup header.
///
/// Shows just the board titles,
/// horizontally scrollable, no `+ board` / `+ pin` / search / edit /
/// settings cluster (those live in the full Desktop tab bar). Tapping a
/// title writes [currentBoardKeyProvider], same contract as the desktop
/// [BoardTabs] widget.
///
/// Built as a dedicated widget — not a flag on [BoardTabs] — so the
/// popup never inherits the full-width buttons that would overflow the
/// 360px popup column.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/keyboard/filter_bar_navigation.dart';
import 'package:upeg/src/rust/api/pegboard.dart' show BoardDto;
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/theme/upeg_theme.dart';

/// Vertical padding on each popup board tab. Smaller than the desktop
/// strip — the popup row sits inside a 360px-wide column.
const double _kPopupTabVerticalPadding = 4;

/// Horizontal padding inside each tab — also driven by the compact
/// popup column width.
const double _kPopupTabHorizontalPadding = 8;

class PopupBoardTabs extends ConsumerWidget {
  const PopupBoardTabs({super.key});

  void _select(WidgetRef ref, BoardDto board) {
    ref
        .read(currentBoardKeyProvider.notifier)
        .select(BoardKey.parse(board.key));
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = context.upeg;
    final AsyncValue<List<BoardDto>> boards = ref.watch(boardsProvider);
    final selected = ref.watch(currentBoardKeyProvider);
    return boards.when(
      loading: () => const SizedBox(
        height: 22,
        width: 80,
        child: LinearProgressIndicator(minHeight: 2),
      ),
      error: (err, _) => Text(
        'boards: $err',
        style: TextStyle(color: tokens.warn, fontSize: 11),
      ),
      data: (list) => Focus(
        onKeyEvent: (_, event) => handleFilterBarKey<BoardDto>(
          ref,
          event,
          items: list,
          currentIndex: selected == null
              ? -1
              : list.indexWhere((board) => board.key == selected.value),
          onSelect: (board) => _select(ref, board),
        ),
        child: SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              for (final board in list)
                _PopupBoardTab(
                  key: ValueKey('popup-board-tab-${board.key}'),
                  board: board,
                  active: board.key == selected?.value,
                  onTap: () => _select(ref, board),
                ),
            ],
          ),
        ),
      ),
    );
  }
}

class _PopupBoardTab extends StatelessWidget {
  const _PopupBoardTab({
    required this.board,
    required this.active,
    required this.onTap,
    super.key,
  });

  final BoardDto board;
  final bool active;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    final color = active ? tokens.fg : tokens.fg3;
    final underline = active ? tokens.accent : Colors.transparent;
    return InkWell(
      onTap: onTap,
      child: Container(
        padding: const EdgeInsets.symmetric(
          horizontal: _kPopupTabHorizontalPadding,
          vertical: _kPopupTabVerticalPadding,
        ),
        decoration: BoxDecoration(
          border: Border(bottom: BorderSide(color: underline, width: 2)),
        ),
        child: Text(
          board.title,
          style: TextStyle(
            color: color,
            fontSize: 11,
            fontFamily: upegMonoFontFamily,
            fontFamilyFallback: upegMonoFontFamilyFallback,
          ),
        ),
      ),
    );
  }
}
