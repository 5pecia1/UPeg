import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';
import 'package:upeg/src/platform/file_picker_bridge.dart';

final imagePreviewFixture = base64Decode(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==',
);

class FakeImageFileBridge extends FilePickerBridge {
  bool failSave = false;
  bool cancelSave = false;
  Completer<void>? writeGate;
  int writes = 0;
  String? savedName;

  @override
  Future<String?> pickSavePath({
    required String defaultName,
    String dialogTitle = 'Save file',
    List<String>? allowedExtensions,
  }) async {
    savedName = defaultName;
    return cancelSave ? null : '/tmp/$defaultName';
  }

  @override
  Future<void> writeBytesTo(String path, Uint8List bytes) async {
    writes++;
    if (writeGate case final gate?) await gate.future;
    if (failSave) throw StateError('destination unavailable');
  }

  @override
  Future<PickedFileData?> pickOpenFile({
    String dialogTitle = 'Pick a file',
    List<String>? allowedExtensions,
  }) async => null;
  @override
  Future<List<PickedFileData>?> pickOpenFiles({
    String dialogTitle = 'Pick files',
    List<String>? allowedExtensions,
    bool allowMultiple = false,
    int? maxCount,
    int? maxFileBytes,
    int? maxTotalBytes,
  }) async => null;
  @override
  Future<String?> pickOpenPath() async => null;
}
