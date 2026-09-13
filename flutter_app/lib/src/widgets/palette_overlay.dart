/// Cmd/Ctrl-K command palette.
///
/// Layout contract:
///   * Top-anchored modal card (520px wide, max 480 tall) with a
///     `›` chevron, a borderless input, and an `esc` kbd chip on the
///     right of the header,
///   * Result rows are compact: a 14x14 chip placeholder, the
///     display label, the dotted id in fg-4 monospace, a
///     kind-coloured pill, and a `+ pin` ghost button when not
///     already pinned,
///   * Footer with ↑↓ / ↵ / ⌘↵ kbd chips and a tool-count summary.
///
/// Plain Enter closes the overlay before forwarding the selected hit to
/// the caller; Cmd/Ctrl+Enter first pins the hit to the active board and
/// then forwards it through the same route-order-safe path.
///
/// Palette ↔ board integration: before closing, both commit paths (and
/// row taps) reveal the tool's pin — switch to the board chosen by
/// [resolvePaletteJumpBoard] and set `focusedPinProvider`, which
/// BoardCanvas answers with a scroll-into-view. Filled board chips are
/// jump buttons: they reveal the pin on their board without running.
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/keyboard/keyboard_command_resolver.dart';
import 'package:upeg/src/platform/keyboard_label.dart';
import 'package:upeg/src/rust/api/keyboard.dart';
import 'package:upeg/src/rust/api/palette.dart';
import 'package:upeg/src/rust/api/pegboard.dart' show BoardDto;
import 'package:upeg/src/rust/api/tools.dart' show ToolDto;
import 'package:upeg/src/widgets/expanded_modal/kind_badge.dart';
import 'package:upeg/src/widgets/palette/board_pin_cluster.dart';
import 'package:upeg/src/widgets/no_transition_dialog.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/focused_pin_provider.dart';
import 'package:upeg/src/state/palette_jump.dart';
import 'package:upeg/src/state/pegboard_mutations_provider.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/state/tag_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';

typedef PaletteHitCallback = void Function(PaletteHit hit);

/// Board-chip jump: reveal `toolId`'s pin on `boardKey` without running
/// the tool.
typedef PaletteJumpCallback = void Function(BoardKey boardKey, ToolId toolId);

const double _paletteFooterGroupGap = 12;
const double _paletteFooterRunGap = 6;
const double _paletteFooterKeyLabelGap = 4;

class PaletteOverlay extends ConsumerStatefulWidget {
  const PaletteOverlay({required this.onPick, super.key});

  final PaletteHitCallback onPick;

  @override
  ConsumerState<PaletteOverlay> createState() => _PaletteOverlayState();
}

class _PaletteOverlayState extends ConsumerState<PaletteOverlay> {
  late final TextEditingController _controller;
  final FocusNode _focusNode = FocusNode();
  final ScrollController _scrollController = ScrollController();
  int _highlighted = 0;

  @override
  void initState() {
    super.initState();
    final String initial = ref.read(paletteQueryProvider);
    _controller = TextEditingController(text: initial);
  }

  @override
  void dispose() {
    _controller.dispose();
    _focusNode.dispose();
    _scrollController.dispose();
    super.dispose();
  }

