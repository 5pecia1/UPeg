/// The pin layer of the board canvas: one placement rendered as a live
/// [Pin], wired to the drag sources, the drop-target highlight, the
/// drag autoscroll, and the SE-corner resize handle.
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/features/host_attach/host_attach_config_provider.dart';
import 'package:upeg/src/features/host_attach/host_attach_dispatch_provider.dart';
import 'package:upeg/src/features/memos/memo_pin_body.dart';
import 'package:upeg/src/features/memos/memo_roles.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/pegboard_units.dart';
import 'package:upeg/src/rust/api/capability.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/capability_provider.dart';
import 'package:upeg/src/state/embed_resolver_provider.dart';
import 'package:upeg/src/state/focused_pin_provider.dart';
import 'package:upeg/src/state/last_outcome_provider.dart';
import 'package:upeg/src/state/live_outcome_provider.dart';
import 'package:upeg/src/state/running_tools_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/tool_roles.dart';
import 'package:upeg/src/widgets/board_canvas/board_grid_metrics.dart';
import 'package:upeg/src/widgets/board_canvas/drop_targets.dart';
import 'package:upeg/src/widgets/board_canvas/pin_drag.dart';
import 'package:upeg/src/widgets/board_canvas/resize_affordance.dart';
import 'package:upeg/src/widgets/controlled_embed/tile.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';
import 'package:upeg/src/widgets/expanded_modal/webview_panel.dart';
import 'package:upeg/src/widgets/host_attach_notice_body.dart';
import 'package:upeg/src/widgets/inline/generic_inline_pin_body.dart';
import 'package:upeg/src/widgets/pin.dart';
import 'package:upeg/src/widgets/provider_not_configured_body.dart';
import 'package:upeg/src/widgets/surface_unsupported_body.dart';

/// Threshold in logical pixels from the top/bottom edge of the vertical
/// scroll viewport at which drag autoscroll activates.
const double _dragAutoscrollEdgeThreshold = 80.0;

/// Base scroll offset applied when the pointer is inside the autoscroll
/// edge zone.
const double _dragAutoscrollBaseDelta = 20.0;

/// Rows ahead of the pointer during a continuous drag that the grid
/// should expand to. Makes expansion **pointer-leading** so the user
/// never has to "step back, expand, step back, expand" — the drop
/// targets exist before the pointer arrives.
const int _dragExpansionLookaheadRows = 6;

/// Opacity applied to the translucent drag-feedback copy of a pin.
const double _dragFeedbackOpacity = 0.75;

/// Wraps [Pin] with a Riverpod lookup for the tool label AND the modeless
/// drag affordance.
///
/// Modeless contract: the move handle is ALWAYS shown and a pin is ALWAYS
/// draggable. Two drag surfaces exist so the embed body keeps its pointer
/// events:
///   * plain pins (text body): the WHOLE surface is the drag source — a
///     tap still runs the tool because tap and the immediate drag
///     recognizer resolve in the gesture arena,
///   * gesture-owning bodies (embed / controlled-embed / memo): drag
///     originates ONLY from the injected move handle so the live body
///     (webview / text field) keeps receiving taps/scrolls untouched.
class PinWithLabel extends ConsumerWidget {
  const PinWithLabel({
    required this.placement,
    required this.boardKey,
    required this.columns,
    required this.onTap,
    required this.gridKey,
    required this.onDragFinished,
    this.onEditColor,
    this.onOpenModal,
    this.onDragRowUpdate,
    this.highlightKey,
    this.verticalScrollController,
    super.key,
  });

  final PlacementDto placement;
  final BoardKey boardKey;

  /// Board column count — bounds the drop-target highlight so it never
  /// renders for a pointer that has wandered past the last column.
  final int columns;

  final PinTapCallback onTap;
  final PinTapCallback? onEditColor;
  final PinTapCallback? onOpenModal;
  final GlobalKey gridKey;
  final VoidCallback onDragFinished;

  /// Called on each pointer move during drag. Receives the computed grid
  /// row under the dragged pointer so the parent can trigger dynamic
  /// expansion.
  final ValueChanged<int>? onDragRowUpdate;

  /// Imperative handle onto the drop-target highlight layer. Updated
  /// DIRECTLY from `_onDragUpdate` (sharing its dispatch with
  /// [onDragRowUpdate] deliberately — see `DropHighlightLayer`'s class
  /// doc) instead of via a lifted-up callback, so a hover-only update
  /// never rebuilds anything outside that one small widget.
  final GlobalKey<DropHighlightLayerState>? highlightKey;

