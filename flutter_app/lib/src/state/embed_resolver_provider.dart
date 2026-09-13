/// FRB embed URL resolver — Riverpod-overridable seam.
///
/// Lives in `state/` rather than the consumers (`board_canvas`,
/// `embed_page`) because both surfaces need the same provider override
/// to swap in a fake during widget tests.
///
/// `resolveEmbedUrl` is a sync FRB call (returns `EmbedResolutionDto?`),
/// safe to invoke from `build` for pin rendering.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/embed.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

typedef ResolveEmbedFn =
    EmbedResolutionDto? Function({
      required ToolId toolId,
      required ToolArgs args,
    });

EmbedResolutionDto? _resolveEmbedBridge({
  required ToolId toolId,
  required ToolArgs args,
}) {
  return resolveEmbedUrl(toolId: toolId.value, argsJson: args.encodeJson());
}

final resolveEmbedFnProvider = Provider<ResolveEmbedFn>(
  (ref) => _resolveEmbedBridge,
);
