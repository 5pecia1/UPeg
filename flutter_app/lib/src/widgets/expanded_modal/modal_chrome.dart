/// Header, footer, and pin action shared by the expanded modal shell.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/pegboard_mutations_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/expanded_modal/kind_badge.dart';

const String _sourcePrefixKey = 'modal.footer.source_prefix';
const String _toolkitPrefixKey = 'modal.footer.toolkit_prefix';
const String _invokerPrefixKey = 'modal.footer.invoker_prefix';

class ExpandedModalHeader extends ConsumerWidget {
  const ExpandedModalHeader({
    required this.tokens,
    required this.tool,
    super.key,
  });

  final UpegTokens tokens;
  final ToolDto tool;

  @override
  Widget build(BuildContext context, WidgetRef ref) => Container(
    padding: const EdgeInsets.fromLTRB(16, 14, 16, 14),
    decoration: BoxDecoration(
      border: Border(bottom: BorderSide(color: tokens.lineSoft)),
    ),
    child: Row(
      children: [
        Icon(iconForTool(tool), color: tokens.accent, size: 14),
        const SizedBox(width: 10),
        Text(
          tool.label,
          style: TextStyle(
            fontFamily: upegMonoFontFamily,
            fontFamilyFallback: upegMonoFontFamilyFallback,
            fontSize: 13,
            color: tokens.fg,
            fontWeight: FontWeight.w500,
          ),
        ),
        if (tool.label != tool.id) ...[
          const SizedBox(width: 8),
          Flexible(
            child: Text(
              tool.id,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                fontFamily: upegMonoFontFamily,
                fontFamilyFallback: upegMonoFontFamilyFallback,
                fontSize: 10,
                color: tokens.fg4,
                letterSpacing: 0.4,
              ),
            ),
          ),
        ],
        const SizedBox(width: 8),
        KindBadge(pinKind: tool.pinKind),
        const Spacer(),
        OutlinedButton(
          key: const Key('expanded-modal-close-btn'),
          style: OutlinedButton.styleFrom(
            foregroundColor: tokens.fg2,
            side: BorderSide(color: tokens.line),
            padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(UpegSizing.radius1),
            ),
            minimumSize: const Size(0, 28),
            textStyle: const TextStyle(
              fontSize: 11,
              fontWeight: FontWeight.w500,
            ),
          ),
          onPressed: () => Navigator.of(context).pop(),
          child: Text(t(ref, 'modal.header.close')),
        ),
      ],
    ),
  );
}

class ExpandedModalFooter extends ConsumerWidget {
  const ExpandedModalFooter({
    required this.tokens,
    required this.tool,
    super.key,
  });

  final UpegTokens tokens;
  final ToolDto tool;

  @override
  Widget build(BuildContext context, WidgetRef ref) => Container(
    padding: const EdgeInsets.fromLTRB(16, 10, 16, 10),
    decoration: BoxDecoration(
      border: Border(top: BorderSide(color: tokens.lineSoft)),
    ),
    child: Wrap(
      spacing: 14,
      runSpacing: 4,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [
        RichText(
          text: TextSpan(
            children: [
              TextSpan(
                text: t(ref, _sourcePrefixKey),
                style: TextStyle(color: tokens.fg4, fontSize: 10),
              ),
              TextSpan(
                text: sourceDtoLabel(tool.source),
                style: TextStyle(color: tokens.fg3, fontSize: 10),
              ),
            ],
          ),
        ),
        Text(
          '${t(ref, _toolkitPrefixKey)}${tool.toolkit}',
          style: TextStyle(color: tokens.fg4, fontSize: 10),
        ),
        Text(
          '${t(ref, _invokerPrefixKey)}${invokerDtoLabel(tool.invoker)}',
          style: TextStyle(color: tokens.fg4, fontSize: 10),
        ),
        TextButton(
          key: const Key('expanded-modal-pin-btn'),
          style: TextButton.styleFrom(
            foregroundColor: tokens.accent,
            padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
            minimumSize: const Size(0, 24),
            textStyle: const TextStyle(
              fontSize: 11,
              fontWeight: FontWeight.w500,
            ),
          ),
          onPressed: () =>
              pinToolFromModal(context: context, ref: ref, tool: tool),
          child: Text(t(ref, 'desktop.tab.add_tool')),
        ),
      ],
    ),
  );
}

Future<void> pinToolFromModal({
  required BuildContext context,
  required WidgetRef ref,
  required ToolDto tool,
}) async {
  final messenger = ScaffoldMessenger.of(context);
  final boardKey = ref.read(currentBoardKeyProvider);
  if (boardKey == null) {
    messenger.showSnackBar(
      SnackBar(content: Text(tRead(ref, 'modal.pin.no_active_board'))),
    );
    return;
  }
  try {
    await ref
        .read(pegboardMutationsProvider)
        .pin(boardKey, ToolId.parse(tool.id));
  } on Object catch (error) {
    messenger.showSnackBar(
      SnackBar(
        content: Text(tRead(ref, 'modal.pin.failed', {'msg': '$error'})),
      ),
    );
    return;
  }
  messenger.showSnackBar(
    SnackBar(
      content: Text(
        tRead(ref, 'modal.pin.success', {
          'tool_id': tool.id,
          'board': '$boardKey',
        }),
      ),
    ),
  );
}