  void _move(int delta, int max) {
    if (max == 0) return;
    setState(() {
      _highlighted = (_highlighted + delta).clamp(0, max - 1);
    });
    // Scroll the highlighted item into view after the frame builds.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (_scrollController.hasClients) {
        final itemExtent = 48.0; // Approximate row height
        _scrollController.animateTo(
          _highlighted * itemExtent,
          duration: const Duration(milliseconds: 100),
          curve: Curves.easeOut,
        );
      }
    });
  }

  void _commit(List<PaletteHit> hits) {
    if (hits.isEmpty) return;
    final i = _highlighted.clamp(0, hits.length - 1);
    _pickHit(hits[i]);
  }

  /// Shared Enter / row-tap path: reveal the tool's pinned board (if
  /// any), close the palette, then forward the hit to the caller so the
  /// activation lands on the already-visible pin.
  void _pickHit(PaletteHit hit) {
    _revealPin(ToolId.parse(hit.id));
    Navigator.of(context).pop();
    widget.onPick(hit);
  }

  /// Cmd/Ctrl+Enter — open the selected hit AND pin it onto the
  /// active board in one shot. The meta/control modifier case is
  /// checked (and early-outs) before the plain-Enter case. The
  /// pin write is fire-and-forget; failures are logged by
  /// `pegboardMutationsProvider`.
  /// Inventory row H06.
  void _commitWithPin(List<PaletteHit> hits) {
    if (hits.isEmpty) return;
    final i = _highlighted.clamp(0, hits.length - 1);
    final hit = hits[i];
    final toolId = ToolId.parse(hit.id);
    final boardKey = ref.read(currentBoardKeyProvider);
    if (boardKey != null) {
      unawaited(ref.read(pegboardMutationsProvider).pin(boardKey, toolId));
    }
    // The pin write above is still in flight, so the pinned-boards
    // snapshot cannot include it yet — pass the board explicitly.
    _revealPin(toolId, justPinnedBoard: boardKey);
    Navigator.of(context).pop();
    widget.onPick(hit);
  }

  /// Palette ↔ board integration: switch to the board that hosts the
  /// tool's pin and hand keyboard focus to that pin, so BoardCanvas
  /// scrolls it into view (`focusedPinProvider` listener). No-op for
  /// tools without a pin anywhere — those keep the plain run behaviour
  /// (result feedback stays on the caller's snackbar path).
  void _revealPin(ToolId toolId, {BoardKey? justPinnedBoard}) {
    final boards = ref.read(boardsProvider).value ?? const <BoardDto>[];
    final target = resolvePaletteJumpBoard(
      currentBoardKey: ref.read(currentBoardKeyProvider),
      boardOrder: [for (final board in boards) ?BoardKey.tryParse(board.key)],
      pinnedBoards: {
        ...ref.read(pinnedBoardsForToolProvider(toolId)),
        ?justPinnedBoard,
      },
    );
    if (target == null) return;
    _jumpToBoardPin(target, toolId);
  }

  /// Switch to [boardKey] (no-op when already current) and focus the
  /// tool's pin.
  void _jumpToBoardPin(BoardKey boardKey, ToolId toolId) {
    if (ref.read(currentBoardKeyProvider) != boardKey) {
      ref.read(currentBoardKeyProvider.notifier).select(boardKey);
    }
    ref.read(focusedPinProvider.notifier).focus(toolId);
  }

  /// Board-chip jump (no run): reveal the pin, then just close the
  /// palette — `onPick` is intentionally not called.
  void _jumpFromChip(BoardKey boardKey, ToolId toolId) {
    _jumpToBoardPin(boardKey, toolId);
    Navigator.of(context).pop();
  }

  KeyEventResult _handlePaletteKey(KeyEvent event, List<PaletteHit> hits) {
    if (_isPrimaryCommitWithPin(event)) {
      _commitWithPin(hits);
      return KeyEventResult.handled;
    }
    final cmd = resolveKeyboardCommand(
      ref,
      event,
      scope: KeyboardScopeDto.toolPicker,
    );
    return switch (cmd) {
      KeyboardCommandDto_Move(:final direction) => _moveCommand(
        direction,
        hits.length,
      ),
      KeyboardCommandDto_Commit() => _commitCommand(hits),
      KeyboardCommandDto_Cancel() => _cancelCommand(),
      _ => KeyEventResult.ignored,
    };
  }

  bool _isPrimaryCommitWithPin(KeyEvent event) {
    return isPrimaryKeyboardShortcut(event, 'Enter');
  }

  KeyEventResult _moveCommand(DirectionDto direction, int hitCount) {
    return switch (direction) {
      DirectionDto.down => _moveBy(1, hitCount),
      DirectionDto.up => _moveBy(-1, hitCount),
      _ => KeyEventResult.ignored,
    };
  }

  KeyEventResult _moveBy(int delta, int hitCount) {
    _move(delta, hitCount);
    return KeyEventResult.handled;
  }

  KeyEventResult _commitCommand(List<PaletteHit> hits) {
    _commit(hits);
    return KeyEventResult.handled;
  }

  KeyEventResult _cancelCommand() {
    Navigator.of(context).pop();
    return KeyEventResult.handled;
  }

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    final AsyncValue<List<PaletteHit>> rawResults = ref.watch(
      paletteResultsProvider,
    );
    // G04: when the user has a [`TagSpecific`] filter active, intersect
    // the palette hits with the tag's tool id set. The intersection
    // is Dart-side so search_tools stays a single FRB call and the
    // filter behaviour stays testable without crossing the dylib.
    final TagSelection selectedTag = ref.watch(selectedTagProvider);
    final AsyncValue<List<PaletteHit>> results = switch (selectedTag) {
      TagAll() => rawResults,
      TagSpecific(:final tag) => rawResults.whenData((hits) {
        final allowed = ref
            .watch(toolsForTagProvider(tag))
            .map((t) => t.id)
            .toSet();
        return [
          for (final hit in hits)
            if (allowed.contains(hit.id)) hit,
        ];
      }),
    };
    // Search must expose every Desktop-searchable tool. The dialog height
    // stays bounded by the surrounding ConstrainedBox; the ListView scrolls
    // instead of dropping rows.
    final hits = results.value ?? const <PaletteHit>[];
    return Dialog(
      backgroundColor: Colors.transparent,
      alignment: Alignment.topCenter,
      insetPadding: const EdgeInsets.only(top: 96, left: 16, right: 16),
      elevation: 0,
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 520, maxHeight: 480),
        child: Container(
          decoration: BoxDecoration(
            color: tokens.surface,
            border: Border.all(color: tokens.line),
            borderRadius: BorderRadius.circular(8),
            boxShadow: const [
              BoxShadow(
                color: Color(0x66000000),
                blurRadius: 60,
                offset: Offset(0, 20),
              ),
            ],
          ),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              _PaletteHeader(
                tokens: tokens,
                controller: _controller,
                focusNode: _focusNode,
                onChanged: (value) {
                  ref.read(paletteQueryProvider.notifier).state = value;
                  setState(() => _highlighted = 0);
                },
                onKey: (event) => _handlePaletteKey(event, hits),
              ),
              Flexible(
                child: _PaletteBody(
                  tokens: tokens,
                  query: ref.watch(paletteQueryProvider),
                  results: results,
                  highlighted: _highlighted,
                  currentBoardKey: ref.watch(currentBoardKeyProvider),
                  scrollController: _scrollController,
                  onHover: (i) => setState(() => _highlighted = i),
                  onPick: _pickHit,
                  onJumpToBoard: _jumpFromChip,
                ),
              ),
              _PaletteFooter(tokens: tokens, count: hits.length),
            ],
          ),
        ),
      ),
    );
  }
}

