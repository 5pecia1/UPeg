/// Popup-mode quick launcher.
///
/// A 360px-wide compact view with a header tag, a search input, and
/// a mini-pegboard grid (2 columns × ~116px rows). Immediate-dispatch
/// tools run *inside* the popup and show their result inline in the
/// tapped cell (chrome-ext popup parity); only form/embed tools switch
/// [windowModeProvider] to the full BoardPage surface. The routing
/// policy lives in `popup/popup_activation_route.dart`; the dispatch
/// side effects in `popup/popup_activation_controller.dart`.
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart' show KeyDownEvent, LogicalKeyboardKey;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/keyboard/keyboard_command_resolver.dart';
import 'package:upeg/src/popup/popup_activation_controller.dart';
import 'package:upeg/src/popup/popup_hits_provider.dart';
import 'package:upeg/src/popup/popup_inline_outcome_provider.dart';
import 'package:upeg/src/popup/popup_selection_provider.dart';
import 'package:upeg/src/rust/api/keyboard.dart';
import 'package:upeg/src/rust/api/palette.dart';
import 'package:upeg/src/rust/api/tools.dart' show CanonicalToolResult;
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/status_provider.dart';
import 'package:upeg/src/state/window_mode_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';
import 'package:upeg/src/widgets/popup_auto_hide_observer.dart';
import 'package:upeg/src/widgets/popup_board_tabs.dart';

/// Catalog keys — En/Ko copy lives in the Rust catalog
/// (upeg-pegboard-ui/src/i18n.rs) and renders through `t()`.
const String popupEmptyHintKey = 'popup.search_placeholder';
const String popupNoResultsHintKey = 'popup.no_match';
const String popupHeaderLabelKey = 'popup.tag';
const String popupPinnedSectionKey = 'popup.pinned_section';
const String popupResultOkKey = 'popup.result.ok';
const String popupResultErrorKey = 'popup.result.error';
const String popupResultEmptyOutputKey = 'popup.result.empty_output';
const String popupResultCopyTooltipKey = 'popup.result.copy_tooltip';

/// Mini-pegboard grid column count: two equal-width columns.
const int popupGridCols = 2;

/// Fixed row height for popup hit cells (px).
const double popupCellHeight = 116;

/// Inter-cell gap (cross + main axis).
const double popupCellGap = 6;

/// Popup close hider seam. Production routes to `window_manager` through
/// [WindowManagerHider]; widget tests override this provider with a recording
/// hider so [PopupPage] never imports or calls platform channels directly.
final popupWindowHiderProvider = Provider<WindowHider>(
  (ref) => const WindowManagerHider(),
);

/// Test seam for explicit close (e.g. a future Close button).
///
/// A `ValueNotifier<bool>` that, when set to `true`, triggers the same
/// close path as `Esc` via the `_PopupPageState` listener. Tests fire
/// this to prove that explicit Close uses the identical code path.
/// Defaults to `false` (no-op) — the page wires the listener in State.
final popupCloseProvider =
    NotifierProvider<PopupCloseNotifier, ValueNotifier<bool>>(
      PopupCloseNotifier.new,
    );

class PopupCloseNotifier extends Notifier<ValueNotifier<bool>> {
  @override
  ValueNotifier<bool> build() {
    final notifier = ValueNotifier<bool>(false);
    notifier.addListener(() {
      if (notifier.value) {
        _doClose();
        notifier.value = false;
      }
    });
    return notifier;
  }

  VoidCallback? _closeCallback;

  /// Register the actual close implementation from the page state.
  void register(VoidCallback callback) => _closeCallback = callback;

  /// Internal dispatcher called by both Esc and explicit Close.
  void _doClose() => _closeCallback?.call();

  /// Public entry point for tests and future Close buttons.
  void handleClose() => _doClose();
}

class PopupPage extends ConsumerStatefulWidget {
  const PopupPage({super.key});

  @override
  ConsumerState<PopupPage> createState() => _PopupPageState();
}

class _PopupPageState extends ConsumerState<PopupPage> {
  late final TextEditingController _controller;
  final FocusNode _rootFocus = FocusNode(debugLabel: 'PopupPage');

