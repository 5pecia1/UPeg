/// Main shell once `initApp()` resolves.
///
/// Layout contract:
///   * top tab bar with boards + the right-aligned `+ pin` / `search`
///     / `edit` / `settings` cluster,
///   * board canvas with the pegboard dot-grid background,
///   * status bar pinned to the bottom.
///
/// Cmd/Ctrl+K shortcut opens the palette; the rest of the keyboard
/// vocabulary (Esc / function keys / arrow keys / F / `[` / `]` /
/// 1..9 / B …) is resolved through the Rust-side
/// `keyboardCommandFor` FRB so the binding policy stays canonical with
/// the TUI surface. See `upeg-frb/src/api/keyboard.rs`.
///
/// Tab traverses the toolbar and the board grid. The grid uses arrows to
/// select pins. Enter can move focus into an embed pin body and Escape
/// returns it to the grid.
library;

export 'package:upeg/src/keyboard/keyboard_command_resolver.dart';
export 'package:upeg/src/pages/board_page/quit_confirm_dialog.dart';

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/boot/startup_profile.dart';
import 'package:upeg/src/keyboard/keyboard_command_resolver.dart';
import 'package:upeg/src/pages/embed_page.dart';
import 'package:upeg/src/platform/keyboard_label.dart';
import 'package:upeg/src/rust/api/boot.dart';
import 'package:upeg/src/rust/api/keyboard.dart';
import 'package:upeg/src/rust/api/palette.dart';
import 'package:upeg/src/rust/api/pegboard.dart' show PlacementDto;
import 'package:upeg/src/rust/api/pin_activation.dart';
import 'package:upeg/src/rust/api/tools.dart'
    show CanonicalToolResult, PinKindDto, SourceDto_Shortcut;
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/features/memos/memo_roles.dart';
import 'package:upeg/src/features/memos/memos_provider.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/focused_pin_provider.dart';
import 'package:upeg/src/state/inline_draft_provider.dart';
import 'package:upeg/src/state/last_outcome_provider.dart';
import 'package:upeg/src/state/live_outcome_provider.dart'
    show liveDispatchToolFnProvider;
import 'package:upeg/src/state/move_mode_provider.dart';
import 'package:upeg/src/state/move_pin_slot_provider.dart';
import 'package:upeg/src/state/pegboard_mutations_provider.dart';
import 'package:upeg/src/state/pin_activation_provider.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/state/reorder_pin_provider.dart';
import 'package:upeg/src/state/resize_mode_provider.dart';
import 'package:upeg/src/state/running_tools_provider.dart';
import 'package:upeg/src/state/tag_provider.dart';
import 'package:upeg/src/state/window_mode_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/tool_roles.dart';
import 'package:upeg/src/widgets/board_canvas.dart';
import 'package:upeg/src/widgets/board_canvas/pin_with_label.dart';
import 'package:upeg/src/widgets/board_tabs.dart';
import 'package:upeg/src/widgets/cheatsheet_overlay.dart';
import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';
import 'package:upeg/src/widgets/copyable_outcome_text.dart';
import 'package:upeg/src/widgets/expanded_modal_button.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';
import 'package:upeg/src/widgets/palette_overlay.dart';
import 'package:upeg/src/widgets/pending_activation_bridge.dart';
import 'package:upeg/src/widgets/settings_overlay.dart';
import 'package:upeg/src/widgets/pin_color_dialog.dart';
import 'package:upeg/src/widgets/status_bar.dart';
import 'package:upeg/src/widgets/tag_chips.dart';
import 'package:upeg/src/pages/board_page/board_selection_commands.dart';
import 'package:upeg/src/pages/board_page/directional_focus.dart';

class BoardPage extends ConsumerStatefulWidget {
  const BoardPage({
    required this.report,
    required this.onQuitRequest,
    super.key,
  });

  final AppInitReport report;
  final Future<void> Function() onQuitRequest;

  @override
  ConsumerState<BoardPage> createState() => _BoardPageState();
}