class _PaletteHeader extends ConsumerWidget {
  const _PaletteHeader({
    required this.tokens,
    required this.controller,
    required this.focusNode,
    required this.onChanged,
    required this.onKey,
  });

  final UpegTokens tokens;
  final TextEditingController controller;
  final FocusNode focusNode;
  final ValueChanged<String> onChanged;
  final KeyEventResult Function(KeyEvent) onKey;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Container(
      padding: const EdgeInsets.fromLTRB(14, 12, 14, 12),
      decoration: BoxDecoration(
        border: Border(bottom: BorderSide(color: tokens.lineSoft)),
      ),
      child: Row(
        children: [
          Text(
            '›',
            style: TextStyle(
              color: tokens.fg3,
              fontSize: 11,
              fontFamily: upegMonoFontFamily,
              fontFamilyFallback: upegMonoFontFamilyFallback,
            ),
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Focus(
              onKeyEvent: (_, event) => onKey(event),
              child: TextField(
                key: const Key('palette-query-field'),
                controller: controller,
                focusNode: focusNode,
                autofocus: true,
                style: TextStyle(
                  fontFamily: upegMonoFontFamily,
                  fontFamilyFallback: upegMonoFontFamilyFallback,
                  fontSize: 14,
                  color: tokens.fg,
                ),
                cursorColor: tokens.accent,
                decoration: InputDecoration(
                  border: InputBorder.none,
                  isCollapsed: true,
                  hintText: t(ref, 'palette.placeholder'),
                  hintStyle: TextStyle(color: tokens.fg3, fontSize: 14),
                ),
                onChanged: onChanged,
              ),
            ),
          ),
          const SizedBox(width: 10),
          _Kbd(tokens: tokens, text: 'esc'),
        ],
      ),
    );
  }
}

class _PaletteBody extends ConsumerWidget {
  const _PaletteBody({
    required this.tokens,
    required this.query,
    required this.results,
    required this.highlighted,
    required this.currentBoardKey,
    required this.scrollController,
    required this.onHover,
    required this.onPick,
    required this.onJumpToBoard,
  });

  final UpegTokens tokens;
  final String query;
  final AsyncValue<List<PaletteHit>> results;
  final int highlighted;

  /// Active board key — drives the pinned-pill / `+ pin` button on
  /// each row. `null` while the BoardPage hasn't seeded the selection
  /// yet; the row hides the pin affordance in that case so we never
  /// dispatch a pin against an unknown board.
  final BoardKey? currentBoardKey;

  final ScrollController scrollController;

