/// One pin tile on the BoardCanvas.
///
/// Visual contract:
///   * `.cell` chrome: surface + line border, 6px radius, hole-corner
///     decoration on all four corners,
///   * `.cell-hd`: 6px 8px header with kind-coloured icon, tool id
///     (uppercase), toolkit on the right,
///   * `.cell-bd`: body — either field-list summary OR a
///     description fallback,
///   * footer with `KindBadge` + invoker label aligned right.
///
/// `Pin` is rendering-only: the host page wires the
/// `onTap` callback to whatever side-effects (modal open, FRB
/// dispatch, …) it cares about, so the Pin itself stays free of
/// FRB + Riverpod concerns.
library;

import 'dart:async' show unawaited;

import 'package:flutter/material.dart';
import 'package:flutter/semantics.dart' show CustomSemanticsAction;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/i18n/relative_time.dart' show lastRunAgoLabel;
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/keyboard/binding_catalog.dart';
import 'package:upeg/src/keyboard/keyboard_command_resolver.dart'
    show EmbedBodyFocusScope;
import 'package:upeg/src/platform/keyboard_label.dart'
    show keyboardPlatformProvider;
import 'package:upeg/src/rust/api/keyboard.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart'
    show CanonicalToolResult, OutputFieldDto;
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/pegboard_mutations_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/pin_renderers/registry.dart'
    show pinKindChrome;
import 'package:upeg/src/widgets/pin/pin_body.dart';
import 'package:upeg/src/widgets/pin/pin_chrome.dart';
import 'package:upeg/src/widgets/pin/pin_footer.dart';
import 'package:upeg/src/widgets/pin/pin_header.dart';
import 'package:upeg/src/widgets/pin/pin_menu.dart';

/// The pin is assembled from `widgets/pin/`; its widget-key test
/// contract stays reachable from this one import.
export 'package:upeg/src/widgets/pin/pin_keys.dart';

/// Dispatch callback fired when the pin is tapped.
typedef PinTapCallback = void Function(PlacementDto placement);

class Pin extends ConsumerWidget {
  const Pin({
    required this.placement,
    this.toolLabel,
    this.toolDescription,
    this.toolkit,
    this.pinKind,
    this.invokerLabel,
    this.outputFields = const <OutputFieldDto>[],
    this.outputResult,
    this.restoredAt,
    this.outputTruncated = false,
    this.stale = false,
    this.running = false,
    this.focused = false,
    this.showMoveHandle = false,
    this.moveHandle,
    this.cursor = SystemMouseCursors.click,
    this.pinColorOverride,
    this.onTap,
    this.bodyOverride,
    this.exposeBodySemantics = false,
    this.onEditColor,
    this.onOpenModal,
    super.key,
  });

  /// When non-null, replaces the default text body (label/description/
  /// outputs) with a caller-supplied widget. Used for Embed-kind pins
  /// where the body is a live `WebViewPanel` rather than text. Header
  /// (toolkit/id) and footer (kind/invoker/stale) chrome stay the same.
  final Widget? bodyOverride;

  /// Whether an interactive [bodyOverride] keeps its descendant semantics.
  /// The default preserves the Pin's legacy single-node summary semantics.
  final bool exposeBodySemantics;

  final PlacementDto placement;

  /// Human-readable label sourced from the tool catalogue. Falls back to
  /// the tool id when the catalogue hasn't loaded yet or the id is stale.
  final String? toolLabel;

  /// One-line description shown when the tool has no output fields.
  final String? toolDescription;

  /// `tool.toolkit` — rendered on the right side of the header.
  final String? toolkit;

  /// Typed pin kind. Drives the `KindBadge` colour + label at the
  /// bottom of the pin. Null means "kind not yet resolved" (tools
  /// catalogue still loading) — the renderer falls back to the
  /// default chrome.
  final UpegPinKind? pinKind;

