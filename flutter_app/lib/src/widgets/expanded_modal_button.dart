/// Tap glue that opens the [ExpandedModalPage] for a given tool.
///
/// Pages that need "tap a pin → open modal" wire this in instead of
/// re-implementing the navigator push. Pin keeps its `onTap`
/// callback shape (a [PlacementDto]); this helper translates the
/// placement to a [ToolDto] lookup before pushing.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/pages/expanded_modal_page.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/widgets/controlled_embed/surface.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

/// Result of resolving a placement/hit to a tool before opening the
/// modal. Exposed so callers can decide what to do when the tool isn't
/// in the catalogue yet (typically: show a snackbar).
sealed class ExpandedModalOpenResult {
  const ExpandedModalOpenResult();
}

class ExpandedModalOpened extends ExpandedModalOpenResult {
  const ExpandedModalOpened(this.tool);
  final ToolDto tool;
}

class ExpandedModalToolUnknown extends ExpandedModalOpenResult {
  const ExpandedModalToolUnknown(this.toolId);
  final ToolId toolId;
}

/// Open the modal for the placement's tool. Returns the resolution
/// result so the caller can react to the "unknown tool id" case.
Future<ExpandedModalOpenResult> openExpandedModalForPlacement(
  BuildContext context,
  WidgetRef ref,
  PlacementDto placement,
) async {
  return openExpandedModalForToolId(
    context,
    ref,
    ToolId.parse(placement.toolId),
  );
}

/// Open the modal for `toolId`. Used by both the placement-based path
/// (Pin tap) and the palette hit path, since `PaletteHit.id` is
/// already the canonical tool id.
Future<ExpandedModalOpenResult> openExpandedModalForToolId(
  BuildContext context,
  WidgetRef ref,
  ToolId toolId, {
  ToolArgs? initialInput,
}) async {
  var tool = ref.read(toolByIdProvider(toolId));
  if (tool == null) {
    final tools = await ref.read(toolsProvider.future);
    if (!context.mounted) return ExpandedModalToolUnknown(toolId);
    for (final candidate in tools) {
      if (candidate.id == toolId.value) {
        tool = candidate;
        break;
      }
    }
  }
  if (tool == null) {
    return ExpandedModalToolUnknown(toolId);
  }
  // ControlledEmbed pins are inline (bodyOverride) on the board.
  // Callers that reach this helper from a non-board surface
  // (palette / deep-link / pending activation) can't drive the
  // inline tile because no pin is mounted. Route them to a
  // full-screen surface that mounts the same tile widget in a
  // Scaffold so the form + Run + outputs experience still works.
  if (tool.pinKind == PinKindDto.controlledEmbed) {
    await ControlledEmbedSurface.open(context, tool);
    return ExpandedModalOpened(tool);
  }
  await ExpandedModalPage.open(context, tool, initialInput: initialInput);
  return ExpandedModalOpened(tool);
}
