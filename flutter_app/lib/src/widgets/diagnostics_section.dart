/// Generic diagnostics browser. It deliberately has no ecosystem-specific
/// rendering: the native report is the canonical source for every failure.
library;

import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/platform/file_picker_bridge.dart';
import 'package:upeg/src/state/diagnostics_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';

class DiagnosticsSection extends ConsumerWidget {
  const DiagnosticsSection({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final diagnostics = ref.watch(recentDiagnosticsProvider);
    return diagnostics.when(
      loading: () => const Center(child: CircularProgressIndicator()),
      error: (error, _) => Text(
        t(ref, 'diagnostics.load_failed', {'msg': '$error'}),
        style: TextStyle(color: context.upeg.warn, fontSize: 10),
      ),
      data: (items) {
        if (items.isEmpty) return Text(t(ref, 'diagnostics.empty'));
        return Column(
          children: [
            for (final item in items)
              Material(
                color: Colors.transparent,
                child: ListTile(
                  key: Key('diagnostic-${item.id}'),
                  dense: true,
                  contentPadding: EdgeInsets.zero,
                  title: Text(item.errorCode),
                  subtitle: Text(
                    '${item.source} · ${item.errorMessage}',
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                  ),
                  trailing: Text(item.status),
                  onTap: () => showDialog<void>(
                    context: context,
                    builder: (_) => _DiagnosticDetailDialog(id: item.id),
                  ),
                ),
              ),
          ],
        );
      },
    );
  }
}

/// Open only the report belonging to this result, even after another run fails.
class DiagnosticRunButton extends ConsumerWidget {
  const DiagnosticRunButton({required this.runId, super.key});
  final String runId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final report = ref.watch(_diagnosticRunProvider(runId)).asData?.value;
    if (report == null) return const SizedBox.shrink();
    return TextButton(
      key: Key('diagnostic-run-$runId'),
      onPressed: () => showDialog<void>(
        context: context,
        builder: (_) => _DiagnosticDetailDialog(id: report.id),
      ),
      child: Text(t(ref, 'diagnostics.detail_title')),
    );
  }
}

final _diagnosticRunProvider = FutureProvider.autoDispose
    .family<DiagnosticSummary?, String>(
      (ref, runId) =>
          diagnosticForRun(ref.watch(diagnosticsApiProvider), runId),
    );

class _DiagnosticDetailDialog extends ConsumerWidget {
  const _DiagnosticDetailDialog({required this.id});
  final String id;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final report = ref.watch(_diagnosticReportProvider(id));
    return AlertDialog(
      title: Text(t(ref, 'diagnostics.detail_title')),
      content: SizedBox(
        width: 640,
        child: report.when(
          loading: () => const Center(child: CircularProgressIndicator()),
          error: (error, _) => Text('$error'),
          data: (value) => value == null
              ? Text(t(ref, 'diagnostics.not_found'))
              : _DiagnosticReportBody(report: value),
        ),
      ),
      actions: [
        if (switch (report) {
              AsyncData(:final value) => value,
              _ => null,
            }
            case final value?) ...[
          TextButton(
            onPressed: () => _copy(context, ref, value),
            child: Text(t(ref, 'diagnostics.copy')),
          ),
          TextButton(
            onPressed: () => _save(context, ref, value.summary.id),
            child: Text(t(ref, 'diagnostics.save')),
          ),
        ],
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(t(ref, 'settings.close')),
        ),
      ],
    );
  }

  Future<void> _copy(
    BuildContext context,
    WidgetRef ref,
    DiagnosticReport report,
  ) async {
    final encoded = await ref
        .read(diagnosticsApiProvider)
        .export(report.summary.id, debug: false);
    await Clipboard.setData(ClipboardData(text: encoded));
    if (context.mounted) {
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(tRead(ref, 'diagnostics.copied'))));
    }
  }

  Future<void> _save(BuildContext context, WidgetRef ref, String id) async {
    final encoded = await ref
        .read(diagnosticsApiProvider)
        .export(id, debug: false);
    final saved = await ref
        .read(filePickerBridgeProvider)
        .saveBytes(
          name: 'upeg-diagnostic-$id.json',
          bytes: Uint8List.fromList(utf8.encode(encoded)),
          dialogTitle: tRead(ref, 'diagnostics.save_title'),
          mime: 'application/json',
        );
    if (saved && context.mounted) {
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(tRead(ref, 'diagnostics.saved'))));
    }
  }
}

final _diagnosticReportProvider =
    FutureProvider.family<DiagnosticReport?, String>(
      (ref, id) => ref.watch(diagnosticsApiProvider).show(id),
    );

class _DiagnosticReportBody extends StatelessWidget {
  const _DiagnosticReportBody({required this.report});
  final DiagnosticReport report;

  @override
  Widget build(BuildContext context) {
    final details = <String>[
      'id: ${report.summary.id}',
      'run: ${report.summary.runId}',
      'status: ${report.summary.status}',
      'source: ${report.summary.source}',
      'code: ${report.summary.errorCode}',
      'message: ${report.summary.errorMessage}',
      'version: ${report.appVersion}',
      'os: ${report.os}',
      if (report.cwd case final cwd?) 'cwd: $cwd',
      if (report.project case final project?) 'project: $project',
      '',
      'stdout:',
      report.stdout,
      '',
      'stderr:',
      report.stderr,
      if (report.debugContext case final debug?) ...['', 'debug:', debug],
    ].join('\n');
    return SingleChildScrollView(
      child: SelectableText(
        details,
        style: TextStyle(
          fontFamily: upegMonoFontFamily,
          fontFamilyFallback: upegMonoFontFamilyFallback,
          fontSize: 11,
        ),
      ),
    );
  }
}
