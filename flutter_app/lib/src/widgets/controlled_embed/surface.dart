/// Full-screen fallback surface for `PinKind::ControlledEmbed` tools
/// reached through paths that bypass the inline pin tile (palette
/// hits, deep links, pending activations from popup).
///
/// The pin tile (`ControlledEmbedTile`) is the canonical UX — clicking
/// a pin on the board never opens this page. But when the user opens
/// a ControlledEmbed tool from somewhere other than its pin tile, the
/// standard `ExpandedModalPage` can't run the pipeline (it dispatches
/// through `dispatchTool` which hits the headless backend — disabled
/// in GUI builds). This page wraps the same `ControlledEmbedTile`
/// in a Scaffold so the inline experience works full-screen too.
library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/embed.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/embed_resolver_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/controlled_embed/tile.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

class ControlledEmbedSurface extends ConsumerWidget {
  const ControlledEmbedSurface({required this.tool, super.key});

  final ToolDto tool;

  static Future<void> open(BuildContext context, ToolDto tool) {
    return Navigator.of(context).push<void>(
      MaterialPageRoute<void>(
        builder: (_) => ControlledEmbedSurface(tool: tool),
      ),
    );
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = context.upeg;
    final resolve = ref.read(resolveEmbedFnProvider);
    final EmbedResolutionDto? resolution = resolve(
      toolId: ToolId.parse(tool.id),
      args: ToolArgs.empty,
    );

    return Scaffold(
      backgroundColor: tokens.bg,
      body: Shortcuts(
        shortcuts: const <ShortcutActivator, Intent>{
          SingleActivator(LogicalKeyboardKey.escape): _CloseIntent(),
        },
        child: Actions(
          actions: <Type, Action<Intent>>{
            _CloseIntent: CallbackAction<_CloseIntent>(
              onInvoke: (_) {
                Navigator.of(context).pop();
                return null;
              },
            ),
          },
          child: Focus(
            autofocus: true,
            child: SafeArea(
              child: Column(
                children: [
                  _Header(
                    tool: tool,
                    tokens: tokens,
                    onClose: () => Navigator.of(context).pop(),
                  ),
                  Expanded(
                    child: resolution == null
                        ? Center(
                            child: Text(
                              t(ref, 'embed.url_unavailable', {
                                'tool_id': tool.id,
                              }),
                              style: TextStyle(color: tokens.fg3),
                            ),
                          )
                        : Padding(
                            padding: const EdgeInsets.all(16),
                            // The same tile widget the pin uses — the
                            // outer Scaffold provides the room the
                            // 118 px pin body can't.
                            child: ControlledEmbedTile(
                              tool: tool,
                              resolution: resolution,
                            ),
                          ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _CloseIntent extends Intent {
  const _CloseIntent();
}

class _Header extends ConsumerWidget {
  const _Header({
    required this.tool,
    required this.tokens,
    required this.onClose,
  });
  final ToolDto tool;
  final UpegTokens tokens;
  final VoidCallback onClose;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Container(
      padding: const EdgeInsets.fromLTRB(12, 8, 12, 8),
      decoration: BoxDecoration(
        color: tokens.surface,
        border: Border(bottom: BorderSide(color: tokens.lineSoft)),
      ),
      child: Row(
        children: [
          Text(
            tool.toolkit.toUpperCase(),
            style: TextStyle(
              fontFamily: upegMonoFontFamily,
              fontFamilyFallback: upegMonoFontFamilyFallback,
              fontSize: 10,
              color: tokens.fg4,
              letterSpacing: 0.4,
            ),
          ),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              tool.label,
              style: TextStyle(color: tokens.fg, fontSize: 13),
              overflow: TextOverflow.ellipsis,
            ),
          ),
          IconButton(
            key: const Key('controlled-embed-surface-close'),
            tooltip: t(ref, 'modal.header.close'),
            onPressed: onClose,
            icon: const Icon(Icons.close),
          ),
        ],
      ),
    );
  }
}
