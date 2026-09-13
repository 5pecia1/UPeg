library;

import 'dart:io' as io;
import 'dart:typed_data';

import 'package:file_picker/file_picker.dart' as fp;
import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:upeg/src/platform/bounded_file_reader.dart';
import 'package:upeg/src/platform/file_extension.dart';
import 'package:upeg/src/platform/file_input_resource_limits.dart';

/// A file the user chose for a `File` (byte) tool input: its display name, raw
/// bytes, and a best-effort MIME type (derived from the extension; `null` when
/// unknown). Carries bytes — not a path — so `File` inputs work on the web
/// build too, where filesystem paths do not exist.
typedef PickedFileData = ({String name, Uint8List bytes, String? mime});

/// A user-selected file existed, but its bytes could not be read.
final class FilePickerReadFailure implements Exception {
  const FilePickerReadFailure(this.fileName);

  final String fileName;

  @override
  String toString() => 'FilePickerReadFailure($fileName)';
}

enum FilePickerSelectionErrorCode {
  extensionNotAllowed,
  tooManyFiles,
  tooManyNodes,
  metadataTooLarge,
  fileTooLarge,
  totalTooLarge,
}

final class FilePickerSelectionFailure implements Exception {
  const FilePickerSelectionFailure(this.code, {this.fileName, this.maximum});

  final FilePickerSelectionErrorCode code;
  final String? fileName;
  final int? maximum;
}

Future<List<PickedFileData>> readPickedFileSelection(
  List<fp.PlatformFile> files, {
  required List<String>? allowedExtensions,
  required int? maxCount,
  required int? maxFileBytes,
  required int? maxTotalBytes,
  required bool createsDirectory,
}) async {
  _preflight(
    files,
    allowedExtensions: allowedExtensions,
    maxCount: maxCount,
    maxFileBytes: maxFileBytes,
    maxTotalBytes: maxTotalBytes,
    createsDirectory: createsDirectory,
  );

  final pickedFiles = <PickedFileData>[];
  var totalBytes = 0;
  final aggregateBudget = effectiveFileInputAggregateBytes(maxTotalBytes);
  for (final file in files) {
    final fileBudget = _effectiveFileBudget(maxFileBytes);
    final remainingAggregateBudget = aggregateBudget - totalBytes;
    final readBudget = _minimum(fileBudget, remainingAggregateBudget);
    final PickedFileData picked;
    try {
      picked = await _readPickedFile(file, readBudget);
    } on FileReadLimitExceeded {
      if (remainingAggregateBudget < fileBudget) {
        throw FilePickerSelectionFailure(
          FilePickerSelectionErrorCode.totalTooLarge,
          maximum: aggregateBudget,
        );
      }
      throw FilePickerSelectionFailure(
        FilePickerSelectionErrorCode.fileTooLarge,
        fileName: file.name,
        maximum: fileBudget,
      );
    }
    pickedFiles.add(picked);
    totalBytes += picked.bytes.length;
  }
  return List<PickedFileData>.unmodifiable(pickedFiles);
}

void _preflight(
  List<fp.PlatformFile> files, {
  required List<String>? allowedExtensions,
  required int? maxCount,
  required int? maxFileBytes,
  required int? maxTotalBytes,
  required bool createsDirectory,
}) {
  if (maxCount != null && files.length > maxCount) {
    throw FilePickerSelectionFailure(
      FilePickerSelectionErrorCode.tooManyFiles,
      maximum: maxCount,
    );
  }
  if (files.length > maxFileInputCount) {
    throw const FilePickerSelectionFailure(
      FilePickerSelectionErrorCode.tooManyFiles,
      maximum: maxFileInputCount,
    );
  }
  final nodeCount = flatFileInputNodeCount(
    files.length,
    createsDirectory: createsDirectory,
  );
  if (nodeCount > maxFileInputNodes) {
    throw const FilePickerSelectionFailure(
      FilePickerSelectionErrorCode.tooManyNodes,
      maximum: maxFileInputNodes,
    );
  }

  if (allowedExtensions case final extensions? when extensions.isNotEmpty) {
    final normalizedExtensions = normalizeFileExtensions(extensions);
    for (final file in files) {
      if (!hasAllowedFileExtension(file.name, normalizedExtensions)) {
        throw FilePickerSelectionFailure(
          FilePickerSelectionErrorCode.extensionNotAllowed,
          fileName: file.name,
        );
      }
    }
  }

  final metadataBytes = fileInputMetadataByteCount(
    files.map(
      (file) => (name: file.name, mime: mimeForFileExtension(file.extension)),
    ),
    includesSelectionRoot: createsDirectory,
  );
  if (metadataBytes > maxFileInputMetadataBytes) {
    throw const FilePickerSelectionFailure(
      FilePickerSelectionErrorCode.metadataTooLarge,
      maximum: maxFileInputMetadataBytes,
    );
  }

  final fileBudget = _effectiveFileBudget(maxFileBytes);
  for (final file in files) {
    if (file.size > fileBudget) {
      throw FilePickerSelectionFailure(
        FilePickerSelectionErrorCode.fileTooLarge,
        fileName: file.name,
        maximum: fileBudget,
      );
    }
  }

  final aggregateBudget = effectiveFileInputAggregateBytes(maxTotalBytes);
  final totalBytes = files.fold<int>(
    0,
    (total, file) => total + (file.size > 0 ? file.size : 0),
  );
  if (totalBytes > aggregateBudget) {
    throw FilePickerSelectionFailure(
      FilePickerSelectionErrorCode.totalTooLarge,
      maximum: aggregateBudget,
    );
  }
}

Future<PickedFileData> _readPickedFile(
  fp.PlatformFile file,
  int maximumBytes,
) async {
  final mime = mimeForFileExtension(file.extension);
  try {
    final stream = file.readStream;
    if (stream != null) {
      return (
        name: file.name,
        bytes: await readBoundedByteStream(stream, maximumBytes: maximumBytes),
        mime: mime,
      );
    }
    if (kIsWeb) throw FilePickerReadFailure(file.name);
    final nativePath = file.path;
    if (nativePath == null) throw FilePickerReadFailure(file.name);
    return (
      name: file.name,
      bytes: await readBoundedByteStream(
        io.File(nativePath).openRead(0, maximumBytes + 1),
        maximumBytes: maximumBytes,
      ),
      mime: mime,
    );
  } on FileReadLimitExceeded {
    rethrow;
  } on FilePickerReadFailure {
    rethrow;
  } on Exception {
    throw FilePickerReadFailure(file.name);
  }
}

int _effectiveFileBudget(int? maxFileBytes) {
  if (maxFileBytes == null) return maxFileInputTransportBytes;
  return _minimum(maxFileInputTransportBytes, maxFileBytes);
}

int _minimum(int left, int right) => left < right ? left : right;