  @override
  void initState() {
    super.initState();
    // Clear the shared query on each popup mount so the catalogue
    // shows the full registered tool list instead of carrying over
    // a stale search from a closed session. Deferred via microtask
    // to avoid Riverpod's "modify during build" assertion.
    Future.microtask(() {
      if (!mounted) return;
      ref.read(paletteQueryProvider.notifier).state = '';
    });
    _controller = TextEditingController(text: '');
    // Register close callback so popupCloseProvider can dispatch through
    // the same code path as Esc (explicit Close → same semantics).
    ref.read(popupCloseProvider.notifier).register(_handleClose);
    // Cold-start hydration: when the app launches directly in popup
    // mode, BoardPage.initState never runs, so restore the shared
    // pegboard selection here, reusing the same board-facing facade
    // BoardPage drains via `_restorePegboardSelection`.
    Future<void>.microtask(_restorePegboardSelection);
  }

  Future<void> _restorePegboardSelection() async {
    if (!mounted) return;
    await ref.read(currentBoardKeyProvider.notifier).restore();
  }

  @override
  void dispose() {
    _controller.dispose();
    _rootFocus.dispose();
    super.dispose();
  }

  /// Translate a key-down event into the popup's typed intent and run
  /// the side-effect. Returns `true` when the event was consumed so the
  /// Focus chain stops propagating.
  KeyEventResult _onKeyEvent(KeyEvent event) {
    if (primaryFocusIsEditableText() && isPlainTextEntryKeyEvent(event)) {
      return KeyEventResult.ignored;
    }
    // F2 — copy the latest inline result (mirrors the modal's [F2] copy
    // affordance). Handled before the Rust resolver: the copy shortcut
    // is popup-local chrome, not a board command.
    if (event is KeyDownEvent && event.logicalKey == LogicalKeyboardKey.f2) {
      final copied = ref
          .read(popupActivationControllerProvider)
          .copyLatestResult();
      return copied ? KeyEventResult.handled : KeyEventResult.ignored;
    }
    final cmd = resolveKeyboardCommand(
      ref,
      event,
      scope: KeyboardScopeDto.moving,
    );
    return switch (cmd) {
      KeyboardCommandDto_Commit() => _activateSelected(),
      KeyboardCommandDto_Move(:final direction) => _moveCursor(direction),
      KeyboardCommandDto_Cancel() => _handleClose(),
      KeyboardCommandDto_Close() => _handleClose(),
      _ => KeyEventResult.ignored,
    };
  }

  /// Context-aware close: instant, no animations or delays.
  ///
  /// - Full GUI mode → switch to popup immediately.
  /// - Popup mode → hide the window to tray.
  ///
  /// Always returns `KeyEventResult.handled` so the key event is
  /// consumed and never propagates further.
  KeyEventResult _handleClose() {
    final mode = ref.read(windowModeProvider);
    if (mode == WindowMode.full) {
      ref.read(windowModeProvider.notifier).set(WindowMode.popup);
    } else {
      unawaited(ref.read(popupWindowHiderProvider).hide());
    }
    return KeyEventResult.handled;
  }

  KeyEventResult _activateSelected() {
    final hits = ref.read(popupEffectiveHitsProvider);
    if (hits.isEmpty) return KeyEventResult.ignored;
    final selection = ref.read(popupSelectionProvider);
    final clamped = selection.index.clamp(0, hits.length - 1).toInt();
    final hit = hits[clamped];
    unawaited(
      ref
          .read(popupActivationControllerProvider)
          .activate(ToolId.parse(hit.id)),
    );
    return KeyEventResult.handled;
  }