  /// Scroll controller for the vertical scroll view. When provided and
  /// the drag pointer approaches the viewport edge, autoscroll is triggered
  /// so newly-expanded drop targets become reachable without lifting.
  final ScrollController? verticalScrollController;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = context.upeg;
    final toolId = ToolId.parse(placement.toolId);
    final ToolDto? tool = ref.watch(toolByIdProvider(toolId));
    final focused = ref.watch(focusedPinProvider) == toolId;
    // Zero-input Inline/Function tools are candidates too: `GenericInlinePinBody`
    // renders a bare Run affordance when a tool declares no input fields, so
    // the input-field count no longer gates eligibility for the inline body.
    final genericInlineCandidate =
        tool != null &&
        tool.pinKind == PinKindDto.inline &&
        tool.invoker == InvokerDto.function;
    final genericInlineBodyEligible =
        genericInlineCandidate &&
        (!ref.watch(isWasmRuntimeProvider) ||
            ref.read(toolCapabilityFnProvider)(toolId.value)
                is! DispatchCapabilityDto_Unsupported);

    // Live polling: Timer pins without an input-aware inline contract watch
    // liveOutcomeProvider and feed structured outcome values into the Pin
    // body. Input-aware candidates never use that empty-args poller, even
    // when this runtime cannot render their form. Non-Timer sources feed the
    // last user-driven dispatch outcome (inline-first activation, issue 7).
    CanonicalToolResult? outputResult;
    var stale = false;
    DateTime? restoredAt;
    var outputTruncated = false;
    if (tool != null &&
        tool.source is SourceDto_Timer &&
        !genericInlineCandidate) {
      final liveState = ref.watch(liveOutcomeProvider(toolId));
      switch (liveState) {
        case LiveOutcomeFresh(:final outcome):
          outputResult = outcome;
        case LiveOutcomeStale(:final lastOutcome):
          outputResult = lastOutcome;
          stale = true;
        case LiveOutcomePending():
          break;
      }
    } else if (tool != null) {
      // Inline rendering keeps its ok-only contract; restored entries
      // additionally carry their recording timestamp so the pin shows
      // the "last run · N ago" badge instead of passing off a previous
      // session's result as fresh.
      final last = ref.watch(pinLastOutcomeProvider(toolId));
      if (last != null && last.result.ok) {
        outputResult = last.result;
        if (last case RestoredOutcome(:final updatedAt, :final truncated)) {
          restoredAt = updatedAt;
          outputTruncated = truncated;
        }
      }
    }

    // Inline bodyOverride pins: replace the text body with a widget that
    // IS the tool. `bodyOwnsGesture` is true when that widget must own
    // the pointer (embed webviews, the memo text field) so tap-to-modal
    // is disabled; it stays false for the passive "needs setup" state, which
    // keeps the tap so the user still gets the clear message.
    //
    // Modeless: the live body is ALWAYS rendered — there is no edit-mode
    // static-chip fallback anymore.
    Widget? bodyOverride;
    var bodyOwnsGesture = false;
    // The generic inline body wants a split gesture: its input controls
    // capture their own taps (inline editing), and a tap anywhere else on
    // the pin opens the expanded modal. `deferToChild` (Pin) routes that
    // for us — so this pin keeps a live tap handler that opens the modal.
    var inlineTapOpensModal = false;
    if (tool != null) {
      if (isMemoNotepadTool(tool)) {
        // memo.scratch: an always-live inline notepad backed by the
        // memos store. No activation step — the text field is the tool.
        bodyOverride = const MemoPinBody();
        bodyOwnsGesture = true;
      } else if (toolNeedsProviderConfig(tool)) {
        // eth.gas-style: live external data, no configured provider.
        // The manifest-level credential name (when the loader registered
        // one) makes the copyable fix-it command concrete.
        bodyOverride = ProviderNotConfiguredBody(
          credentialName: tool.credentialName,
        );
      } else if (tool.pinKind == PinKindDto.embed ||
          tool.pinKind == PinKindDto.controlledEmbed) {
        // Both Passive Embed (naked WebView) and Controlled Embed (form +
        // WebView + Run + outputs) use the same slot; the live UI owns
        // the gesture surface.
        final resolveEmbed = ref.read(resolveEmbedFnProvider);
        final resolution = resolveEmbed(toolId: toolId, args: ToolArgs.empty);
        if (resolution != null) {
          bodyOverride = switch (tool.pinKind) {
            PinKindDto.embed => WebViewPanel(
              resolution: resolution,
              userAgent: kMobileUserAgent,
            ),
            PinKindDto.controlledEmbed => ControlledEmbedTile(
              tool: tool,
              resolution: resolution,
              // ControlledEmbed uses the same canonical output display rows
              // as normal pins; pass the board tool schema explicitly so
              // selector output ids resolve to user-facing labels inline.
              outputFields: tool.outputFields,
            ),
            _ => null,
          };
          bodyOwnsGesture = bodyOverride != null;
        }
      } else if (genericInlineBodyEligible) {
        // Generic inline body: an Inline-kind in-process function renders
        // its form + live result directly on the tile. The form adapts to
        // the tool's input signature (any field count/type, zero included:
        // no fields means a bare Run affordance) — no per-tool template.
        // Gated off for wasm-unsupported tools so the
        // honest capability notice / attach routing below still applies.
        bodyOverride = GenericInlinePinBody(
          tool: tool,
          pinKey: (boardKey, toolId),
        );
        bodyOwnsGesture = true;
        inlineTapOpensModal = true;
      }
    }

