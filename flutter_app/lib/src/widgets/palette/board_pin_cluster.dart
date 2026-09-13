/// Per-board chip cluster surfaced inside each palette row.
///
/// Extracted from `palette_overlay.dart` (Batch Y) so the cluster is
/// independently testable and the palette overlay file stays focused
/// on the modal scaffold + result rows.
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart' show BoardDto;
import 'package:upeg/src/state/pegboard_mutations_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';

/// Renders one chip per registered board. A board chip is **filled**
/// (accent background) when [`toolId`] is already pinned there and
/// **outlined** otherwise. Tapping an outlined chip pins the tool to
/// that board; tapping a filled chip jumps to that board's pin via
/// [onJump] (board switch + pin focus, no run) — the palette ↔ board
/// integration. Chips are plain [InkWell]s, so they stay reachable by
/// Tab traversal and activate on Enter/Space.
class BoardPinChipCluster extends ConsumerWidget {
  const BoardPinChipCluster({
    required this.tokens,
    required this.toolId,
    required this.boards,
    required this.pinnedBoards,
    required this.currentBoardKey,
    required this.onJump,
    super.key,
  });

  final UpegTokens tokens;
  final ToolId toolId;
  final List<BoardDto> boards;
  final Set<BoardKey> pinnedBoards;
  final BoardKey? currentBoardKey;

  /// Called with the chip's board key when a **pinned** chip is
  /// activated — the host reveals the pin and closes the palette.
  final ValueChanged<BoardKey> onJump;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Wrap(
      spacing: 4,
      runSpacing: 4,
      alignment: WrapAlignment.end,
      children: [
        for (final board in boards)
          if (BoardKey.tryParse(board.key) case final boardKey?)
            BoardPinChip(
              key: Key('palette-board-chip-${toolId.value}-${board.key}'),
              tokens: tokens,
              label: board.title,
              pinned: pinnedBoards.contains(boardKey),
              isCurrent: boardKey == currentBoardKey,
              onTap: () {
                if (pinnedBoards.contains(boardKey)) {
                  onJump(boardKey);
                } else {
                  unawaited(
                    ref.read(pegboardMutationsProvider).pin(boardKey, toolId),
                  );
                }
              },
            ),
      ],
    );
  }
}

/// Single per-board chip. Filled when [pinned] is true; outlined
/// otherwise. The [isCurrent] flag bumps the border opacity so the
/// user can still tell which board is active.
class BoardPinChip extends StatelessWidget {
  const BoardPinChip({
    required this.tokens,
    required this.label,
    required this.pinned,
    required this.isCurrent,
    required this.onTap,
    super.key,
  });

  final UpegTokens tokens;
  final String label;
  final bool pinned;
  final bool isCurrent;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final borderColor = pinned
        ? tokens.accent
        : (isCurrent ? tokens.fg3 : tokens.line);
    final bgColor = pinned ? tokens.accent : Colors.transparent;
    final fgColor = pinned
        ? tokens.onAccent
        : (isCurrent ? tokens.fg : tokens.fg2);
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(UpegSizing.radius1),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
        decoration: BoxDecoration(
          color: bgColor,
          border: Border.all(color: borderColor),
          borderRadius: BorderRadius.circular(UpegSizing.radius1),
        ),
        child: Text(
          label,
          style: TextStyle(
            fontFamily: upegMonoFontFamily,
            fontFamilyFallback: upegMonoFontFamilyFallback,
            fontSize: 10,
            color: fgColor,
            height: 1.0,
            fontWeight: pinned ? FontWeight.w600 : FontWeight.w500,
          ),
        ),
      ),
    );
  }
}