  KeyEventResult _moveCursor(DirectionDto direction) {
    final hits = ref.read(popupEffectiveHitsProvider);
    if (hits.isEmpty) return KeyEventResult.ignored;
    final current = ref
        .read(popupSelectionProvider)
        .index
        .clamp(0, hits.length - 1)
        .toInt();
    final next = _popupIndexForDirection(current, hits.length, direction);
    ref.read(popupSelectionProvider.notifier).resetTo(next);
    return KeyEventResult.handled;
  }

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    final query = ref.watch(paletteQueryProvider);
    final results = ref.watch(paletteResultsProvider);
    final catalogueHits = ref.watch(popupCatalogueHitsProvider);
    return Scaffold(
      backgroundColor: tokens.bg,
      body: Focus(
        focusNode: _rootFocus,
        // Don't grab autofocus — the search TextField owns it, and the
        // Focus chain bubbles Enter/arrow keys up to this node when
        // the field ignores them.
        canRequestFocus: false,
        skipTraversal: true,
        onKeyEvent: (_, event) => _onKeyEvent(event),
        child: SafeArea(
          child: Container(
            width: 360,
            padding: const EdgeInsets.all(10),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Row(
                  children: [
                    Text(
                      t(ref, popupHeaderLabelKey).toUpperCase(),
                      style: TextStyle(
                        fontFamily: upegMonoFontFamily,
                        fontFamilyFallback: upegMonoFontFamilyFallback,
                        fontSize: 10,
                        letterSpacing: 0.4,
                        color: tokens.fg3,
                      ),
                    ),
                    const Spacer(),
                    InkWell(
                      key: const Key('popup-open-desktop'),
                      onTap: () => ref
                          .read(windowModeProvider.notifier)
                          .set(WindowMode.full),
                      child: Text(
                        t(ref, 'popup.open_desktop'),
                        style: TextStyle(
                          color: tokens.accent,
                          fontSize: 11,
                          fontFamily: upegMonoFontFamily,
                          fontFamilyFallback: upegMonoFontFamilyFallback,
                        ),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 6),
                const PopupBoardTabs(),
                const SizedBox(height: 8),
                Container(
                  decoration: BoxDecoration(
                    color: tokens.bg2,
                    border: Border.all(color: tokens.line),
                    borderRadius: BorderRadius.circular(UpegSizing.radius1),
                  ),
                  padding: const EdgeInsets.symmetric(
                    horizontal: 8,
                    vertical: 4,
                  ),
                  child: TextField(
                    key: const Key('popup-search-field'),
                    controller: _controller,
                    autofocus: true,
                    style: TextStyle(
                      fontFamily: upegMonoFontFamily,
                      fontFamilyFallback: upegMonoFontFamilyFallback,
                      color: tokens.fg,
                      fontSize: 12,
                    ),
                    decoration: InputDecoration(
                      border: InputBorder.none,
                      isCollapsed: true,
                      hintText: t(ref, popupEmptyHintKey),
                      hintStyle: TextStyle(color: tokens.fg3, fontSize: 12),
                    ),
                    onChanged: (value) {
                      ref.read(paletteQueryProvider.notifier).state = value;
                    },
                  ),
                ),
                const SizedBox(height: 8),
                ConstrainedBox(
                  constraints: const BoxConstraints(maxHeight: 360),
                  child: _PopupBody(
                    tokens: tokens,
                    query: query,
                    results: results,
                    catalogueHits: catalogueHits,
                  ),
                ),
                const SizedBox(height: 6),
                const _PopupFooter(),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// Compact footer that prints the build version.
/// Watches [statusSnapshotProvider] directly
/// so a version bump (Cargo rebuild) shows up without a popup reopen.
class _PopupFooter extends ConsumerWidget {
  const _PopupFooter();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = context.upeg;
    final version = ref.watch(
      statusSnapshotProvider.select((s) => s.buildVersion),
    );
    return Align(
      alignment: Alignment.centerRight,
      child: Text(
        'v$version',
        style: TextStyle(
          fontFamily: upegMonoFontFamily,
          fontFamilyFallback: upegMonoFontFamilyFallback,
          fontSize: 10,
          color: tokens.fg3,
        ),
      ),
    );
  }
}

class _PopupBody extends ConsumerWidget {
  const _PopupBody({
    required this.tokens,
    required this.query,
    required this.results,
    required this.catalogueHits,
  });

  final UpegTokens tokens;
  final String query;
  final AsyncValue<List<PaletteHit>> results;
  final AsyncValue<List<PaletteHit>> catalogueHits;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    // Board-scoped landing grid: the board-tab strip actually scopes
    // what the launcher shows. Only the selected board's pins render,
    // under a "PINNED · {board}" section header.
    final scope = ref.watch(popupBoardScopeProvider);
    if (scope is PopupScopePinned) {
      return Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(
            // Catalog copy carries the uppercase "PINNED" prefix; the
            // board title renders verbatim (no forced casing).
            t(ref, popupPinnedSectionKey, {'board': scope.boardTitle}),
            key: const Key('popup-pinned-header'),
            style: TextStyle(
              fontFamily: upegMonoFontFamily,
              fontFamilyFallback: upegMonoFontFamilyFallback,
              fontSize: 10,
              letterSpacing: 0.4,
              color: tokens.fg3,
            ),
          ),
          const SizedBox(height: 6),
          Flexible(
            child: _PopupHitGrid(hits: scope.hits, tokens: tokens),
          ),
        ],
      );
    }
    // Empty query → render the full registered catalogue as the
    // default landing grid (not a 6-item slice).
    if (query.trim().isEmpty) {
      return catalogueHits.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (err, _) => Padding(
          padding: const EdgeInsets.all(12),
          child: Text(
            t(ref, 'popup.catalogue_failed', {'msg': '$err'}),
            style: TextStyle(color: tokens.warn),
          ),
        ),
        data: (hits) {
          if (hits.isEmpty) {
            return Padding(
              key: const Key('popup-empty-hint'),
              padding: const EdgeInsets.all(12),
              child: Text(
                t(ref, popupEmptyHintKey),
                style: TextStyle(color: tokens.fg3, fontSize: 11),
              ),
            );
          }
          return _PopupHitGrid(hits: hits, tokens: tokens);
        },
      );
    }
    return results.when(
      loading: () => const Center(child: CircularProgressIndicator()),
      error: (err, _) => Padding(
        padding: const EdgeInsets.all(12),
        child: Text(
          t(ref, 'common.search_failed', {'msg': '$err'}),
          style: TextStyle(color: tokens.warn),
        ),
      ),
      data: (hits) {
        if (hits.isEmpty) {
          return Padding(
            key: const Key('popup-no-results'),
            padding: const EdgeInsets.all(12),
            child: Text(
              t(ref, popupNoResultsHintKey, {'needle': query.trim()}),
              style: TextStyle(color: tokens.fg3, fontSize: 11),
            ),
          );
        }
        return _PopupHitGrid(hits: hits, tokens: tokens);
      },
    );
  }
}

class _PopupHitGrid extends ConsumerWidget {
  const _PopupHitGrid({required this.hits, required this.tokens});

  final List<PaletteHit> hits;
  final UpegTokens tokens;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final selectionIndex = ref.watch(popupSelectionProvider).index;
    return GridView.builder(
      shrinkWrap: true,
      gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
        crossAxisCount: popupGridCols,
        mainAxisExtent: popupCellHeight,
        crossAxisSpacing: popupCellGap,
        mainAxisSpacing: popupCellGap,
      ),
      itemCount: hits.length,
      itemBuilder: (context, i) {
        final hit = hits[i];
        return PopupHitCell(
          key: ValueKey('popup-hit-${hit.id}'),
          hit: hit,
          tokens: tokens,
          isSelected: i == selectionIndex,
          onTap: () => _activate(ref, ToolId.parse(hit.id)),
        );
      },
    );
  }

  /// Route the tap through the shared popup activation policy:
  /// immediate-dispatch tools run inline (the popup stays open and the
  /// cell renders the result); form/embed tools bridge to the full
  /// dashboard via `pendingActivationProvider`.
  static void _activate(WidgetRef ref, ToolId toolId) {
    unawaited(ref.read(popupActivationControllerProvider).activate(toolId));
  }
}

/// One cell in the popup hit grid. Extracted from `_PopupBody` so the
/// keyboard cursor + tap callback can live on a single widget without
/// duplicating the visual shell. Renders the inline run state in place:
/// an activity dot while the dispatch is in flight, then the OK/ERROR
/// badge + output preview once the outcome lands. The cell stays
/// tappable throughout so the same tool can run again from the result
/// state.
class PopupHitCell extends ConsumerWidget {
  const PopupHitCell({
    required this.hit,
    required this.tokens,
    this.isSelected = false,
    this.onTap,
    super.key,
  });

