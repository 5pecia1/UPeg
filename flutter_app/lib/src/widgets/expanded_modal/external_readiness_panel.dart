/// Shared readiness presentation for External tools.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/rust/api/readiness.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/external_readiness_provider.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/external_readiness_guidance.dart';

const Key externalReadinessPanelKey = Key('external-readiness-panel');
const Key externalReadinessRetryKey = Key('external-readiness-retry');
const Key externalReadinessBadgeKey = Key('external-readiness-badge');
const Key externalReadinessDialogKey = Key('external-readiness-dialog');

bool externalReadinessAllowsRun(
  AsyncValue<ExternalReadinessInspection> inspection,
) => switch (inspection) {
  AsyncData(value: ExternalReadinessNotApplicable()) => true,
  AsyncData(value: ExternalReadinessInspected(:final blocksRun)) => !blocksRun,
  _ => false,
};

class ExternalReadinessPanel extends ConsumerWidget {
  const ExternalReadinessPanel({required this.tool, super.key});

  final ToolDto tool;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    if (tool.invoker != InvokerDto.external_) return const SizedBox.shrink();
    final target = readinessTargetFor(
      tool,
      boardKey: ref.watch(currentBoardKeyProvider),
    );
    return Padding(
      key: externalReadinessPanelKey,
      padding: const EdgeInsets.only(bottom: 12),
      child: ExternalReadinessInspectionView(
        inspection: ref.watch(externalReadinessProvider(target)),
        onRecheck: () => recheckExternalReadiness(ref, target),
      ),
    );
  }
}

class ExternalReadinessInspectionView extends ConsumerWidget {
  const ExternalReadinessInspectionView({
    required this.inspection,
    required this.onRecheck,
    this.showReady = false,
    super.key,
  });

  final AsyncValue<ExternalReadinessInspection> inspection;
  final VoidCallback onRecheck;
  final bool showReady;

  @override
  Widget build(BuildContext context, WidgetRef ref) => inspection.when(
    data: (value) => switch (value) {
      ExternalReadinessNotApplicable() => const SizedBox.shrink(),
      ExternalReadinessHostUnpaired() => _StatusLine(
        icon: Icons.link_off,
        text: t(ref, 'readiness.host_unpaired'),
      ),
      ExternalReadinessInspected(:final readiness)
          when readiness.status == ExternalReadinessStatusDto.ready =>
        showReady
            ? _StatusLine(
                icon: Icons.check_circle_outline,
                text: t(ref, 'readiness.ready_on_platform', {
                  'platform': readiness.platform,
                }),
                action: TextButton(
                  key: externalReadinessRecheckKey,
                  onPressed: onRecheck,
                  child: Text(t(ref, 'readiness.recheck')),
                ),
              )
            : const SizedBox.shrink(),
      ExternalReadinessInspected(:final readiness) => ExternalReadinessGuidance(
        readiness: readiness,
        onRecheck: onRecheck,
      ),
    },
    loading: () => _StatusLine(
      icon: Icons.hourglass_top,
      text: t(ref, 'readiness.checking'),
    ),
    error: (error, _) => _StatusLine(
      icon: Icons.error_outline,
      text: t(ref, externalReadinessErrorKey(error)),
      action: TextButton(
        key: externalReadinessRetryKey,
        onPressed: onRecheck,
        child: Text(t(ref, 'readiness.recheck')),
      ),
    ),
  );
}

class ExternalReadinessBadge extends ConsumerWidget {
  const ExternalReadinessBadge({
    required this.tool,
    required this.target,
    super.key,
  });

  final ToolDto tool;
  final ExternalReadinessTarget target;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = context.upeg;
    return Center(
      child: OutlinedButton.icon(
        key: externalReadinessBadgeKey,
        onPressed: () => showDialog<void>(
          context: context,
          builder: (_) => ExternalReadinessDialog(tool: tool, target: target),
        ),
        icon: Icon(Icons.build_circle_outlined, size: 14, color: tokens.warn),
        label: Text(
          t(ref, 'readiness.setup_required'),
          overflow: TextOverflow.ellipsis,
        ),
      ),
    );
  }
}

class ExternalReadinessStatusBadge extends ConsumerWidget {
  const ExternalReadinessStatusBadge({
    required this.labelKey,
    this.onRecheck,
    super.key,
  });

  final String labelKey;
  final VoidCallback? onRecheck;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = context.upeg;
    return Center(
      child: TextButton.icon(
        onPressed: onRecheck,
        icon: Icon(Icons.info_outline, size: 14, color: tokens.fg3),
        label: Text(
          t(ref, labelKey),
          maxLines: 2,
          overflow: TextOverflow.ellipsis,
          textAlign: TextAlign.center,
        ),
      ),
    );
  }
}

class ExternalReadinessDialog extends ConsumerWidget {
  const ExternalReadinessDialog({
    required this.tool,
    required this.target,
    super.key,
  });

  final ToolDto tool;
  final ExternalReadinessTarget target;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return AlertDialog(
      key: externalReadinessDialogKey,
      title: Text(t(ref, 'readiness.setup_title', {'tool': tool.label})),
      content: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 520),
        child: ExternalReadinessInspectionView(
          inspection: ref.watch(externalReadinessProvider(target)),
          onRecheck: () => recheckExternalReadiness(ref, target),
          showReady: true,
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

class _StatusLine extends StatelessWidget {
  const _StatusLine({required this.icon, required this.text, this.action});

  final IconData icon;
  final String text;
  final Widget? action;

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    final trailing = switch (action) {
      final Widget action => <Widget>[action],
      null => const <Widget>[],
    };
    return Row(
      children: [
        Icon(icon, size: 15, color: tokens.fg3),
        const SizedBox(width: 6),
        Expanded(
          child: Text(text, style: TextStyle(color: tokens.fg3, fontSize: 11)),
        ),
        ...trailing,
      ],
    );
  }
}

String externalReadinessErrorKey(Object error) => switch (error) {
  HostReadinessUnauthorized() => 'host_attach.notice.unauthorized_hint',
  HostReadinessMalformed() => 'readiness.host_malformed',
  _ => 'readiness.host_unavailable',
};
