/// Backup export / import section, mounted under the Settings →
/// Backup heading (Batch L3, K09/K10).
///
/// Export: ask FRB for the JSON snapshot, hand it to the
/// [FilePickerBridge] so the OS save dialog writes it to disk.
/// Import: ask the bridge for the user-selected file bytes, pass the
/// utf-8 decoded JSON to FRB, surface the returned counts as a
/// confirmation line (or the error message on failure).
///
/// The bridge is an abstract test seam — production wraps the
/// `file_picker` package (MIT, pub.dev); tests inject a recording fake.
library;

import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/platform/file_picker_bridge.dart';
import 'package:upeg/src/state/backup_provider.dart';

/// Catalog keys (upeg-pegboard-ui/src/i18n.rs) — copy renders through
/// `t()`/`tRead()` so the section follows the active locale. The button
/// and failure keys predate this widget; the dialog/result keys were
/// added alongside it.
const String _exportButtonKey = 'settings.backup.export';
const String _importButtonKey = 'settings.backup.import';
const String _exportDialogTitleKey = 'settings.backup.export_dialog_title';
const String _importDialogTitleKey = 'settings.backup.import_dialog_title';
const String _exportedKey = 'settings.backup.exported';
const String _exportFailedKey = 'settings.backup.export_failed';
const String _importedKey = 'settings.backup.imported';
const String _importFailedKey = 'settings.backup.import_failed';

class BackupSection extends ConsumerStatefulWidget {
  const BackupSection({super.key});

  @override
  ConsumerState<BackupSection> createState() => _BackupSectionState();
}

class _BackupSectionState extends ConsumerState<BackupSection> {
  String? _statusMessage;
  String? _errorMessage;

  Future<void> _onExport() async {
    setState(() {
      _errorMessage = null;
      _statusMessage = null;
    });
    try {
      final exporter = ref.read(backupExporterProvider);
      final bridge = ref.read(filePickerBridgeProvider);
      final json = exporter();
      final path = await bridge.pickSavePath(
        defaultName: 'upeg-backup.json',
        dialogTitle: tRead(ref, _exportDialogTitleKey),
        allowedExtensions: const <String>['json'],
      );
      if (path == null) {
        // User cancelled — stay silent (no toast, no error).
        return;
      }
      await bridge.writeBytesTo(path, Uint8List.fromList(utf8.encode(json)));
      setState(() {
        _statusMessage = tRead(ref, _exportedKey, {'path': path});
      });
    } on Object catch (err) {
      setState(() {
        _errorMessage = tRead(ref, _exportFailedKey, {'msg': '$err'});
      });
    }
  }

  Future<void> _onImport() async {
    setState(() {
      _errorMessage = null;
      _statusMessage = null;
    });
    try {
      final bridge = ref.read(filePickerBridgeProvider);
      final importer = ref.read(backupImporterProvider);
      final selected = await bridge.pickOpenFile(
        dialogTitle: tRead(ref, _importDialogTitleKey),
        allowedExtensions: const <String>['json'],
      );
      if (selected == null) {
        return;
      }
      final json = utf8.decode(selected.bytes);
      final report = importer(json);
      setState(() {
        _statusMessage = tRead(ref, _importedKey, {
          'boards': '${report.boardCount}',
          'layouts': '${report.layoutCount}',
          'memos': '${report.memoCount}',
        });
      });
    } on Object catch (err) {
      setState(() {
        _errorMessage = tRead(ref, _importFailedKey, {'msg': '$err'});
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            OutlinedButton(
              key: const Key('backup-section-export-button'),
              onPressed: _onExport,
              child: Text(t(ref, _exportButtonKey)),
            ),
            const SizedBox(width: 8),
            OutlinedButton(
              key: const Key('backup-section-import-button'),
              onPressed: _onImport,
              child: Text(t(ref, _importButtonKey)),
            ),
          ],
        ),
        if (_statusMessage != null)
          Padding(
            padding: const EdgeInsets.only(top: 8),
            child: Text(
              _statusMessage!,
              key: const Key('backup-section-status'),
            ),
          ),
        if (_errorMessage != null)
          Padding(
            padding: const EdgeInsets.only(top: 8),
            child: Text(
              _errorMessage!,
              key: const Key('backup-section-error'),
              style: TextStyle(color: Theme.of(context).colorScheme.error),
            ),
          ),
      ],
    );
  }
}
