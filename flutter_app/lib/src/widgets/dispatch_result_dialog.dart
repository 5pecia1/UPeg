/// Modal dialog that renders a [CanonicalToolResult] from `dispatchTool`.
///
/// Picks a green/red header from `outcome.ok` and surfaces canonical
/// output/error content.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';

class DispatchResultDialog extends ConsumerWidget {
  const DispatchResultDialog({
    required this.toolId,
    required this.outcome,
    super.key,
  });

  final ToolId toolId;
  final CanonicalToolResult outcome;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final theme = Theme.of(context);
    final headerColor = outcome.ok
        ? theme.colorScheme.primary
        : theme.colorScheme.error;
    final headerText = outcome.ok
        ? t(ref, 'modal.outcome.ok')
        : t(ref, 'modal.outcome.error');
    final outputRows = outcome.displayRows(const []);
    return AlertDialog(
      title: Row(
        children: [
          Expanded(
            child: Text(
              toolId.value,
              style: theme.textTheme.titleMedium,
              overflow: TextOverflow.ellipsis,
            ),
          ),
          const SizedBox(width: 8),
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
            decoration: BoxDecoration(
              color: headerColor,
              borderRadius: BorderRadius.circular(4),
            ),
            child: Text(
              headerText,
              style: theme.textTheme.labelSmall?.copyWith(
                color: theme.colorScheme.surface,
              ),
            ),
          ),
        ],
      ),
      content: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 600),
        child: SingleChildScrollView(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              if (outcome.errorMessage != null &&
                  outcome.errorMessage!.isNotEmpty) ...[
                Text(
                  '${t(ref, 'modal.outcome.error')}:',
                  style: theme.textTheme.labelMedium,
                ),
                const SizedBox(height: 4),
                SelectableText(
                  outcome.errorMessage!,
                  style: theme.textTheme.bodySmall,
                ),
                const SizedBox(height: 12),
              ],
              for (final row in outputRows) ...[
                _DialogOutputLabel(row: row, theme: theme),
                const SizedBox(height: 4),
                SelectableText(row.value, style: theme.textTheme.bodySmall),
                const SizedBox(height: 12),
              ],
              if (!outcome.hasDisplayContent)
                Text(
                  t(ref, 'modal.no_output'),
                  style: theme.textTheme.bodySmall?.copyWith(
                    fontStyle: FontStyle.italic,
                  ),
                ),
            ],
          ),
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(t(ref, 'settings.close')),
        ),
      ],
    );
  }
}

class _DialogOutputLabel extends ConsumerWidget {
  const _DialogOutputLabel({required this.row, required this.theme});

  final CanonicalOutputDisplayRow row;
  final ThemeData theme;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Text('${row.label}:', style: theme.textTheme.labelMedium),
        if (row.isPrimary) ...[
          const SizedBox(width: 6),
          Text(
            t(ref, 'modal.output.primary'),
            style: theme.textTheme.labelSmall,
          ),
        ],
      ],
    );
  }
}
