library;

import '../test_helpers/i18n_test_catalog.dart';

import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/features/host_attach/attach_client.dart';
import 'package:upeg/src/platform/file_picker_bridge.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';
import 'package:upeg/src/widgets/expanded_modal/structured_output.dart';

const String _hostFileBody =
    '{"ok":true,"primary_output_id":"file","outputs":[{"id":"file",'
    '"label":"File","kind":"file","value":{"name":"report.txt",'
    '"is_dir":false,"mime":"text/plain","content":{"kind":"bytes",'
    '"bytes":"AQID"}}}]}';
const String _savePath = '/tmp/report.txt';

final class _RecordingFileBridge extends FilePickerBridge {
  String? requestedDefaultName;
  Uint8List? savedBytes;

  @override
  Future<String?> pickSavePath({
    required String defaultName,
    String dialogTitle = 'Save file',
    List<String>? allowedExtensions,
  }) async {
    requestedDefaultName = defaultName;
    return _savePath;
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

  @override
  Future<void> writeBytesTo(String path, Uint8List bytes) async {
    if (path != _savePath) {
      throw StateError('unexpected save path: $path');
    }
    savedBytes = bytes;
  }
}

final class _NoopClipboardWriter extends ClipboardWriter {
  @override
  Future<void> write(String text) async {}
}

final class _HostAttachFileOutputRobot {
  _HostAttachFileOutputRobot(this.tester, this.bridge);

  final WidgetTester tester;
  final _RecordingFileBridge bridge;
  Uint8List? decodedBytes;

  Future<void> pumpHostFileOutput() async {
    final dispatch = decodeDispatchBody(_hostFileBody);
    expect(dispatch, isA<AttachDispatchOk>());
    final entry = (dispatch as AttachDispatchOk).result.outputs.single;
    switch (entry.value) {
      case CanonicalOutputValue_File(
        value: CanonicalFileValue(
          content: CanonicalFileContent_Bytes(:final bytes),
        ),
      ):
        decodedBytes = bytes;
      default:
        decodedBytes = null;
    }
    expect(
      entry.value,
      isA<CanonicalOutputValue_File>(),
      reason: 'host File은 렌더 전에 typed 값으로 승격되어야 한다',
    );

    final theme = UpegTheme.darkTheme();
    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          ...i18nTestOverrides,
          filePickerBridgeProvider.overrideWithValue(bridge),
        ],
        child: MaterialApp(
          theme: theme,
          home: Scaffold(
            body: StructuredOutputBlock(
              field: const OutputFieldDto(
                key: 'file',
                label: 'File',
                fieldType: OutputFieldType_File(),
              ),
              entry: entry,
              tokens: theme.extension<UpegTokens>()!,
              writer: _NoopClipboardWriter(),
              primary: true,
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
  }

  void expectDecodedSummary() {
    expect(find.text('report.txt · text/plain · 3 bytes'), findsOneWidget);
  }

  Future<void> saveFile() async {
    await tester.tap(find.byKey(const Key('structured-output-file-save')));
    await tester.pumpAndSettle();
  }

  void expectDecodedBufferSavedWithoutAnotherDecode() {
    expect(bridge.requestedDefaultName, 'report.txt');
    expect(bridge.savedBytes, orderedEquals(<int>[1, 2, 3]));
    expect(
      identical(bridge.savedBytes, decodedBytes),
      isTrue,
      reason: '렌더러는 attach boundary에서 디코드한 buffer를 그대로 저장해야 한다',
    );
  }
}

void main() {
  testWidgets('host File 출력은 요약을 렌더하고 디코드한 buffer를 그대로 저장한다', (tester) async {
    final bridge = _RecordingFileBridge();
    final robot = _HostAttachFileOutputRobot(tester, bridge);

    await robot.pumpHostFileOutput();
    robot.expectDecodedSummary();
    await robot.saveFile();
    robot.expectDecodedBufferSavedWithoutAnotherDecode();
  });
}
