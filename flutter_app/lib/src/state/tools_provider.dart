/// Tool catalogue cached for the lifetime of the provider scope.
///
/// `listTools()` is cheap, but caching the result lets Pin look up a
/// label by tool id without re-crossing the FFI boundary on every paint.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart' show toolsForTag;
import 'package:upeg/src/rust/api/tools.dart';

typedef ToolsLoader = List<ToolDto> Function();

final toolsLoaderProvider = Provider<ToolsLoader>(
  (ref) =>
      () => listTools(),
);

final toolsProvider = FutureProvider<List<ToolDto>>((ref) async {
  final load = ref.watch(toolsLoaderProvider);
  return load();
});

/// Convenience: tool id → ToolDto. Returns `null` for unknown ids so the
/// caller can decide whether to render a placeholder.
final toolByIdProvider = Provider.family<ToolDto?, ToolId>((ref, id) {
  final tools = ref.watch(toolsProvider).value;
  if (tools == null) return null;
  for (final tool in tools) {
    if (tool.id == id.value) return tool;
  }
  return null;
});

// ─── Tag-filtered tool list (Batch Q4, inventory row G04) ─────────────

/// Synchronous loader for `tools_for_tag` — wraps the FRB
/// `toolsForTag` call so widget tests can substitute an in-memory
/// list without crossing the FFI boundary.
typedef ToolsForTagLoader = List<ToolDto> Function(String tag);

final toolsForTagLoaderProvider = Provider<ToolsForTagLoader>(
  (ref) =>
      (tag) => toolsForTag(tag: tag),
);

/// Family of `List<ToolDto>` keyed by tag: the tool list for a single tag.
/// Backed by [toolsForTagLoaderProvider] so widget tests stay
/// dylib-free. The palette consumes this to restrict its search hits
/// when a non-`all` tag is selected.
final toolsForTagProvider = Provider.family<List<ToolDto>, String>((ref, tag) {
  final load = ref.watch(toolsForTagLoaderProvider);
  return load(tag);
});
