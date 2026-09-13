/// File picker and filesystem write boundary shared by forms, backup, and
/// output renderers.
library;

import 'dart:io' as io;
import 'dart:typed_data';

import 'package:file_picker/file_picker.dart' as fp;
import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/platform/file_picker_selection.dart';
import 'package:upeg/src/platform/file_download_stub.dart'
    if (dart.library.js_interop) 'package:upeg/src/platform/file_download_web.dart'
    as download;

export 'package:upeg/src/platform/file_extension.dart'
    show mimeForFileExtension;
export 'package:upeg/src/platform/file_picker_selection.dart'
    show
        FilePickerReadFailure,
        FilePickerSelectionErrorCode,
        FilePickerSelectionFailure,
        PickedFileData;

/// Filesystem boundary used by UI surfaces that need native open/save dialogs.
///
/// Production binds each operation to `file_picker`; tests bind a fake so no OS
/// dialog or real filesystem write is needed.
abstract class FilePickerBridge {
  const FilePickerBridge();

  /// Save a complete output. Returns false when the destination dialog is cancelled.
  Future<bool> saveBytes({
    required String name,
    required Uint8List bytes,
    required String dialogTitle,
    String mime = 'application/octet-stream',
  }) async {
    final path = await pickSavePath(
      defaultName: name,
      dialogTitle: dialogTitle,
    );
    if (path == null) return false;
    await writeBytesTo(path, bytes);
    return true;
  }

  /// Ask the user for a destination path. Returns `null` when the user cancels
  /// the save dialog.
  Future<String?> pickSavePath({
    required String defaultName,
    String dialogTitle = 'Save file',
    List<String>? allowedExtensions,
  });

  /// Ask the user for a file and return its display name, bytes, and
  /// best-effort MIME type.
  ///
  /// Returns `null` when cancelled and throws [FilePickerReadFailure] when the
  /// selected file cannot be read.
  ///
  /// Powers `File` (byte) tool inputs across every surface, including the web
  /// build, where filesystem paths do not exist.
  Future<PickedFileData?> pickOpenFile({
    String dialogTitle = 'Pick a file',
    List<String>? allowedExtensions,
  });

  /// Ask the user for one or more files.
  Future<List<PickedFileData>?> pickOpenFiles({
    String dialogTitle = 'Pick files',
    List<String>? allowedExtensions,
    bool allowMultiple = false,
    int? maxCount,
    int? maxFileBytes,
    int? maxTotalBytes,
  });

  /// Ask the user for a file path only.
  ///
  /// Returns `null` when cancelled or when the platform does not expose native
  /// file paths, such as Web.
  Future<String?> pickOpenPath();

  /// Write [bytes] to [path].
  Future<void> writeBytesTo(String path, Uint8List bytes);
}

/// Production implementation backed by the `file_picker` package.
class FilePickerPluginBridge extends FilePickerBridge {
  const FilePickerPluginBridge();

  @override
  Future<bool> saveBytes({
    required String name,
    required Uint8List bytes,
    required String dialogTitle,
    String mime = 'application/octet-stream',
  }) async {
    if (kIsWeb) {
      download.downloadBytes(bytes, name, mime);
      return true;
    }
    return super.saveBytes(
      name: name,
      bytes: bytes,
      dialogTitle: dialogTitle,
      mime: mime,
    );
  }

  @override
  Future<String?> pickSavePath({
    required String defaultName,
    String dialogTitle = 'Save file',
    List<String>? allowedExtensions,
  }) async {
    return fp.FilePicker.platform.saveFile(
      dialogTitle: dialogTitle,
      fileName: defaultName,
      type: allowedExtensions == null ? fp.FileType.any : fp.FileType.custom,
      allowedExtensions: allowedExtensions,
    );
  }

  @override
  Future<PickedFileData?> pickOpenFile({
    String dialogTitle = 'Pick a file',
    List<String>? allowedExtensions,
  }) async {
    final picked = await pickOpenFiles(
      dialogTitle: dialogTitle,
      allowedExtensions: allowedExtensions,
    );
    return picked?.firstOrNull;
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
    final extensionFilter = allowedExtensions?.isNotEmpty == true
        ? allowedExtensions
        : null;
    final result = await fp.FilePicker.platform.pickFiles(
      dialogTitle: dialogTitle,
      type: extensionFilter == null ? fp.FileType.any : fp.FileType.custom,
      allowedExtensions: extensionFilter,
      allowMultiple: allowMultiple,
      withData: false,
      withReadStream: true,
    );
    if (result == null || result.files.isEmpty) {
      return null;
    }
    return readPickedFileSelection(
      result.files,
      allowedExtensions: extensionFilter,
      maxCount: maxCount,
      maxFileBytes: maxFileBytes,
      maxTotalBytes: maxTotalBytes,
      createsDirectory: allowMultiple,
    );
  }

  @override
  Future<String?> pickOpenPath() async {
    if (kIsWeb) {
      return null;
    }
    final result = await fp.FilePicker.platform.pickFiles(
      dialogTitle: 'Pick a file',
    );
    if (result == null || result.files.isEmpty) {
      return null;
    }
    return result.files.first.path;
  }

  @override
  Future<void> writeBytesTo(String path, Uint8List bytes) async {
    await io.File(path).writeAsBytes(bytes, flush: true);
  }
}

/// Default bridge. Tests override this provider to avoid touching plugins.
final filePickerBridgeProvider = Provider<FilePickerBridge>(
  (ref) => const FilePickerPluginBridge(),
);