    // Honest surface capability (Rust decides once, surfaces obey): on the
    // wasm/PWA runtime some invokers (external/http/chain/llm/wasm) and
    // native-only Function tools physically cannot dispatch in-process.
    // Native surfaces support everything, so this is gated on the wasm
    // runtime and only applies to plain runnable pins (memo/embed/provider
    // bodies keep their own specialized rendering).
    //
    // When such a tool is attach-solvable AND a host is paired (Task B3),
    // route the pin through the daemon instead of showing only a notice:
    // tapping dispatches remotely and the canonical result renders inline
    // via `lastOutcomeProvider`, exactly like an in-process run. When no
    // host is paired (or the reason is native-only) fall back to the honest
    // "unsupported" notice, with a pairing nudge for attach-solvable reasons.
    var attachRouted = false;
    if (tool != null &&
        bodyOverride == null &&
        ref.watch(isWasmRuntimeProvider)) {
      final capability = ref.read(toolCapabilityFnProvider)(toolId.value);
      if (capability is DispatchCapabilityDto_Unsupported) {
        final reason = capability.reason;
        final attachable = hostAttachCanSolve(reason);
        final configured = ref.watch(hostAttachConfigProvider).isConfigured;
        if (attachable && configured) {
          attachRouted = true;
          final notice = hostAttachNoticeFor(
            ref.watch(pinAttachResultProvider(toolId)),
          );
          if (notice != null) {
            // A failed remote attempt: show the honest attach notice. The
            // pin stays tappable so the user can retry after fixing the
            // token / re-enabling the daemon's REST API.
            bodyOverride = HostAttachNoticeBody(notice: notice);
            bodyOwnsGesture = false;
          }
          // ok / not-yet-dispatched: keep the plain runnable body; the ok
          // result (recorded into lastOutcomeProvider) renders as output.
        } else {
          bodyOverride = SurfaceUnsupportedBody(
            reason: reason,
            showAttachHint: attachable,
          );
          bodyOwnsGesture = false;
        }
      }
    }

    // Gesture-owning bodyOverride pins don't activate on tap — the inline
    // widget owns the gesture surface. Attach-routed pins tap to dispatch
    // remotely; everyone else keeps the host-supplied tap = run.
    final PinTapCallback? tapHandler;
    if (inlineTapOpensModal) {
      // Tap outside the inline input controls opens the expanded modal
      // (seeded with the inline draft); taps on the fields edit in place.
      tapHandler = onOpenModal;
    } else if (bodyOwnsGesture) {
      tapHandler = null;
    } else if (attachRouted) {
      tapHandler = (_) => unawaited(
        ref
            .read(hostAttachDispatchProvider.notifier)
            .run(toolId, ToolArgs.empty),
      );
    } else {
      tapHandler = onTap;
    }
    final payload = PinDragPayload(toolId: toolId, boardKey: boardKey);

