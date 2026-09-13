/// Full-screen webview page for `PinKind::Embed` tools.
///
/// Replaces the embed branch that previously lived inside
/// `ExpandedModalPage`. No Run/Form/Output chrome — the pin IS the
/// browser. SoC: this page owns layout (thin top strip + WebViewPanel)
/// only; platform branching is inside `WebViewPanel`.
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
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';
import 'package:upeg/src/widgets/expanded_modal/webview_panel.dart';
import 'package:url_launcher/url_launcher.dart';

class EmbedPage extends ConsumerStatefulWidget {
  const EmbedPage({required this.tool, this.debugTargetOverride, super.key});

  final ToolDto tool;

  /// Forwarded to `WebViewPanel` so widget tests can pin the platform
  /// branch without depending on the actual build target.
  final WebViewTarget? debugTargetOverride;

  static Future<void> open(BuildContext context, ToolDto tool) {
    return Navigator.of(context).push<void>(
      MaterialPageRoute<void>(builder: (_) => EmbedPage(tool: tool)),
    );
  }

  @override
  ConsumerState<EmbedPage> createState() => _EmbedPageState();
}

class _EmbedPageState extends ConsumerState<EmbedPage> {
  EmbedResolutionDto? _resolution;

  @override
  void initState() {
    super.initState();
    final resolve = ref.read(resolveEmbedFnProvider);
    _resolution = resolve(
      toolId: ToolId.parse(widget.tool.id),
      args: ToolArgs.empty,
    );
  }

  Future<void> _openExternally() async {
    final url = _resolution?.url;
    if (url == null) return;
    final uri = Uri.tryParse(url);
    if (uri == null) return;
    await launchUrl(uri, mode: LaunchMode.externalApplication);
  }

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    final resolution = _resolution;
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
                  _EmbedHeader(
                    tool: widget.tool,
                    onClose: () => Navigator.of(context).pop(),
                    onOpenExternally: _openExternally,
                  ),
                  Expanded(
                    child: resolution == null
                        ? Center(
                            child: Text(
                              t(ref, 'embed.url_unavailable', {
                                'tool_id': widget.tool.id,
                              }),
                              style: TextStyle(color: tokens.fg3),
                            ),
                          )
                        : WebViewPanel(
                            resolution: resolution,
                            debugTargetOverride: widget.debugTargetOverride,
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

class _EmbedHeader extends ConsumerWidget {
  const _EmbedHeader({
    required this.tool,
    required this.onClose,
    required this.onOpenExternally,
  });

  final ToolDto tool;
  final VoidCallback onClose;
  final VoidCallback onOpenExternally;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = context.upeg;
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
          TextButton(
            key: const Key('embed-page-open-externally'),
            onPressed: onOpenExternally,
            child: Text(t(ref, 'embed.open_externally')),
          ),
          IconButton(
            key: const Key('embed-page-close'),
            tooltip: t(ref, 'modal.header.close'),
            onPressed: onClose,
            icon: const Icon(Icons.close),
          ),
        ],
      ),
    );
  }
}
