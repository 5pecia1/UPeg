/// Pure popup activation routing.
///
/// The popup is a quick launcher: a tool that can dispatch with no
/// input runs *inside* the popup and renders its result inline; only
/// tools that genuinely need the full surface (a form, an embed
/// webview, or a board-side effect like memo-create) switch the window
/// to the full dashboard.
///
/// [decidePopupActivationRoute] is a pure function over the shared
/// `pinActivationFor` verdict plus typed tool metadata — no Riverpod,
/// no side effects — so the inline-vs-full policy is unit-testable in
/// isolation (same seam philosophy as `pin_activation_provider.dart`).
library;

import 'package:upeg/src/features/memos/memo_roles.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pin_activation.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/tool_roles.dart';

/// Where a popup activation should go.
sealed class PopupActivationRoute {
  const PopupActivationRoute(this.toolId);

  /// The tool the route applies to.
  final ToolId toolId;
}

/// Run the tool inside the popup and render the outcome inline.
final class PopupRunInline extends PopupActivationRoute {
  const PopupRunInline(super.toolId);
}

/// Switch to the full dashboard and queue the activation there
/// (form-backed modal, embed page, or board-side action).
final class PopupOpenFull extends PopupActivationRoute {
  const PopupOpenFull(super.toolId);
}

/// Map the shared activation verdict to a popup route.
///
/// - `DispatchImmediate` → [PopupRunInline], unless the tool metadata
///   marks it as a board-side action (memo-create) or an unconfigured
///   live-http pin — those have no meaningful headless dispatch, so the
///   full surface handles them exactly as before.
/// - `OpenModal` (form needed) and `OpenEmbed` → [PopupOpenFull].
///
/// [tool] may be `null` when the catalog has not resolved yet; an
/// immediate-dispatch verdict still runs inline in that case because
/// the dispatcher reports unknown tools as a canonical error result.
PopupActivationRoute decidePopupActivationRoute({
  required PinActivationDto activation,
  required ToolDto? tool,
}) {
  switch (activation) {
    case PinActivationDto_DispatchImmediate(:final toolId):
      final parsed = ToolId.parse(toolId);
      final needsFullSurface =
          tool != null &&
          (isMemoCreateAction(tool) || toolNeedsProviderConfig(tool));
      return needsFullSurface ? PopupOpenFull(parsed) : PopupRunInline(parsed);
    case PinActivationDto_OpenModal(:final toolId):
      return PopupOpenFull(ToolId.parse(toolId));
    case PinActivationDto_OpenEmbed(:final toolId):
      return PopupOpenFull(ToolId.parse(toolId));
  }
}