    // A cheap, text-body copy of the pin used as the translucent drag
    // feedback. Built without the live body so an embed pin never mounts a
    // second webview while dragging.
    final feedbackPin = Pin(
      placement: placement,
      toolLabel: tool?.label,
      toolDescription: tool?.description,
      toolkit: tool?.toolkit,
      pinKind: tool == null ? null : UpegPinKind.fromDto(tool.pinKind),
      invokerLabel: tool == null ? null : invokerDtoLabel(tool.invoker),
      outputFields: tool?.outputFields ?? const <OutputFieldDto>[],
      outputResult: outputResult,
      restoredAt: restoredAt,
      outputTruncated: outputTruncated,
      stale: stale,
      focused: focused,
      showMoveHandle: true,
      pinColorOverride: placement.color,
    );

    final pin = Pin(
      placement: placement,
      toolLabel: tool?.label,
      toolDescription: tool?.description,
      toolkit: tool?.toolkit,
      pinKind: tool == null ? null : UpegPinKind.fromDto(tool.pinKind),
      invokerLabel: tool == null ? null : invokerDtoLabel(tool.invoker),
      outputFields: tool?.outputFields ?? const <OutputFieldDto>[],
      outputResult: outputResult,
      restoredAt: restoredAt,
      outputTruncated: outputTruncated,
      stale: stale,
      running: ref.watch(toolIsRunningProvider(toolId)),
      focused: focused,
      showMoveHandle: true,
      // Gesture-owning bodies drag only from the injected handle; the pin
      // surface itself stays a plain click target so the webview/text
      // field keeps its pointer events.
      moveHandle: bodyOwnsGesture
          ? _buildDragHandle(payload, feedbackPin, tokens.accent)
          : null,
      cursor: bodyOwnsGesture
          ? SystemMouseCursors.click
          : SystemMouseCursors.grab,
      pinColorOverride: placement.color,
      onTap: tapHandler,
      onEditColor: onEditColor,
      onOpenModal: onOpenModal,
      bodyOverride: bodyOverride,
      exposeBodySemantics: bodyOwnsGesture,
    );

    final Widget interactive;
    if (bodyOwnsGesture) {
      // Embed/memo pin: the pin body is NOT a drag source; only the
      // handle (built above) is. Use the pin as-is.
      interactive = pin;
    } else {
      // Plain pin: the whole surface is the drag source.
      interactive = Draggable<PinDragPayload>(
        data: payload,
        // Capture the tool id BEFORE handing the payload off so an async
        // commit later cannot race a UI-driven state reset (see
        // feedback_dragend_race.md — the retired Dioxus surface hit the
        // same race).
        onDragCompleted: onDragFinished,
        onDraggableCanceled: (_, _) => onDragFinished(),
        onDragUpdate: _onDragUpdate,
        feedback: _dragFeedback(pin),
        // Replace the source slot with a hit-test-pass-through placeholder
        // while dragging so the DragTarget cells underneath receive the
        // hover/drop events. A translucent overlay would absorb the hit
        // test and silently swallow the drop.
        childWhenDragging: const SizedBox.expand(),
        child: pin,
      );
    }

