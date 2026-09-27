/// Injectable state seam for persisted native diagnostics.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/rust/api/diagnostics.dart' as frb;

final class DiagnosticSummary {
  const DiagnosticSummary({
    required this.id,
    required this.runId,
    required this.occurredAtMs,
    required this.source,
    required this.errorCode,
    required this.errorMessage,
    required this.status,
    this.toolId,
  });

  final String id;
  final String runId;
  final int occurredAtMs;
  final String? toolId;
  final String source;
  final String errorCode;
  final String errorMessage;
  final String status;
}

final class DiagnosticReport {
  const DiagnosticReport({
    required this.summary,
    required this.appVersion,
    required this.os,
    required this.stdout,
    required this.stderr,
    this.cwd,
    this.project,
    this.debugContext,
  });

  final DiagnosticSummary summary;
  final String appVersion;
  final String os;
  final String? cwd;
  final String? project;
  final String stdout;
  final String stderr;
  final String? debugContext;
}

abstract interface class DiagnosticsApi {
  Future<List<DiagnosticSummary>> list({required int limit});
  Future<DiagnosticReport?> show(String id);

  /// Returns the native canonical, redacted JSON report. `debug` is only
  /// selected explicitly by the user; normal export is the safe default.
  Future<String> export(String id, {required bool debug});
}

final diagnosticsApiProvider = Provider<DiagnosticsApi>(
  (ref) => const FrbDiagnosticsApi(),
);

final class FrbDiagnosticsApi implements DiagnosticsApi {
  const FrbDiagnosticsApi();

  @override
  Future<String> export(String id, {required bool debug}) =>
      frb.exportDiagnostic(id: id, debug: debug);

  @override
  Future<List<DiagnosticSummary>> list({required int limit}) async =>
      frb.listDiagnostics(limit: limit).map(_summary).toList(growable: false);

  @override
  Future<DiagnosticReport?> show(String id) async {
    final dto = frb.showDiagnostic(id: id);
    return dto == null ? null : _report(dto);
  }
}

DiagnosticSummary _summary(frb.DiagnosticSummaryDto dto) => DiagnosticSummary(
  id: dto.id,
  runId: dto.runId,
  occurredAtMs: dto.occurredAtMs.toInt(),
  toolId: dto.toolId,
  source: dto.source,
  errorCode: dto.errorCode,
  errorMessage: dto.errorMessage,
  status: dto.status,
);

DiagnosticReport _report(frb.DiagnosticReportDto dto) => DiagnosticReport(
  summary: DiagnosticSummary(
    id: dto.id,
    runId: dto.runId,
    occurredAtMs: dto.occurredAtMs.toInt(),
    toolId: dto.toolId,
    source: dto.source,
    errorCode: dto.errorCode,
    errorMessage: dto.errorMessage,
    status: dto.status,
  ),
  appVersion: dto.appVersion,
  os: dto.os,
  cwd: dto.cwd,
  project: dto.project,
  stdout: dto.stdout,
  stderr: dto.stderr,
  debugContext: dto.debugContext,
);

typedef DiagnosticsLimit = int;
const DiagnosticsLimit kRecentDiagnosticsLimit = 12;

final recentDiagnosticsProvider =
    FutureProvider.autoDispose<List<DiagnosticSummary>>(
      (ref) => ref
          .watch(diagnosticsApiProvider)
          .list(limit: kRecentDiagnosticsLimit),
    );

Future<DiagnosticSummary?> diagnosticForRun(
  DiagnosticsApi api,
  String runId,
) async {
  final records = await api.list(limit: 200);
  for (final record in records) {
    if (record.runId == runId) return record;
  }
  return null;
}