  final PaletteHit hit;
  final UpegTokens tokens;
  final bool isSelected;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final toolId = ToolId.parse(hit.id);
    final outcome = ref.watch(popupInlineOutcomeForProvider(toolId));
    final running = ref.watch(
      popupRunningToolsProvider.select((tools) => tools.contains(toolId)),
    );
    final idText = Text(
      hit.id.toUpperCase(),
      style: TextStyle(
        fontFamily: upegMonoFontFamily,
        fontFamilyFallback: upegMonoFontFamilyFallback,
        fontSize: 10,
        letterSpacing: 0.4,
        color: tokens.fg3,
      ),
    );
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(UpegSizing.radius1),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
        decoration: BoxDecoration(
          color: tokens.surface,
          border: Border.all(
            color: isSelected ? tokens.accent : tokens.line,
            width: isSelected ? 2 : 1,
          ),
          borderRadius: BorderRadius.circular(UpegSizing.radius1),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            if (running)
              Row(
                children: [
                  Expanded(child: idText),
                  SizedBox(
                    key: ValueKey('popup-inline-running-${hit.id}'),
                    width: 10,
                    height: 10,
                    child: CircularProgressIndicator(
                      strokeWidth: 2,
                      color: tokens.accent,
                    ),
                  ),
                ],
              )
            else
              idText,
            const SizedBox(height: 2),
            Text(
              hit.label,
              style: TextStyle(
                fontFamily: upegMonoFontFamily,
                fontFamilyFallback: upegMonoFontFamilyFallback,
                fontSize: 12,
                color: tokens.fg,
              ),
            ),
            if (outcome != null) ...[
              const SizedBox(height: 4),
              Expanded(
                child: _PopupInlineResult(
                  toolId: toolId,
                  outcome: outcome,
                  tokens: tokens,
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

/// Inline OK/ERROR + output-preview block rendered inside a popup cell
/// after an inline run (chrome-ext popup parity).
class _PopupInlineResult extends ConsumerWidget {
  const _PopupInlineResult({
    required this.toolId,
    required this.outcome,
    required this.tokens,
  });

  final ToolId toolId;
  final CanonicalToolResult outcome;
  final UpegTokens tokens;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final ok = outcome.ok;
    final badgeKey = ok
        ? 'popup-inline-ok-${toolId.value}'
        : 'popup-inline-error-${toolId.value}';
    final preview = ok
        ? (outcome.primaryOutputText.isEmpty
              ? t(ref, popupResultEmptyOutputKey)
              : outcome.primaryOutputText)
        : (outcome.errorMessage ?? t(ref, popupResultEmptyOutputKey));
    final copyText = popupCopyTextFor(outcome);
    return Column(
      key: ValueKey('popup-inline-result-${toolId.value}'),
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Row(
          children: [
            Text(
              t(ref, ok ? popupResultOkKey : popupResultErrorKey),
              key: ValueKey(badgeKey),
              style: TextStyle(
                fontFamily: upegMonoFontFamily,
                fontFamilyFallback: upegMonoFontFamilyFallback,
                fontSize: 9,
                letterSpacing: 0.4,
                fontWeight: FontWeight.bold,
                color: ok ? tokens.accent : tokens.warn,
              ),
            ),
            const Spacer(),
            CopyToClipboardButton(
              key: ValueKey('popup-inline-copy-${toolId.value}'),
              textToCopy: copyText ?? '',
              writer: ref.watch(clipboardWriterProvider),
              tooltip: t(ref, popupResultCopyTooltipKey),
            ),
          ],
        ),
        Flexible(
          child: Text(
            preview,
            maxLines: 2,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(
              fontFamily: upegMonoFontFamily,
              fontFamilyFallback: upegMonoFontFamilyFallback,
              fontSize: 10,
              color: tokens.fg2,
            ),
          ),
        ),
      ],
    );
  }
}

int _popupIndexForDirection(int current, int hitCount, DirectionDto direction) {
  if (hitCount <= 0) return 0;
  final col = current % popupGridCols;
  return switch (direction) {
    DirectionDto.left => col == 0 ? current : current - 1,
    DirectionDto.right =>
      col + 1 >= popupGridCols || current + 1 >= hitCount
          ? current
          : current + 1,
    DirectionDto.up =>
      current - popupGridCols < 0 ? current : current - popupGridCols,
    DirectionDto.down =>
      current + popupGridCols >= hitCount ? current : current + popupGridCols,
  };
}