    // SE-corner resize handle — always visible (same modeless policy as
    // the move handle) and stacked ABOVE the pin surface so it keeps
    // its pointer events even when the body owns gestures (embed
    // webview / memo field).
    final manifest =
        tool?.pegboardUnits.footprint ?? (cols: placement.w, rows: placement.h);
    return Stack(
      clipBehavior: Clip.none,
      // `expand` sizes the non-positioned pin surface to the slot
      // WITHOUT an extra `Positioned` wrapper, so existing
      // "grid Positioned is the Pin's ancestor" contracts stay intact.
      fit: StackFit.expand,
      children: [
        interactive,
        Positioned(
          right: resizeHandleInset,
          bottom: resizeHandleInset,
          child: ResizeHandle(
            placement: placement,
            boardKey: boardKey,
            maxCols: columns,
            manifestCols: manifest.cols,
            manifestRows: manifest.rows,
          ),
        ),
      ],
    );
  }

  /// Draggable move handle for gesture-owning (embed/memo) pins: dragging
  /// this small icon moves the pin while the live body underneath keeps
  /// every pointer event.
  Widget _buildDragHandle(
    PinDragPayload payload,
    Widget feedbackPin,
    Color color,
  ) {
    return Draggable<PinDragPayload>(
      data: payload,
      onDragCompleted: onDragFinished,
      onDraggableCanceled: (_, _) => onDragFinished(),
      onDragUpdate: _onDragUpdate,
      feedback: _dragFeedback(feedbackPin),
      child: MouseRegion(
        cursor: SystemMouseCursors.grab,
        child: Tooltip(
          message: pinMoveTooltip,
          child: Icon(
            Icons.drag_indicator,
            key: pinMoveHandleKey,
            size: 13,
            color: color,
          ),
        ),
      ),
    );
  }

  /// Sized, translucent copy of the pin used as drag feedback. The overlay
  /// sits inside an unconstrained Theater so the feedback widget must
  /// provide its own finite size (Pin chrome uses Expanded inside Column).
  Widget _dragFeedback(Widget child) {
    return Material(
      color: Colors.transparent,
      child: SizedBox(
        width: BoardGridMetrics.pixelW(placement.w),
        height: BoardGridMetrics.pixelH(placement.h),
        child: Opacity(opacity: _dragFeedbackOpacity, child: child),
      ),
    );
  }

  void _onDragUpdate(DragUpdateDetails details) {
    final RenderBox? box =
        gridKey.currentContext?.findRenderObject() as RenderBox?;
    if (box == null) return;
    // Use full global position for correct local offset (not
    // Offset(0, dy) which produces wrong results when the grid
    // is not aligned to the screen origin).
    final local = box.globalToLocal(details.globalPosition);
    final localX = local.dx;
    final localY = local.dy;
    final cellH = BoardGridMetrics.cellHeight;
    final gap = BoardGridMetrics.gap;
    final rowSize = cellH + gap;
    final colSize = BoardGridMetrics.cellWidth + gap;
    final pointerRow = (localY / rowSize).floor();
    final pointerCol = (localX / colSize).floor();
    // Expand pointer-leading: request rows ahead of the cursor so
    // drop targets exist before the pointer arrives, eliminating
    // the "step back, expand, step back, expand" pattern.
    onDragRowUpdate?.call(pointerRow + _dragExpansionLookaheadRows);

    // Drop-target hover highlight: the ACTUAL cell under the pointer
    // (no lookahead — a drop commits where the pointer IS, not where
    // it's headed). Shares this dispatch with the row-expansion call
    // above rather than a second DragTarget-driven one; updates the
    // highlight layer's OWN state directly (see `DropHighlightLayer`'s
    // class doc for why that isolation matters here).
    if (pointerCol >= 0 && pointerCol < columns && pointerRow >= 0) {
      highlightKey?.currentState?.updateHover(
        pointerCol,
        pointerRow,
        PinDragPayload(
          toolId: ToolId.parse(placement.toolId),
          boardKey: boardKey,
        ),
      );
    } else {
      highlightKey?.currentState?.clearHover();
    }

    // Autoscroll when the drag pointer approaches either vertical
    // viewport edge. Downward scrolling makes newly-expanded rows
    // reachable; upward scrolling keeps existing rows above reachable.
    final ctrl = verticalScrollController;
    if (ctrl != null && ctrl.hasClients) {
      // Use the scroll position's viewportDimension for the visible area.
      final viewportHeight = ctrl.position.viewportDimension;
      // localY is in the content coordinate system. Convert to viewport
      // coordinates by subtracting the current scroll offset.
      final pointerInViewport = localY - ctrl.offset;
      final pointerInView = pointerInViewport
          .clamp(0.0, viewportHeight)
          .toDouble();
      final distanceFromTop = pointerInView;
      final distanceFromBottom = viewportHeight - pointerInView;
      final maxExtent = ctrl.position.maxScrollExtent;
      if (maxExtent <= 0) return;

      if (distanceFromTop < _dragAutoscrollEdgeThreshold) {
        final progress = 1.0 - (distanceFromTop / _dragAutoscrollEdgeThreshold);
        final delta =
            _dragAutoscrollBaseDelta + progress * _dragAutoscrollBaseDelta;
        final targetOffset = (ctrl.offset - delta).clamp(0.0, maxExtent);
        if (targetOffset == ctrl.offset) return;
        ctrl.jumpTo(targetOffset);
      } else if (distanceFromBottom < _dragAutoscrollEdgeThreshold) {
        final progress =
            1.0 - (distanceFromBottom / _dragAutoscrollEdgeThreshold);
        final delta =
            _dragAutoscrollBaseDelta + progress * _dragAutoscrollBaseDelta;
        final targetOffset = (ctrl.offset + delta).clamp(0.0, maxExtent);
        if (targetOffset == ctrl.offset) return;
        ctrl.jumpTo(targetOffset);
      }
    }
  }
}
