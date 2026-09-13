/// BackupSection widget tests (Batch L3, K09/K10).
///
/// The section mounts under `TweaksForm`'s Backup heading and exposes
/// two buttons: Export (asks the FRB layer for a JSON snapshot, hands
/// it to a [FilePickerBridge] to save) and Import (asks the bridge for
/// file bytes, hands them to the FRB layer). The bridge is the test
/// seam — production wraps the `file_picker` package, tests swap in
/// `_RecordingFilePickerBridge`.
library;

import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/platform/file_picker_bridge.dart';
import 'package:upeg/src/rust/api/backup.dart';
import 'package:upeg/src/state/backup_provider.dart';
import 'package:upeg/src/widgets/backup_section.dart';

import '../test_helpers/i18n_test_catalog.dart';

class _RecordingFilePickerBridge extends FilePickerBridge {
  _RecordingFilePickerBridge();

  String? savePathToReturn;
  PickedFileData? openFileToReturn;
  final List<String> requestedDefaultNames = <String>[];
  final List<String> savedJsons = <String>[];
  int openCalls = 0;

  @override
  Future<String?> pickSavePath({
    required String defaultName,
    String dialogTitle = 'Save file',
    List<String>? allowedExtensions,
  }) async {
    requestedDefaultNames.add(defaultName);
    return savePathToReturn;
  }

  @override
  Future<PickedFileData?> pickOpenFile({
    String dialogTitle = 'Pick a file',
    List<String>? allowedExtensions,
  }) async {
    openCalls += 1;
    return openFileToReturn;
  }

  @override
  Future<List<PickedFileData>?> pickOpenFiles({
    String dialogTitle = 'Pick files',
    List<String>? allowedExtensions,
    bool allowMultiple = false,
    int? maxCount,
    int? maxFileBytes,
    int? maxTotalBytes,
  }) async {
    final selected = openFileToReturn;
    return selected == null ? null : [selected];
  }

  @override
  Future<void> writeBytesTo(String path, Uint8List bytes) async {
    savedJsons.add(utf8.decode(bytes));
  }

  // Not exercised by the backup flow — defined to satisfy the abstract
  // contract for Batch N5's modal `FilePath` field.
  @override
  Future<String?> pickOpenPath() async => null;
}

const String _validJson =
    '{"version":2,"tweaks":{"theme":"Dark","accent":"Green","show_holes":true,"locale":"En"},"layouts":{},"memos":{},"boards":[]}';

Widget _harness({
  required FilePickerBridge bridge,
  required String Function() exportFn,
  required BackupImportReportDto Function(String) importFn,
}) {
  return ProviderScope(
    overrides: [
      ...i18nTestOverrides,
      backupExporterProvider.overrideWith((ref) => exportFn),
      backupImporterProvider.overrideWith((ref) => importFn),
      filePickerBridgeProvider.overrideWithValue(bridge),
    ],
    child: const MaterialApp(home: Scaffold(body: BackupSection())),
  );
}

void main() {
  group('BackupSection', () {
    testWidgets(
      'BackupSection_export_탭은_FRB_export_backup을_호출하고_받은_JSON을_file_picker로_저장한다',
      (tester) async {
        final bridge = _RecordingFilePickerBridge()
          ..savePathToReturn = '/tmp/u.json';
        int exportCalls = 0;
        await tester.pumpWidget(
          _harness(
            bridge: bridge,
            exportFn: () {
              exportCalls += 1;
              return _validJson;
            },
            importFn: (_) => const BackupImportReportDto(
              boardCount: 0,
              layoutCount: 0,
              memoCount: 0,
            ),
          ),
        );
        await tester.pumpAndSettle();

        await tester.tap(find.byKey(const Key('backup-section-export-button')));
        await tester.pumpAndSettle();

        expect(exportCalls, 1);
        expect(bridge.requestedDefaultNames, ['upeg-backup.json']);
        expect(bridge.savedJsons, [_validJson]);
      },
    );

    testWidgets('BackupSection_export_탭은_사용자가_취소하면_파일을_쓰지_않는다', (tester) async {
      final bridge = _RecordingFilePickerBridge()..savePathToReturn = null;
      await tester.pumpWidget(
        _harness(
          bridge: bridge,
          exportFn: () => _validJson,
          importFn: (_) => const BackupImportReportDto(
            boardCount: 0,
            layoutCount: 0,
            memoCount: 0,
          ),
        ),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('backup-section-export-button')));
      await tester.pumpAndSettle();

      // No write attempt when the user cancels the save dialog.
      expect(bridge.savedJsons, isEmpty);
    });

    testWidgets(
      'BackupSection_import_탭은_file_picker로_파일을_열고_FRB_import_backup을_호출한다',
      (tester) async {
        final bridge = _RecordingFilePickerBridge()
          ..openFileToReturn = (
            name: 'selected-backup.json',
            bytes: Uint8List.fromList(utf8.encode(_validJson)),
            mime: null,
          );
        final importCalls = <String>[];
        await tester.pumpWidget(
          _harness(
            bridge: bridge,
            exportFn: () => _validJson,
            importFn: (json) {
              importCalls.add(json);
              return const BackupImportReportDto(
                boardCount: 3,
                layoutCount: 1,
                memoCount: 0,
              );
            },
          ),
        );
        await tester.pumpAndSettle();

        await tester.tap(find.byKey(const Key('backup-section-import-button')));
        await tester.pumpAndSettle();

        expect(bridge.openCalls, 1);
        expect(importCalls, hasLength(1));
        expect(importCalls.single, _validJson);
      },
    );

    testWidgets('BackupSection_import_실패는_에러_메시지를_표시한다', (tester) async {
      final bridge = _RecordingFilePickerBridge()
        ..openFileToReturn = (
          name: 'broken.json',
          bytes: Uint8List.fromList(utf8.encode('not-json')),
          mime: null,
        );
      await tester.pumpWidget(
        _harness(
          bridge: bridge,
          exportFn: () => _validJson,
          importFn: (_) => throw Exception('schema mismatch'),
        ),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('backup-section-import-button')));
      await tester.pumpAndSettle();

      // The error surface is identified by a deterministic Key so the
      // test doesn't pin the exact message wording (i18n changes
      // freely).
      expect(find.byKey(const Key('backup-section-error')), findsOneWidget);
    });

    testWidgets('BackupSection_import_성공은_보드_count를_노출한다', (tester) async {
      final bridge = _RecordingFilePickerBridge()
        ..openFileToReturn = (
          name: 'selected-backup.json',
          bytes: Uint8List.fromList(utf8.encode(_validJson)),
          mime: null,
        );
      await tester.pumpWidget(
        _harness(
          bridge: bridge,
          exportFn: () => _validJson,
          importFn: (_) => const BackupImportReportDto(
            boardCount: 5,
            layoutCount: 2,
            memoCount: 0,
          ),
        ),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('backup-section-import-button')));
      await tester.pumpAndSettle();

      expect(find.byKey(const Key('backup-section-status')), findsOneWidget);
      // The status surface contains the imported board count so the
      // user can confirm the restore took.
      expect(find.textContaining('5'), findsWidgets);
    });
  });
}