  /// `tool.invoker.label()` — short string ("function", "shell", …)
  /// rendered in the footer.
  final String? invokerLabel;

  /// Ordered output fields sourced from the tool's `output_spec.fields`.
  /// Drives compact Pin auto-render from typed schema metadata.
  final List<OutputFieldDto> outputFields;

  /// Canonical dispatch output rendered as compact label/value rows.
  /// Populated by Live+Timer pins via `liveOutcomeProvider`; null keeps
  /// legacy label-only output-field rendering for static catalogue pins.
  final CanonicalToolResult? outputResult;

  /// Non-null when [outputResult] was restored from the persisted
  /// last-outcome store rather than produced this session: the instant
  /// it was originally recorded. Drives the "last run · N ago" badge
  /// under the output rows and the matching Semantics value part.
  final DateTime? restoredAt;

  /// True when the persisted result's outputs were truncated by the
  /// store's size cap — output row previews get a trailing ellipsis.
  final bool outputTruncated;

  /// True when the most-recent dispatch failed but a prior success is
  /// still being shown — drives a muted dot next to the kind label.
  final bool stale;

  /// True while this pin's tool has a dispatch in flight — drives a small
  /// activity spinner in the footer (PRD §6.3 per-pin activity anatomy)
  /// so a slow tool reads as "running" instead of an unresponsive pin.
  final bool running;

  /// True when keyboard move/reorder focus is on this pin.
  final bool focused;

  /// Whether the move handle is shown in the header. Modeless: the board
  /// always shows it so a pin is always movable.
  final bool showMoveHandle;

  /// Optional replacement for the default drag-handle icon. Embed-family
  /// pins inject a `Draggable`-wrapped handle here so the pin can be moved
  /// ONLY from the handle while the live body keeps its pointer events;
  /// plain pins leave this `null` (the whole surface is the drag source)
  /// and render the default icon.
  final Widget? moveHandle;

  /// Mouse cursor selected by the host surface. Normal pins click to open;
  /// edit-mode pins are draggable and should advertise that affordance.
  final MouseCursor cursor;

  /// Optional color override for the pin's border. When present, this
  /// takes precedence over the focused accent color and the kind color.
  final String? pinColorOverride;

  final PinTapCallback? onTap;

  final PinTapCallback? onEditColor;

  /// The explicit "open the expanded modal" affordance for mouse/touch
  /// users, mirroring the keyboard `o` command. Fired from the "open"
  /// context-menu entry (right-click / long-press). Inline-first `onTap`
  /// stays a plain Run; opening the modal is always this deliberate gesture.
  final PinTapCallback? onOpenModal;

