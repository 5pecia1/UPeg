/// FRB seam for the backup pipeline (Batch L3, K09/K10).
///
/// Two function-typed providers stand in front of the generated FRB
/// calls so widget tests can swap them for in-memory fakes without
/// loading the native dylib.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/rust/api/backup.dart';

typedef BackupExporter = String Function();
typedef BackupImporter = BackupImportReportDto Function(String json);

/// Production wiring calls into the generated FRB function. Tests
/// override with a closure that returns canned JSON / counts.
final backupExporterProvider = Provider<BackupExporter>((ref) => exportBackup);

final backupImporterProvider = Provider<BackupImporter>(
  (ref) =>
      (String json) => importBackup(json: json),
);