  final ValueChanged<int> onHover;
  final PaletteHitCallback onPick;
  final PaletteJumpCallback onJumpToBoard;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    // H07 — distinguish "empty toolbox" (no tools registered at all)
    // from an empty browse result. Empty query itself is not an empty
    // state anymore; Rust search returns browseable tools for it.
    final List<ToolDto>? toolbox = ref.watch(toolsProvider).value;
    final bool toolboxIsEmpty = toolbox != null && toolbox.isEmpty;
    if (toolboxIsEmpty) {
      return Padding(
        padding: const EdgeInsets.all(16),
        key: const Key('palette-empty-toolbox'),
        child: Text(
          t(ref, 'palette.empty_toolbox'),
          style: TextStyle(color: tokens.fg3, fontSize: 12),
        ),
      );
    }
    return results.when(
      loading: () => const Padding(
        padding: EdgeInsets.all(16),
        child: Center(child: CircularProgressIndicator()),
      ),
      error: (err, _) => Padding(
        padding: const EdgeInsets.all(16),
        child: Text(
          t(ref, 'common.search_failed', {'msg': '$err'}),
          style: TextStyle(color: tokens.warn),
        ),
      ),
      data: (rawHits) {
        // Render every match; the route's max height makes the list scroll
        // instead of hiding rows.
        final hits = rawHits;
        if (hits.isEmpty) {
          // Echo the trimmed query so users see what they typed. The
          // empty-toolbox copy is reserved for the (rare) case the user
          // hasn't typed anything yet — for an explicit search we
          // route through the localised `palette.no_match` template.
          final trimmed = query.trim();
          final hint = t(ref, 'palette.no_match', {'needle': trimmed});
          return Padding(
            padding: const EdgeInsets.all(16),
            key: const Key('palette-no-results'),
            child: Text(
              hint,
              style: TextStyle(color: tokens.fg3, fontSize: 12),
            ),
          );
        }
        return ListView.builder(
          controller: scrollController,
          shrinkWrap: true,
          padding: const EdgeInsets.all(6),
          itemCount: hits.length,
          itemBuilder: (context, i) {
            final hit = hits[i];
            final highlighted = i == this.highlighted;
            return _PaletteRow(
              key: ValueKey('palette-hit-${hit.id}'),
              tokens: tokens,
              hit: hit,
              highlighted: highlighted,
              currentBoardKey: currentBoardKey,
              onHover: () => onHover(i),
              onTap: () => onPick(hit),
              onJumpToBoard: onJumpToBoard,
            );
          },
        );
      },
    );
  }
}

class _PaletteRow extends ConsumerWidget {
  const _PaletteRow({
    required this.tokens,
    required this.hit,
    required this.highlighted,
    required this.currentBoardKey,
    required this.onHover,
    required this.onTap,
    required this.onJumpToBoard,
    super.key,
  });

  final UpegTokens tokens;
  final PaletteHit hit;
  final bool highlighted;
  final BoardKey? currentBoardKey;
  final VoidCallback onHover;
  final VoidCallback onTap;
  final PaletteJumpCallback onJumpToBoard;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final bg = highlighted ? tokens.surface2 : Colors.transparent;
    final boards = ref.watch(boardsProvider).value ?? const <BoardDto>[];
    final toolId = ToolId.parse(hit.id);
    final pinnedBoards = ref.watch(pinnedBoardsForToolProvider(toolId));
    return MouseRegion(
      onEnter: (_) => onHover(),
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(5),
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
          decoration: BoxDecoration(
            color: bg,
            borderRadius: BorderRadius.circular(5),
          ),
          child: Row(
            children: [
              Container(
                width: 14,
                height: 14,
                decoration: BoxDecoration(
                  color: tokens.bg2,
                  border: Border.all(color: tokens.line),
                  borderRadius: BorderRadius.circular(3),
                ),
              ),
              const SizedBox(width: 10),
              Flexible(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Row(
                      children: [
                        Flexible(
                          child: Text(
                            hit.label,
                            overflow: TextOverflow.ellipsis,
                            style: TextStyle(
                              fontFamily: upegMonoFontFamily,
                              fontFamilyFallback: upegMonoFontFamilyFallback,
                              fontSize: 12.5,
                              color: tokens.fg,
                              fontWeight: FontWeight.w500,
                            ),
                          ),
                        ),
                        if (hit.label != hit.id) ...[
                          const SizedBox(width: 8),
                          Flexible(
                            child: Text(
                              hit.id,
                              overflow: TextOverflow.ellipsis,
                              style: TextStyle(
                                fontFamily: upegMonoFontFamily,
                                fontFamilyFallback: upegMonoFontFamilyFallback,
                                fontSize: 10,
                                color: tokens.fg4,
                              ),
                            ),
                          ),
                        ],
                      ],
                    ),
                    if (hit.description.isNotEmpty)
                      Padding(
                        padding: const EdgeInsets.only(top: 2),
                        child: Text(
                          hit.description,
                          overflow: TextOverflow.ellipsis,
                          style: TextStyle(color: tokens.fg3, fontSize: 11),
                        ),
                      ),
                  ],
                ),
              ),
              const SizedBox(width: 10),
              // H11 — kind badge between label and per-board chip
              // cluster.
              KindBadge(
                key: Key('palette-kind-badge-${hit.id}'),
                pinKind: hit.pinKind,
              ),
              const SizedBox(width: 8),
              if (boards.isNotEmpty)
                BoardPinChipCluster(
                  key: Key('palette-board-chips-${hit.id}'),
                  tokens: tokens,
                  toolId: toolId,
                  boards: boards,
                  pinnedBoards: pinnedBoards,
                  currentBoardKey: currentBoardKey,
                  onJump: (boardKey) => onJumpToBoard(boardKey, toolId),
                ),
            ],
          ),
        ),
      ),
    );
  }
}

