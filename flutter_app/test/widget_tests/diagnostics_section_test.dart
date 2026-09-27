library;

import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/rust/api/i18n.dart' show LocaleDto;
import 'package:upeg/src/rust/api/tweaks.dart';
import 'package:upeg/src/state/diagnostics_provider.dart';
import 'package:upeg/src/state/tweaks_provider.dart';
import 'package:upeg/src/platform/file_picker_bridge.dart';
import 'package:upeg/src/widgets/diagnostics_section.dart';

const _tweaks = TweaksDto(
  theme: 'Light',
  accent: 'Green',
  showHoles: true,
  locale: 'En',
  localHttpHost: false,
);

class _DiagnosticsApi implements DiagnosticsApi {
  final List<bool> exportDebugValues = [];

  @override
  Future<String> export(String id, {required bool debug}) async {
    exportDebugValues.add(debug);
    return '{"id":"$id"}';
  }

  @override
  Future<List<DiagnosticSummary>> list({required int limit}) async => const [
    DiagnosticSummary(
      id: 'd-1',
      runId: 'run-1',
      occurredAtMs: 1,
      source: 'external',
      errorCode: 'PROCESS_FAILED',
      errorMessage: 'command exited 1',
      status: 'failed',
    ),
  ];

  @override
  Future<DiagnosticReport?> show(String id) async => const DiagnosticReport(
    summary: DiagnosticSummary(
      id: 'd-1',
      runId: 'run-1',
      occurredAtMs: 1,
      source: 'external',
      errorCode: 'PROCESS_FAILED',
      errorMessage: 'command exited 1',
      status: 'failed',
    ),
    appVersion: 'test',
    os: 'test-os',
    stdout: 'complete stdout',
    stderr: 'complete stderr',
  );
}

class _SaveBridge extends FilePickerBridge {
  String? name;
  String? mime;
  Uint8List? bytes;

  @override
  Future<bool> saveBytes({
    required String name,
    required Uint8List bytes,
    required String dialogTitle,
    String mime = 'application/octet-stream',
  }) async {
    this.name = name;
    this.mime = mime;
    this.bytes = bytes;
    return true;
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => throw UnimplementedError();
}

Widget _harness({_DiagnosticsApi? api, _SaveBridge? fileBridge}) =>
    ProviderScope(
      overrides: [
        diagnosticsApiProvider.overrideWithValue(api ?? _DiagnosticsApi()),
        if (fileBridge != null)
          filePickerBridgeProvider.overrideWithValue(fileBridge),
        tweaksLoaderProvider.overrideWith(
          (ref) =>
              () => _tweaks,
        ),
        i18nTranslateOverride.overrideWithValue((key, LocaleDto locale) => key),
        i18nTranslateArgsOverride.overrideWithValue(
          (key, LocaleDto locale, List<String> names, List<String> values) =>
              key,
        ),
      ],
      child: const MaterialApp(home: Scaffold(body: DiagnosticsSection())),
    );

void main() {
  test(
    'run lookup does not substitute another failure of the same tool',
    () async {
      final api = _DiagnosticsApi();
      expect((await diagnosticForRun(api, 'run-1'))?.id, 'd-1');
      expect(await diagnosticForRun(api, 'different-run'), isNull);
    },
  );

  testWidgets('opens the complete canonical report from a recent row', (
    tester,
  ) async {
    await tester.pumpWidget(_harness());
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const Key('diagnostic-d-1')));
    await tester.pumpAndSettle();

    expect(find.textContaining('complete stdout'), findsOneWidget);
    expect(find.textContaining('complete stderr'), findsOneWidget);
    expect(find.textContaining('PROCESS_FAILED'), findsWidgets);
  });

  testWidgets('copy and save use the native redacted report', (tester) async {
    final api = _DiagnosticsApi();
    final fileBridge = _SaveBridge();
    String? clipboardText;
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(SystemChannels.platform, (call) async {
      if (call.method == 'Clipboard.setData') {
        clipboardText =
            (call.arguments as Map<Object?, Object?>)['text'] as String?;
      }
      return null;
    });
    addTearDown(
      () => messenger.setMockMethodCallHandler(SystemChannels.platform, null),
    );
    await tester.pumpWidget(_harness(api: api, fileBridge: fileBridge));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('diagnostic-d-1')));
    await tester.pumpAndSettle();

    await tester.tap(find.text('diagnostics.copy'));
    await tester.pumpAndSettle();
    expect(clipboardText, '{"id":"d-1"}');

    await tester.tap(find.text('diagnostics.save'));
    await tester.pumpAndSettle();
    expect(fileBridge.name, 'upeg-diagnostic-d-1.json');
    expect(fileBridge.mime, 'application/json');
    expect(utf8.decode(fileBridge.bytes!), '{"id":"d-1"}');
    expect(api.exportDebugValues, [false, false]);
  });
}
