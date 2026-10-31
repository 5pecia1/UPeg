import 'package:upeg/src/state/diagnostics_provider.dart';

class FakeClipboardDiagnostics implements DiagnosticsApi {
  static const payload = '{"diagnostic":"redacted export"}';
  String? runId;
  final debugExports = <bool>[];

  @override
  Future<List<DiagnosticSummary>> list({required int limit}) async => [
    if (runId != null)
      DiagnosticSummary(
        id: 'saved-report',
        runId: runId!,
        occurredAtMs: 1000,
        source: 'external',
        errorCode: 'failed',
        errorMessage: 'tool error',
        status: 'failed',
      ),
  ];

  @override
  Future<DiagnosticReport?> show(String id) async => null;

  @override
  Future<String> export(String id, {required bool debug}) async {
    debugExports.add(debug);
    return payload;
  }
}