class _PaletteFooter extends ConsumerWidget {
  const _PaletteFooter({required this.tokens, required this.count});
  final UpegTokens tokens;

  /// Number of visible hits — drives the count summary (H09). Zero
  /// hides the summary so an empty-state palette doesn't shout
  /// "0 tools".
  final int count;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final platform = ref.watch(keyboardPlatformProvider);
    final boardCount = ref.watch(boardsProvider).value?.length ?? 0;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
      decoration: BoxDecoration(
        border: Border(top: BorderSide(color: tokens.lineSoft)),
      ),
      child: Wrap(
        spacing: _paletteFooterGroupGap,
        runSpacing: _paletteFooterRunGap,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          _FooterShortcut(
            tokens: tokens,
            keyLabel: '↑↓',
            label: t(ref, 'palette.footer.navigate'),
          ),
          _FooterShortcut(
            tokens: tokens,
            keyLabel: '↵',
            label: t(ref, 'palette.footer.open'),
          ),
          _FooterShortcut(
            tokens: tokens,
            keyLabel: shortcutLabel(platform, '↵'),
            label: t(ref, 'palette.footer.open_pin'),
          ),
          if (count > 0)
            Text(
              t(ref, 'palette.footer.summary', {
                'count': '$count',
                'total': '$boardCount',
              }),
              key: const Key('palette-count-summary'),
              style: _paletteFooterTextStyle(tokens),
            ),
        ],
      ),
    );
  }
}

class _FooterShortcut extends StatelessWidget {
  const _FooterShortcut({
    required this.tokens,
    required this.keyLabel,
    required this.label,
  });

  final UpegTokens tokens;
  final String keyLabel;
  final String label;

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        _Kbd(tokens: tokens, text: keyLabel),
        const SizedBox(width: _paletteFooterKeyLabelGap),
        Text(label, style: _paletteFooterTextStyle(tokens)),
      ],
    );
  }
}

TextStyle _paletteFooterTextStyle(UpegTokens tokens) => TextStyle(
  fontFamily: upegMonoFontFamily,
  fontFamilyFallback: upegMonoFontFamilyFallback,
  fontSize: 10,
  color: tokens.fg4,
);

class _Kbd extends StatelessWidget {
  const _Kbd({required this.tokens, required this.text});
  final UpegTokens tokens;
  final String text;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 2),
      decoration: BoxDecoration(
        color: tokens.surface2,
        border: Border.all(color: tokens.line),
        borderRadius: BorderRadius.circular(3),
      ),
      child: Text(
        text,
        style: TextStyle(
          fontFamily: upegMonoFontFamily,
          fontFamilyFallback: upegMonoFontFamilyFallback,
          fontSize: 11,
          color: tokens.fg2,
          height: 1.0,
        ),
      ),
    );
  }
}

/// Helper for [BoardPage] / shortcuts.
Future<void> showPaletteOverlay(
  BuildContext context, {
  required PaletteHitCallback onPick,
}) {
  return showNoTransitionDialog<void>(
    context: context,
    barrierColor: Colors.black54,
    builder: (_) => PaletteOverlay(onPick: onPick),
  );
}