  /// Show a Material context menu anchored at [globalPosition]. Tapping the
  /// "unpin" entry dispatches through the central pegboard mutation path using
  /// the active [currentBoardKeyProvider]. The Pin widget owns the menu surface
  /// itself because the menu options are per-pin and very small.
  Future<void> _showContextMenu(
    BuildContext context,
    WidgetRef ref,
    Offset globalPosition,
  ) async {
    final boardKey = ref.read(currentBoardKeyProvider);
    if (boardKey == null) return;
    final overlay =
        Overlay.of(context).context.findRenderObject()! as RenderBox;
    // Keyboard mirrors of the menu entries, looked up from the shared
    // binding catalog (never hard-coded): `o` open, `c` color, `p`
    // toggle-pin/unpin, `e` resize (the menu's "reset size" is the
    // one-shot mirror of entering resize mode and pressing `0`).
    final catalog = ref.read(keyboardBindingCatalogProvider);
    final platform = ref.read(keyboardPlatformProvider);
    final tokens = context.upeg;
    String? boardCap(bool Function(KeyboardCommandDto command) matches) =>
        chordCapForCommand(
          catalog,
          platform,
          scope: KeyboardScopeDto.board,
          matches: matches,
        );
    Widget menuRow(String labelKey, String? keyCap) =>
        PinMenuRow(tokens: tokens, label: tRead(ref, labelKey), keyCap: keyCap);
    final action = await showMenu<PinMenuAction>(
      context: context,
      position: RelativeRect.fromRect(
        Rect.fromPoints(globalPosition, globalPosition),
        Offset.zero & overlay.size,
      ),
      items: <PopupMenuEntry<PinMenuAction>>[
        // "open" mirrors the keyboard `o` command — the explicit mouse/touch
        // affordance for the expanded modal. Only offered when a handler is
        // wired (the board), so bare Pin usages keep a color/unpin-only menu.
        if (onOpenModal != null)
          PopupMenuItem<PinMenuAction>(
            key: const Key('pin-context-open'),
            value: PinMenuAction.open,
            child: menuRow(
              'pin.menu.open',
              boardCap((command) => command is KeyboardCommandDto_Open),
            ),
          ),
        PopupMenuItem<PinMenuAction>(
          key: const Key('pin-context-edit-color'),
          value: PinMenuAction.editColor,
          child: menuRow(
            'pin.menu.edit_color',
            boardCap((command) => command is KeyboardCommandDto_EditPinColor),
          ),
        ),
        // "reset size" only makes sense while a user span override
        // exists — at the manifest footprint there is nothing to reset
        // (same conditional pattern as the onOpenModal-gated "open").
        if (_hasSpanOverride)
          PopupMenuItem<PinMenuAction>(
            key: const Key('pin-context-reset-size'),
            value: PinMenuAction.resetSize,
            child: menuRow(
              'pin.menu.reset_size',
              boardCap((command) => command is KeyboardCommandDto_StartResize),
            ),
          ),
        PopupMenuItem<PinMenuAction>(
          value: PinMenuAction.unpin,
          child: menuRow(
            'pin.menu.unpin',
            boardCap((command) => command is KeyboardCommandDto_TogglePin),
          ),
        ),
      ],
    );
    if (action == PinMenuAction.open) {
      onOpenModal?.call(placement);
      return;
    }
    if (action == PinMenuAction.editColor) {
      onEditColor?.call(placement);
      return;
    }
    if (action == PinMenuAction.resetSize) {
      await _resetSize(ref);
      return;
    }
    if (action == PinMenuAction.unpin) {
      await _unpin(ref);
    }
  }

  /// Whether this placement carries a user span override (either axis).
  bool get _hasSpanOverride =>
      placement.spanCols != null || placement.spanRows != null;

  /// Clear the span override through the central pegboard mutation path
  /// against the active board. Shared by the context-menu entry and the
  /// assistive [CustomSemanticsAction] (mirrors [_unpin]).
  Future<void> _resetSize(WidgetRef ref) async {
    final boardKey = ref.read(currentBoardKeyProvider);
    if (boardKey == null) return;
    await ref
        .read(pegboardMutationsProvider)
        .clearSpan(boardKey, ToolId.parse(placement.toolId));
  }