class _BoardPageState extends ConsumerState<BoardPage> {
  static bool _firstPostFrameRecorded = false;
  final FocusNode _rootFocus = FocusNode(
    debugLabel: 'BoardPage',
    skipTraversal: true,
  );
  final FocusNode _gridFocus = FocusNode(debugLabel: 'Board grid');

  @override
  void dispose() {
    _gridFocus.dispose();
    _rootFocus.dispose();
    super.dispose();
  }

  @override
  void initState() {
    super.initState();
    if (startupProfileEnabled && !_firstPostFrameRecorded) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted || _firstPostFrameRecorded) return;
        _firstPostFrameRecorded = true;
        markStartupPhase('board_first_post_frame');
      });
    }
    Future<void>.microtask(_restorePegboardSelection);
  }

  /// Boot-time hydration from the shared native pegboard selection.
  /// `selectedTagProvider` watches the same central state, so restoring
  /// through the board-facing facade hydrates both board and tag.
  Future<void> _restorePegboardSelection() async {
    await ref.read(currentBoardKeyProvider.notifier).restore();
  }

  /// Right-click / long-press "open" context-menu affordance: focus the pin
  /// and open its expanded modal — the mouse/touch mirror of keyboard `o`.
  Future<void> _openModalForPlacement(
    BoardKey boardKey,
    PlacementDto placement,
  ) async {
    final toolId = ToolId.parse(placement.toolId);
    final pinKey = (boardKey, PinId.parse(placement.pinId));
    ref.read(focusedPinProvider.notifier).focus(pinKey.$2);
    await _openModalForToolId(boardKey, toolId, pinKey: pinKey);
  }

  Future<void> _openModalForToolId(
    BoardKey boardKey,
    ToolId toolId, {
    PinKey? pinKey,
  }) async {
    // Seed the modal with anything typed inline on the tile (points 1 + 3).
    final draft = pinKey == null
        ? null
        : ref.read(inlineDraftProvider).read(pinKey);
    final result = await openExpandedModalForToolId(
      context,
      ref,
      toolId,
      initialInput: draft,
      pinKey: pinKey,
    );
    if (!mounted) return;
    if (result is ExpandedModalToolUnknown) {
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text('unknown tool: ${result.toolId}')));
    }
  }

  void _onPinTap(PlacementDto placement) {
    _gridFocus.requestFocus();
    // Capture the tool id BEFORE handing control to async branches so a
    // mid-flight UI state reset cannot race the activation lookup —
    // mirrors the snapshot pattern memorialised in
    // feedback_dragend_race.md.
    final toolId = ToolId.parse(placement.toolId);
    focusPlacement(ProviderScope.containerOf(context), placement);
    _activateToolId(
      toolId,
      pinKey: (
        ref.read(currentBoardKeyProvider)!,
        PinId.parse(placement.pinId),
      ),
    );
  }

  /// Route [toolId] through the shared activation policy. Single seam for
  /// pin taps, palette hits, keyboard Run, and shortcut-source tools.
  void _activateToolId(ToolId toolId, {PinKey? pinKey}) {
    final activation = ref.read(pinActivationProvider)(
      toolId: toolId,
      argsJson: ToolArgs.emptyJson,
    );
    unawaited(_dispatchActivation(activation, pinKey: pinKey));
  }

  /// Whether [toolId] has a placement on the currently visible board
  /// (honouring the active tag filter). An embed tool pinned on the
  /// visible board is already rendered inline via `bodyOverride`, so
  /// activation focuses that inline pin instead of pushing a full-screen
  /// EmbedPage.
  bool _isPinnedOnVisibleBoard(String toolId) {
    for (final placement in visiblePlacementsForKeyboard(ref)) {
      if (placement.toolId == toolId) return true;
    }
    return false;
  }

  Future<void> _dispatchActivation(
    PinActivationDto activation, {
    PinKey? pinKey,
  }) async {
    switch (activation) {
      case PinActivationDto_OpenEmbed(:final toolId):
        // Inline-canonical: the inline pin already IS the embed tool.
        // Focus it rather than pushing a full-screen EmbedPage. The
        // full-screen surface stays only for off-board entry (deep
        // links, tools not pinned on the visible board).
        if (pinKey != null || _isPinnedOnVisibleBoard(toolId)) {
          final placement = visiblePlacementsForKeyboard(
            ref,
          ).where((p) => p.toolId == toolId).firstOrNull;
          final focusId =
              pinKey?.$2 ??
              (placement == null ? null : PinId.parse(placement.pinId));
          if (focusId != null) {
            ref.read(focusedPinProvider.notifier).focus(focusId);
          }
          return;
        }
        final parsedToolId = ToolId.parse(toolId);
        // Catalog might still be loading — wait for it, then retry.
        // Mirrors `LaunchIntentApplier._openEmbedForToolId`.
        var tool = ref.read(toolByIdProvider(parsedToolId));
        if (tool == null) {
          await ref.read(toolsProvider.future);
          if (!mounted) return;
          tool = ref.read(toolByIdProvider(parsedToolId));
        }
        if (tool == null) {
          // Tool still missing after catalog load — show error, don't navigate.
          final messenger = ScaffoldMessenger.maybeOf(context);
          messenger?.showSnackBar(
            SnackBar(content: Text('Tool not found: $parsedToolId')),
          );
          return;
        }
        unawaited(EmbedPage.open(context, tool));
      case PinActivationDto_DispatchImmediate(:final toolId):
        final parsedToolId = ToolId.parse(toolId);
        final tool = ref.read(toolByIdProvider(parsedToolId));
        // memo.create-style action: no headless dispatcher — create a new
        // memo via the memos feature and focus the on-board notepad so
        // the fresh (empty) memo is ready to type into.
        if (tool != null && isMemoCreateAction(tool)) {
          _createMemoAndFocusNotepad();
          return;
        }
        // Honest state: an unconfigured live-http pin (eth.gas) cannot
        // run — surface a clear "needs setup" message instead of firing a
        // dispatch that would just fail generically.
        if (tool != null && toolNeedsProviderConfig(tool)) {
          final messenger = ScaffoldMessenger.maybeOf(context);
          messenger?.showSnackBar(
            SnackBar(
              content: Text(
                '$parsedToolId: ${tRead(ref, providerNotConfiguredMessageKey)}',
              ),
            ),
          );
          return;
        }
        // Inline-first dispatch: run on FRB's async worker so a slow tool
        // never freezes the board. The pin shows an in-flight indicator
        // (runningToolsProvider); on success the canonical result is
        // recorded so the pin body renders it INLINE (issue 7). A
        // snackbar is kept only for errors and for off-board tools that
        // have no pin to render into.
        final running = ref.read(runningToolsProvider.notifier);
        final dispatch = ref.read(liveDispatchToolFnProvider);
        final recorder = ref.read(lastOutcomeProvider.notifier);
        final runToken = pinKey == null ? null : recorder.beginRun(pinKey);
        final runningLease = pinKey == null ? null : running.begin(pinKey);
        late final CanonicalToolResult outcome;
        try {
          outcome = await dispatch(
            pinKey: pinKey,
            toolId: parsedToolId,
            args: ToolArgs.empty,
          );
        } finally {
          if (runningLease != null) running.end(runningLease);
        }
        if (pinKey != null) {
          recorder.record(
            pinKey,
            parsedToolId,
            outcome,
            args: ToolArgs.empty,
            runToken: runToken,
          );
        }
        if (!mounted) return;
        final pinnedOnBoard = _isPinnedOnVisibleBoard(toolId);
        if (!outcome.ok || !pinnedOnBoard) {
          final messenger = ScaffoldMessenger.maybeOf(context);
          if (messenger == null) return;
          final text = outcome.snackbarText('$parsedToolId: ok');
          messenger.showSnackBar(SnackBar(content: Text(text)));
        }
      case PinActivationDto_OpenModal(:final toolId):
        final boardKey = ref.read(currentBoardKeyProvider);
        if (boardKey == null) return;
        unawaited(
          _openModalForToolId(boardKey, ToolId.parse(toolId), pinKey: pinKey),
        );
    }
  }

  /// Create a fresh memo, make it the active notepad memo, and focus the
  /// on-board notepad pin (if one is visible) so typing lands in the new
  /// memo. Backs `memo.create` / Cmd+Shift+N.
  void _createMemoAndFocusNotepad() {
    final key = ref.read(memosProvider.notifier).create();
    for (final placement in visiblePlacementsForKeyboard(ref)) {
      final tool = ref.read(toolByIdProvider(ToolId.parse(placement.toolId)));
      if (tool != null && isMemoNotepadTool(tool)) {
        final boardKey = ref.read(currentBoardKeyProvider);
        if (boardKey == null) return;
        final pinKey = (boardKey, PinId.parse(placement.pinId));
        ref.read(activeMemoKeyProvider(pinKey).notifier).setActive(key);
        ref
            .read(focusedPinProvider.notifier)
            .focus(PinId.parse(placement.pinId));
        return;
      }
    }
  }

  void _onPaletteHit(PaletteHit hit, PinKey? pinKey) {
    // Route palette selections through the same activation policy as
    // pin taps so embed tools land in EmbedPage and launcher tools fire
    // immediately. Without this, every palette hit forced the modal,
    // which renders nothing useful for embed-kind tools.
    _activateToolId(ToolId.parse(hit.id), pinKey: pinKey);
  }

  Future<void> _openPalette() {
    return showPaletteOverlay(context, onPick: _onPaletteHit);
  }

  Future<void> _openSettings() {
    return showSettingsOverlay(context);
  }

  /// Translate a raw Flutter key event into Rust-resolved
  /// [KeyboardCommandDto] and dispatch the result to the right
  /// side-effect. Returns `true` when the event was handled so the
  /// caller can mark it as consumed.
  bool _handleKey(KeyEvent event) {
    if (!isKeyboardPress(event)) return false;
    if (primaryFocusHasActiveTextComposition()) return false;
    if (event is KeyRepeatEvent &&
        event.logicalKey == LogicalKeyboardKey.escape) {
      return true;
    }
    if (_handleBoardShortcut(event)) return true;
    if (event.logicalKey == LogicalKeyboardKey.tab) return false;
    if (_handleEmbedBodyKey(event)) return true;

    final boardOwnsFocus =
        _gridFocus.hasPrimaryFocus || _rootFocus.hasPrimaryFocus;
    if (!boardOwnsFocus && !isGlobalShortcutKeyEvent(event)) return false;

    final hasFocus = ref.read(focusedPinProvider) != null;
    // Resize and move modes are mutually exclusive (starting one cancels
    // the other — see handleResizeModeCommand), so the scope check order
    // only matters as a tie-break for corrupt state: resize wins.
    final KeyboardScopeDto scope;
    if (ref.read(resizeModeProvider) is ResizeModeActive) {
      scope = KeyboardScopeDto.resize;
    } else if (ref.read(moveModeProvider) is MoveModeActive) {
      scope = KeyboardScopeDto.moving;
    } else {
      scope = KeyboardScopeDto.board;
    }
    if (boardOwnsFocus &&
        scope == KeyboardScopeDto.board &&
        event is KeyDownEvent) {
      if (isKeyboardContextMenuEvent(event)) {
        final showMenu = _selectedPinFocusNode()?.showContextMenu;
        if (showMenu == null) return false;
        showMenu();
        return true;
      }
      final keyboard = HardwareKeyboard.instance;
      if (keyboardLabelForEvent(event) == 'Enter' &&
          !keyboard.isControlPressed &&
          !keyboard.isMetaPressed &&
          !keyboard.isAltPressed &&
          !keyboard.isShiftPressed &&
          _enterSelectedEmbedBody()) {
        return true;
      }
    }

    // Shortcut-source tools use the chord declared in the catalog.
    if (_handleShortcutSourceTool(event)) return true;
    final cmd = resolveKeyboardCommand(
      ref,
      event,
      scope: scope,
      hasToolFocus: hasFocus,
    );
    if (cmd == null) return false;
    return _dispatchCommand(cmd);
  }

  bool _handleBoardShortcut(KeyEvent event) {
    if (primaryFocusIsEditableText() || primaryFocusIsInsideEmbedBody()) {
      return false;
    }
    final keyboard = HardwareKeyboard.instance;
    if (keyboard.isAltPressed) return false;
    if (event.logicalKey == LogicalKeyboardKey.tab &&
        keyboard.isControlPressed &&
        !keyboard.isMetaPressed) {
      cycleBoard(ref, forward: !keyboard.isShiftPressed);
      return true;
    }
    if (event is! KeyDownEvent ||
        event.logicalKey != LogicalKeyboardKey.keyN ||
        keyboard.isShiftPressed) {
      return false;
    }
    final isMacOS = ref.read(keyboardPlatformProvider) == TargetPlatform.macOS;
    final hasPrimary = isMacOS
        ? keyboard.isMetaPressed && !keyboard.isControlPressed
        : keyboard.isControlPressed && !keyboard.isMetaPressed;
    if (!hasPrimary) return false;
    unawaited(promptCreateBoard(context, ref));
    return true;
  }

  bool _handleEmbedBodyKey(KeyEvent event) {
    if (event.logicalKey != LogicalKeyboardKey.escape ||
        !primaryFocusIsInsideEmbedBody()) {
      return false;
    }
    _gridFocus.requestFocus();
    return true;
  }

  BoardPinFocusNode? _selectedPinFocusNode() {
    final pinId = ref.read(focusedPinProvider);
    if (pinId == null) return null;
    return _gridFocus.descendants
        .whereType<BoardPinFocusNode>()
        .where((node) => node.pinId == pinId)
        .firstOrNull;
  }

  bool _enterSelectedEmbedBody() {
    final node = _selectedPinFocusNode();
    if (node == null) return false;
    final tool = ref.read(toolByIdProvider(node.toolId));
    if (tool == null) return false;
    if (tool.pinKind != PinKindDto.embed &&
        tool.pinKind != PinKindDto.controlledEmbed &&
        !isMemoNotepadTool(tool)) {
      return false;
    }
    if (node.enterEmbedBody()) return true;
    if (tool.pinKind != PinKindDto.embed) return false;
    // Some platform views lack a Flutter keyboard focus target. Their
    // expanded page gives keyboard users a focused browser-launch action.
    unawaited(EmbedPage.open(context, tool, focusExternalAction: true));
    return true;
  }

  /// Activate any catalog tool whose declared `Shortcut` source chord
  /// matches [event]. Generic over example: the binding lives in the
  /// tool's metadata (`SourceDto.shortcut(keys:)`), so no chord is
  /// hard-coded here. Only scans when a primary modifier is held, since
  /// every shortcut-source tool uses a Cmd/Ctrl chord.
  bool _handleShortcutSourceTool(KeyEvent event) {
    if (!isKeyboardPress(event)) return false;
    final keyboard = HardwareKeyboard.instance;
    if (!keyboard.isControlPressed && !keyboard.isMetaPressed) return false;
    final tools = ref.read(toolsProvider).value;
    if (tools == null) return false;
    for (final tool in tools) {
      final source = tool.source;
      if (source is! SourceDto_Shortcut) continue;
      if (!shortcutKeysMatchEvent(source.keys, event)) continue;
      _activateToolId(ToolId.parse(tool.id));
      return true;
    }
    return false;
  }

  /// Apply a resolved [KeyboardCommandDto] to the page state. Returns
  /// `true` when the command consumed the event.
  bool _dispatchCommand(KeyboardCommandDto cmd) {
    // F15: move-mode commands intercept ahead of the other handlers
    // so `m / arrows / Enter / Esc` flow into the state machine when
    // a pin is focused. The handler returns `false` when the
    // command does not apply, letting the downstream switch claim
    // the event.
    // Resize-mode intercept runs FIRST: while resize is Active it owns
    // Commit/Cancel; it also cancels a stale resize when StartMove
    // arrives (returning false so the move intercept below claims it).
    if (handleResizeModeCommand(ProviderScope.containerOf(context), cmd: cmd)) {
      setState(() {}); // trigger rebuild for the canvas preview overlay
      return true;
    }
    if (handleMoveModeCommand(ProviderScope.containerOf(context), cmd: cmd)) {
      setState(() {}); // trigger rebuild for the canvas preview overlay
      return true;
    }
    switch (cmd) {
      case KeyboardCommandDto_Search():
        unawaited(_openPalette());
        return true;
      case KeyboardCommandDto_Close():
        ref.read(windowModeProvider.notifier).set(WindowMode.popup);
        return true;
      case KeyboardCommandDto_OpenSettings():
        unawaited(_openSettings());
        return true;
      case KeyboardCommandDto_CycleBoardFilter():
        cycleBoard(ref, forward: true);
        return true;
      case KeyboardCommandDto_CycleTagFilter():
        cycleTag(ref);
        return true;
      case KeyboardCommandDto_ClearBoardFilter():
        clearBoardFilter(ref);
        return true;
      case KeyboardCommandDto_SwitchBoard(:final slot):
        switchBoardSlot(ref, slot);
        return true;
      case KeyboardCommandDto_NewBoard():
        unawaited(promptCreateBoard(context, ref));
        return true;
      case KeyboardCommandDto_RenameBoard():
        final board = currentBoard(ref);
        if (board != null) {
          unawaited(promptRenameBoard(context, ref, board));
        }
        return true;
      case KeyboardCommandDto_DeleteBoard():
        final board = currentBoard(ref);
        if (board != null) {
          unawaited(confirmDeleteBoard(context, ref, board));
        }
        return true;
      case KeyboardCommandDto_OpenToolPicker():
        unawaited(_openPalette());
        return true;
      case KeyboardCommandDto_Reorder():
        unawaited(dispatchReorderPin(ProviderScope.containerOf(context), cmd));
        return true;
      case KeyboardCommandDto_MovePinPrev():
      case KeyboardCommandDto_MovePinNext():
        // Cmd+[ / Cmd+] — Q6, inventory row F12. The helper resolves
        // the focused pin and dispatches the matching delta; no-op
        // when no pin is in focus (dispatchMovePinSlot short-circuits).
        unawaited(
          dispatchMovePinSlot(ProviderScope.containerOf(context), cmd: cmd),
        );
        return true;
      case KeyboardCommandDto_StartMove():
      case KeyboardCommandDto_StartResize():
        // Consumed even when the intercepts above declined (no focused
        // pin / stale layout) so `m` / `e` never leak downstream.
        return true;
      case KeyboardCommandDto_ResizeBy():
      case KeyboardCommandDto_ResetSpan():
        // Only emitted in the resize scope, which the resize intercept
        // already claimed — reaching here means resize is Idle.
        return false;
      case KeyboardCommandDto_TogglePin():
        unawaited(toggleFocusedPin(ref));
        return true;
      case KeyboardCommandDto_Run():
        // Enter / F1 — inline-first activation (issue 7): runs the pin in
        // place. Inline / Action / Live / Launcher pins render their
        // result inline; the modal is no longer opened implicitly.
        _activateFocusedPin();
        return true;
      case KeyboardCommandDto_Open():
        // `o` — the explicit "inspect" affordance. Opens the expanded
        // modal for the focused pin. The new contract demotes the modal
        // to this gesture (plus the pin context menu) so a plain Run no
        // longer implies a full-screen page.
        _openModalForFocusedPin();
        return true;
      // Variants that don't apply at the BoardPage scope fall through
      // so the key event keeps propagating to a more specific handler.
      case KeyboardCommandDto_Move(:final direction):
        _moveKeyboardFocus(direction);
        return true;
      case KeyboardCommandDto_FocusNext():
        _moveKeyboardFocusLinear(forward: true);
        return true;
      case KeyboardCommandDto_FocusPrevious():
        _moveKeyboardFocusLinear(forward: false);
        return true;
      case KeyboardCommandDto_EditPinColor():
        return _openPinColorDialog();
      case KeyboardCommandDto_Quit():
        // Contract (upeg_core::keyboard_catalog): `q` opens the
        // quit-confirm dialog — never an immediate exit. Mirrors the
        // TUI ConfirmQuit view.
        unawaited(widget.onQuitRequest());
        return true;
      case KeyboardCommandDto_ShowCheatsheet():
        // `?` — keyboard cheatsheet overlay. Renders the shared binding
        // catalog; Esc closes it (shared confirm scope).
        unawaited(
          showCheatsheetOverlay(context, currentScope: KeyboardScopeDto.board),
        );
        return true;
      case KeyboardCommandDto_Page():
      case KeyboardCommandDto_Home():
      case KeyboardCommandDto_End():
      case KeyboardCommandDto_Commit():
      case KeyboardCommandDto_Confirm():
      case KeyboardCommandDto_Cancel():
      case KeyboardCommandDto_Text():
      case KeyboardCommandDto_Backspace():
        return false;
      case KeyboardCommandDto_Copy():
        return _copyFocusedPin();
      // Display-only text editing shortcut — keep propagating.
      case KeyboardCommandDto_ClearInput():
        return false;
    }
  }

  bool _openPinColorDialog() {
    final toolId = ref.read(focusedPinProvider);
    if (toolId == null) return false;
    return _openPinColorDialogFor(toolId);
  }

  bool _copyFocusedPin() {
    final pinId = ref.read(focusedPinProvider);
    final boardKey = ref.read(currentBoardKeyProvider);
    if (pinId == null || boardKey == null) return false;
    final placement = visiblePlacementsForKeyboard(
      ref,
    ).where((p) => p.pinId == pinId.value).firstOrNull;
    if (placement == null) return false;
    final saved = ref.read(pinLastOutcomeProvider((boardKey, pinId)));
    final result = saved?.result;
    final draft = ref.read(inlineDraftProvider).read((
      boardKey,
      pinId,
    ), toolId: ToolId.parse(placement.toolId));
    final tool = ref.read(toolByIdProvider(ToolId.parse(placement.toolId)));
    final current =
        draft ??
        (tool != null && tool.inputFields.isEmpty ? ToolArgs.empty : null);
    final relation = resultInputRelation(saved?.args, current);
    final text = copyableOutcomeText(result);
    if (text == null) {
      final messenger = ScaffoldMessenger.of(context);
      messenger.clearSnackBars();
      messenger.removeCurrentSnackBar();
      messenger.showSnackBar(
        SnackBar(content: Text(tRead(ref, 'clipboard.no_text_result'))),
      );
      return true;
    }
    final writer = ref.read(clipboardWriterProvider);
    unawaited(
      copyWithFeedback(
        context: context,
        ref: ref,
        write: () => writer.write(text),
        successKey: copySuccessKeyForRelation(relation),
      ),
    );
    return true;
  }

  bool _openPinColorDialogFor(PinId toolId) {
    final boardKey = ref.read(currentBoardKeyProvider);
    if (boardKey == null) return false;

    final selectedTag = ref.read(selectedTagProvider);
    final snapshot = ref.read(layoutLoaderProvider)(
      LayoutQuery(boardKey: boardKey, tag: selectedTag),
    );
    String? currentColor;
    var placementFound = false;
    for (final placement in snapshot.placements) {
      if (placement.pinId == toolId.value) {
        currentColor = placement.color;
        placementFound = true;
        break;
      }
    }
    if (!placementFound) {
      ref.read(focusedPinProvider.notifier).blur();
      return false;
    }

    showDialog<void>(
      context: context,
      barrierColor: Colors.black54,
      barrierDismissible: true,
      builder: (_) => Center(
        child: Material(
          color: Colors.transparent,
          child: PinColorDialog(
            initialColor: currentColor,
            onSave: (colorHex) {
              if (!mounted) return;
              Navigator.of(context).pop();
              ref.read(pegboardMutationsProvider).setPinColor((
                boardKey,
                toolId,
              ), color: colorHex);
            },
            onReset: (_) {
              if (!mounted) return;
              Navigator.of(context).pop();
              ref.read(pegboardMutationsProvider).setPinColor((
                boardKey,
                toolId,
              ), color: null);
            },
            onCancel: () {
              if (mounted) Navigator.of(context).pop();
            },
          ),
        ),
      ),
    );
    return true;
  }

  bool _focusAndOpenPinColorDialog(PlacementDto placement) {
    final toolId = PinId.parse(placement.pinId);
    ref.read(focusedPinProvider.notifier).focus(toolId);
    return _openPinColorDialogFor(toolId);
  }

  void _activateFocusedPin() {
    final toolId = ref.read(focusedPinProvider);
    if (toolId == null) return;
    final boardKey = ref.read(currentBoardKeyProvider);
    if (boardKey == null) return;
    final placement = visiblePlacementsForKeyboard(
      ref,
    ).where((p) => p.pinId == toolId.value).firstOrNull;
    if (placement != null) {
      _activateToolId(
        ToolId.parse(placement.toolId),
        pinKey: (boardKey, toolId),
      );
    }
  }

  /// Explicit "inspect" affordance (keyboard `o`): open the expanded
  /// modal for the focused pin regardless of its activation kind.
  void _openModalForFocusedPin() {
    final toolId = ref.read(focusedPinProvider);
    final boardKey = ref.read(currentBoardKeyProvider);
    if (toolId == null || boardKey == null) return;
    final placement = visiblePlacementsForKeyboard(
      ref,
    ).where((p) => p.pinId == toolId.value).firstOrNull;
    if (placement != null) {
      unawaited(
        _openModalForToolId(
          boardKey,
          ToolId.parse(placement.toolId),
          pinKey: (boardKey, toolId),
        ),
      );
    }
  }

  void _moveKeyboardFocusLinear({required bool forward}) {
    final placements = visiblePlacementsForKeyboard(ref);
    if (placements.isEmpty) return;
    final current = ref.read(focusedPinProvider);
    final ids = [
      for (final placement in placements) PinId.parse(placement.pinId),
    ];
    var idx = current == null ? -1 : ids.indexOf(current);
    if (idx < 0) {
      idx = forward ? ids.length - 1 : 0;
    }
    final nextIdx = forward
        ? (idx + 1) % ids.length
        : idx == 0
        ? ids.length - 1
        : idx - 1;
    ref.read(focusedPinProvider.notifier).focus(ids[nextIdx]);
  }

  void _moveKeyboardFocus(DirectionDto direction) {
    final placements = visiblePlacementsForKeyboard(ref);
    if (placements.isEmpty) return;
    final current = ref.read(focusedPinProvider);
    if (current == null) {
      _moveKeyboardFocusLinear(
        forward:
            direction == DirectionDto.right || direction == DirectionDto.down,
      );
      return;
    }
    final currentPlacement = placementForTool(placements, current);
    if (currentPlacement == null) {
      _moveKeyboardFocusLinear(
        forward:
            direction == DirectionDto.right || direction == DirectionDto.down,
      );
      return;
    }
    final next = spatialPlacementCandidate(
      placements,
      currentPlacement,
      direction,
    );
    if (next == null) return;
    ref.read(focusedPinProvider.notifier).focus(PinId.parse(next.pinId));
  }

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    final selectedKey = ref.watch(currentBoardKeyProvider);
    return Focus(
      focusNode: _rootFocus,
      onKeyEvent: (_, event) =>
          _handleKey(event) ? KeyEventResult.handled : KeyEventResult.ignored,
      child: PendingActivationBridge(
        child: Scaffold(
          backgroundColor: tokens.bg,
          body: Column(
            children: [
              BoardTabs(
                onOpenPalette: () => unawaited(_openPalette()),
                onOpenSettings: () => unawaited(_openSettings()),
              ),
              const TagChipRow(),
              Expanded(
                child: Focus(
                  focusNode: _gridFocus,
                  autofocus: true,
                  child: _bodyForSelection(selectedKey),
                ),
              ),
              const StatusBar(),
            ],
          ),
        ),
      ),
    );
  }

  Widget _bodyForSelection(BoardKey? selectedKey) {
    if (selectedKey == null) {
      return const Center(child: CircularProgressIndicator());
    }
    return BoardCanvas(
      boardKey: selectedKey,
      onPinTap: _onPinTap,
      onEditPinColor: _focusAndOpenPinColorDialog,
      onOpenModal: (placement) =>
          unawaited(_openModalForPlacement(selectedKey, placement)),
      onOpenPalette: () => unawaited(_openPalette()),
    );
  }
}
