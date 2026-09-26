/// Bottom status bar — the fixed strip pinned to the bottom of the
/// board surface.
///
/// Shows:
///   * a network-reachability dot + label (color + text driven by the
///     sealed `NetworkReachabilityDto` enum — no string discriminator),
///   * the current board name + `X/Y pinned` summary,
///   * a paused chip when the host is paused,
///   * an imports chip: `imports loading…` while this process's MCP
///     import load is still running, else `imports N` once tools are
///     registered,
///   * the build version label.
///
/// Reads `currentBoardKeyProvider` + `boardsProvider` for board info
/// and `statusSnapshotProvider` for everything else — drops in
/// anywhere a row of status icons makes sense.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/pause.dart';
// `show`-restricted: `StatusSnapshotDto` also comes from
// `status_provider.dart` (as a typedef), and importing both
// wholesale makes the name ambiguous the moment it is written out.
import 'package:upeg/src/rust/api/status.dart'
    show McpImportPhaseDto, NetworkReachabilityDto;
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/status_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';

class StatusBar extends ConsumerWidget {
  const StatusBar({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = context.upeg;
    final boards = ref.watch(boardsProvider);
    final selectedKey = ref.watch(currentBoardKeyProvider);
    final status = ref.watch(statusSnapshotProvider);
    final layoutAsync = selectedKey == null
        ? const AsyncValue<LayoutSnapshotDto>.loading()
        : ref.watch(layoutProvider(LayoutQuery.all(selectedKey)));
    String boardTitle = '—';
    if (boards.value != null && selectedKey != null) {
      for (final b in boards.value!) {
        if (b.key == selectedKey.value) {
          boardTitle = b.title;
          break;
        }
      }
    }
    final pinnedCount = layoutAsync.value?.placements.length ?? 0;
    final importsLabel = _importsLabel(ref, status);

    return Container(
      height: UpegSizing.statusBarHeight,
      padding: const EdgeInsets.symmetric(horizontal: 12),
      decoration: BoxDecoration(
        color: tokens.bg2,
        border: Border(top: BorderSide(color: tokens.line)),
      ),
      child: Row(
        children: [
          Container(
            width: 6,
            height: 6,
            decoration: BoxDecoration(
              color: _dotColor(tokens, status.network.reachability),
              shape: BoxShape.circle,
            ),
          ),
          const SizedBox(width: 5),
          Text(status.network.label, style: _baseTextStyle(tokens)),
          const SizedBox(width: 14),
          Text(
            t(ref, 'desktop.status.board_prefix'),
            style: _baseTextStyle(tokens),
          ),
          Text(
            boardTitle,
            style: _baseTextStyle(tokens).copyWith(color: tokens.fg3),
          ),
          Text(
            t(ref, 'desktop.status.pinned_count', {'count': '$pinnedCount'}),
            style: _baseTextStyle(tokens),
          ),
          if (status.paused == PausedStateDto.paused) ...<Widget>[
            const SizedBox(width: 14),
            Text(
              t(ref, 'desktop.status.paused'),
              style: _baseTextStyle(tokens),
            ),
          ],
          if (importsLabel != null) ...<Widget>[
            const SizedBox(width: 14),
            Text(importsLabel, style: _baseTextStyle(tokens)),
          ],
          Expanded(child: Container()),
          Text('v${status.buildVersion}', style: _baseTextStyle(tokens)),
        ],
      ),
    );
  }

  /// Imports chip text, or `null` when the chip stays hidden.
  ///
  /// Total function over the sealed phase enum, for the same reason
  /// [`_dotColor`] is: a new `McpImportPhaseDto` variant in `upeg-frb`
  /// must be a compile error here, not a chip that silently vanishes.
  ///
  /// `loading` wins over the count because during the embedded host's
  /// async import window the count is not "0 imports" — it is "not in
  /// yet" (`upeg_cli::infrastructure::mcp_imports` module docs).
  String? _importsLabel(WidgetRef ref, StatusSnapshotDto status) {
    switch (status.mcpImportPhase) {
      case McpImportPhaseDto.loading:
        return t(ref, 'desktop.status.imports_loading');
      case McpImportPhaseDto.notStarted:
      case McpImportPhaseDto.done:
      case McpImportPhaseDto.skipped:
        return status.mcpImportCount > 0
            ? t(ref, 'desktop.status.imports', {
                'count': '${status.mcpImportCount}',
              })
            : null;
    }
  }

  /// Reachability dot color is a total function over the sealed enum.
  /// Adding a new variant in `upeg-frb` is a compile error here until
  /// the switch arm is added — the status bar can never silently
  /// render an unknown reachability.
  Color _dotColor(UpegTokens tokens, NetworkReachabilityDto reachability) {
    switch (reachability) {
      case NetworkReachabilityDto.loopbackOnly:
        return tokens.accent;
      case NetworkReachabilityDto.remoteBindAllowed:
        return tokens.warn;
      case NetworkReachabilityDto.offline:
        // fg3, not fg4: the reachability dot is always-visible status
        // information — it must clear UpegTokens.minNonTextContrast
        // against bg2 (fg4 sits at ~2.4:1).
        return tokens.fg3;
    }
  }

  /// fg3, not fg4: every string in this bar (network label, board
  /// name, paused chip, import count, version) is always-visible
  /// status information. fg4 lands at ~2.4:1 against bg2 — below even
  /// the WCAG large-text/non-text 3:1 floor — while fg3 clears it in
  /// both themes (pinned by the upeg_theme contrast tests).
  TextStyle _baseTextStyle(UpegTokens tokens) => TextStyle(
    fontFamily: upegMonoFontFamily,
    fontFamilyFallback: upegMonoFontFamilyFallback,
    fontSize: 10,
    color: tokens.fg3,
    height: 1.0,
  );
}