  /// Unpin through the central pegboard mutation path against the
  /// active board. Shared by the context-menu entry and the assistive
  /// [CustomSemanticsAction] so both surfaces stay behaviourally equal.
  Future<void> _unpin(WidgetRef ref) async {
    final boardKey = ref.read(currentBoardKeyProvider);
    if (boardKey == null) return;
    await ref
        .read(pegboardMutationsProvider)
        .unpin(boardKey, ToolId.parse(placement.toolId));
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = context.upeg;
    final accentColor = tokens.pinKindColor(pinKind);
    // Focus is NOT signalled through the border colour any more: a
    // `pinColorOverride` used to swallow the focused accent, leaving a
    // 1px width delta as the only cue. The border stays override/line
    // and `PinChrome` paints a dedicated offset focus ring in
    // `tokens.focusRing` instead.
    final borderColor = _parsePinColor(pinColorOverride) ?? tokens.line;
    final toolkitLabel = toolkit ?? '';
    final kind = pinKind;
    final chrome = kind == null ? null : pinKindChrome(kind);
    final bodyContent = bodyOverride == null
        ? PinBody(
            tokens: tokens,
            label: toolLabel,
            description: toolDescription,
            outputFields: outputFields,
            outputResult: outputResult,
            truncated: outputTruncated,
          )
        // Embed bodies (form field + live webview) mark their
        // subtree so the board yields plain-letter keys to the
        // embed instead of resolving them into board commands.
        : EmbedBodyFocusScope(child: bodyOverride!);
    // An interactive inline body (#50 generic inline form, embed tiles)
    // owns its own children — input fields, Run button, output rows —
    // so its descendant semantics must survive instead of collapsing
    // into the Pin's single summary node. When they do, the summary
    // `value`/`liveRegion` below is suppressed so the exposed child
    // result is never announced twice.
    final exposesInteractiveBodySemantics =
        bodyOverride != null && exposeBodySemantics;
    final semanticsValue = exposesInteractiveBodySemantics
        ? ''
        : _semanticsValue(ref);
    return MouseRegion(
      cursor: cursor,
      child: Semantics(
        container: exposesInteractiveBodySemantics,
        explicitChildNodes: exposesInteractiveBodySemantics,
        label: _semanticsLabel(ref),
        value: semanticsValue.isEmpty ? null : semanticsValue,
        hint: onTap == null ? null : t(ref, 'a11y.pin.hint_run'),
        button: onTap != null,
        enabled: onTap != null,
        focusable: true,
        // Announce inline run results as they land: the value string
        // carries running/OK/ERROR/stale, so flipping any of those (a
        // dispatch starting, a result arriving, a poll going stale)
        // re-reads the node to assistive technology — the inline-first
        // board never shows a modal the screen reader could latch onto.
        liveRegion: semanticsValue.isNotEmpty,
        onTap: onTap == null ? null : () => onTap!(placement),
        // Context-menu affordances (right-click / long-press) are
        // unreachable for switch-access and screen-reader users; expose
        // the same open / edit-color / unpin trio as custom actions.
        customSemanticsActions: <CustomSemanticsAction, VoidCallback>{
          if (onOpenModal != null)
            CustomSemanticsAction(label: t(ref, 'pin.menu.open')): () =>
                onOpenModal!(placement),
          if (onEditColor != null)
            CustomSemanticsAction(label: t(ref, 'pin.menu.edit_color')): () =>
                onEditColor!(placement),
          if (_hasSpanOverride)
            CustomSemanticsAction(label: t(ref, 'pin.menu.reset_size')): () =>
                unawaited(_resetSize(ref)),
          CustomSemanticsAction(label: t(ref, 'pin.menu.unpin')): () =>
              unawaited(_unpin(ref)),
        },
        child: ExcludeSemantics(
          excluding: !exposesInteractiveBodySemantics,
          child: GestureDetector(
            excludeFromSemantics: true,
            // Embed pins replace the body with a live WebViewPanel; the
            // webview must receive its own gestures, so an opaque
            // detector would swallow taps. `deferToChild` lets the
            // platform view layer take precedence while still letting
            // the chrome (header/footer) catch right-click for the
            // context menu when the body is text.
            behavior: bodyOverride == null
                ? HitTestBehavior.opaque
                : HitTestBehavior.deferToChild,
            onTap: onTap == null ? null : () => onTap!(placement),
            // Right-click (mouse) and long-press (touch) both raise the same
            // per-pin context menu — the explicit "open modal / edit color /
            // unpin" affordance. Anchored at the pin frame level so a
            // `deferToChild` body (embed webview / memo field) keeps its own
            // pointer events untouched.
            onSecondaryTapDown: (details) =>
                _showContextMenu(context, ref, details.globalPosition),
            onLongPressStart: (details) =>
                _showContextMenu(context, ref, details.globalPosition),
            child: PinChrome(
              tokens: tokens,
              focused: focused,
              borderColor: borderColor,
              clipContent: bodyOverride == null,
              header: PinHeader(
                tokens: tokens,
                accentColor: accentColor,
                chrome: chrome,
                toolId: placement.toolId,
                toolkit: toolkitLabel,
                showMoveHandle: showMoveHandle,
                moveHandle: moveHandle,
              ),
              body: pinBodyChrome(
                variant: chrome?.variant,
                tokens: tokens,
                accentColor: accentColor,
                child: bodyContent,
              ),
              footer: PinFooter(
                tokens: tokens,
                kind: pinKind,
                invokerLabel: invokerLabel,
                restoredLabel: _restoredLabel(ref),
                stale: stale,
                running: running,
              ),
            ),
          ),
        ),
      ),
    );
  }

  /// Semantics label: tool name (label when the catalogue resolved it,
  /// canonical id otherwise) plus the pin kind. Localised via the
  /// `a11y.pin.label*` catalog keys.
  String _semanticsLabel(WidgetRef ref) {
    final displayLabel = toolLabel?.trim();
    final name = (displayLabel == null || displayLabel.isEmpty)
        ? placement.toolId
        : displayLabel;
    final kind = pinKind;
    if (kind == null) {
      return t(ref, 'a11y.pin.label_plain', {'name': name});
    }
    return t(ref, 'a11y.pin.label', {'name': name, 'kind': kind.label});
  }

  /// Semantics value: the latest inline run summarised as
  /// running / OK / ERROR (+ one-line preview) / restored / stale.
  /// Empty when the pin has nothing to report yet.
  String _semanticsValue(WidgetRef ref) {
    final parts = <String>[];
    if (running) parts.add(t(ref, 'a11y.pin.running'));
    final result = outputResult;
    if (result != null) {
      final preview = _firstLine(
        result.ok ? result.primaryOutputText : (result.errorMessage ?? ''),
      );
      if (result.ok) {
        parts.add(
          preview.isEmpty
              ? t(ref, 'a11y.pin.result_ok_empty')
              : t(ref, 'a11y.pin.result_ok', {'preview': preview}),
        );
      } else {
        parts.add(
          preview.isEmpty
              ? t(ref, 'a11y.pin.result_error_empty')
              : t(ref, 'a11y.pin.result_error', {'preview': preview}),
        );
      }
      // A restored (previous-session) result reads differently from a
      // fresh one: announce it plus the same "last run · N ago" label
      // the badge shows.
      final restoredLabel = _restoredLabel(ref);
      if (restoredLabel != null) {
        parts
          ..add(t(ref, 'a11y.pin.restored'))
          ..add(restoredLabel);
      }
    }
    if (stale) parts.add(t(ref, 'a11y.pin.stale'));
    return parts.join(' · ');
  }

  /// "last run · N ago" copy for a restored result, or `null` when the
  /// rendered result is session-fresh (or absent).
  String? _restoredLabel(WidgetRef ref) {
    final restoredAt = this.restoredAt;
    if (restoredAt == null || outputResult == null) return null;
    final label = lastRunAgoLabel(DateTime.now().difference(restoredAt));
    return t(ref, label.key, label.args);
  }
}

/// First non-empty line of a (possibly multi-line) result preview so a
/// screen reader gets a one-line summary, not a JSON dump.
String _firstLine(String text) {
  for (final line in text.split('\n')) {
    final trimmed = line.trim();
    if (trimmed.isNotEmpty) return trimmed;
  }
  return '';
}

Color? _parsePinColor(String? hexColor) {
  if (hexColor == null || hexColor.length < 7 || !hexColor.startsWith('#')) {
    return null;
  }
  final hexPart = hexColor.substring(1);
  final rgbValue = int.tryParse(hexPart, radix: 16);
  if (rgbValue == null) return null;
  return Color(rgbValue + 0xFF000000);
}
