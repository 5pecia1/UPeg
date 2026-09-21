/// Tests for the `File` (byte) input field kind.
///
/// `InputFieldType_File` carries *bytes*, not a path — the picked file is held
/// as a [FileFormValue] wrapping a [CanonicalFileValue], and lowers to the
/// canonical `FileValue` JSON the Rust dispatcher's `read_file` deserializes.
/// These tests pin both the widget behaviour (choose → stored value) and the
/// JSON encoding shape, including the MIME the picker infers from the
/// extension.
library;

import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/platform/file_picker_bridge.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/rust/api/tools/file_input_policy.dart';
import 'package:upeg/src/widgets/expanded_modal/file_input_field.dart';
import 'package:upeg/src/widgets/expanded_modal/form_value.dart';
import 'package:upeg/src/widgets/expanded_modal/generic_form.dart';

import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/tool_fixture.dart';

/// A bridge whose file picker returns a fixed [PickedFileData] (or `null`).
class _FileBridge extends FilePickerBridge {
  const _FileBridge(this.file);
  final PickedFileData? file;

  @override
  Future<String?> pickSavePath({
    required String defaultName,
    String dialogTitle = 'Save file',
    List<String>? allowedExtensions,
  }) async => null;

  @override
  Future<PickedFileData?> pickOpenFile({
    String dialogTitle = 'Pick a file',
    List<String>? allowedExtensions,
  }) async => file;

  @override
  Future<List<PickedFileData>?> pickOpenFiles({
    String dialogTitle = 'Pick files',
    List<String>? allowedExtensions,
    bool allowMultiple = false,
    int? maxCount,
    int? maxFileBytes,
    int? maxTotalBytes,
  }) async {
    final selected = file;
    return selected == null ? null : [selected];
  }

  @override
  Future<String?> pickOpenPath() async => null;

  @override
  Future<void> writeBytesTo(String path, Uint8List bytes) async {}
}

Widget _harness({
  required ToolDto tool,
  required GenericFormController controller,
  required FilePickerBridge bridge,
}) {
  return ProviderScope(
    overrides: [
      ...i18nTestOverrides,
      filePickerBridgeProvider.overrideWithValue(bridge),
    ],
    child: MaterialApp(
      home: Scaffold(
        body: GenericFormWidget(tool: tool, controller: controller),
      ),
    ),
  );
}

ToolDto _fileTool() => fixtureToolDto(
  id: 'fixture.file',
  inputFields: const [
    InputFieldDto(
      key: 'input',
      label: 'Input',
      fieldType: InputFieldType_File(
        policy: FileInputPolicyDto(
          extensions: <String>[],
          maxCount: 1,
          maxFileBytes: null,
          maxTotalBytes: null,
        ),
      ),
      required_: true,
    ),
  ],
);

/// Decode the form's canonical tool args and return the `input` object.
Map<String, dynamic> _encodedInput(GenericFormController controller) {
  final args = formValuesToToolArgs({'input': controller.value('input')!});
  final decoded = jsonDecode(args.encodeJson()) as Map<String, dynamic>;
  return decoded['input'] as Map<String, dynamic>;
}

void main() {
  group('GenericForm File field', () {
    testWidgets('a_file_field_starts_empty_and_carries_no_path_label', (
      tester,
    ) async {
      final controller = GenericFormController();
      await tester.pumpWidget(
        _harness(
          tool: _fileTool(),
          controller: controller,
          bridge: const _FileBridge(null),
        ),
      );

      expect(find.text(i18nEn('modal.file.empty_prompt')), findsOneWidget);
      expect(find.byKey(FileInputKeys.pickButton), findsOneWidget);
      expect(controller.value('input'), isNull);
      // No stray "(file path)" label — this field takes bytes, not a path.
      expect(find.textContaining('file path'), findsNothing);
    });

    testWidgets(
      'a_file_field_stores_the_picked_file_bytes_and_mime_as_a_fileformvalue',
      (tester) async {
        final controller = GenericFormController();
        final bytes = Uint8List.fromList(const [0x50, 0x4B, 0x03, 0x04]);
        await tester.pumpWidget(
          _harness(
            tool: _fileTool(),
            controller: controller,
            bridge: _FileBridge((
              name: 'deck.pptx',
              bytes: bytes,
              mime: 'application/zip',
            )),
          ),
        );

        await tester.tap(find.byIcon(Icons.attach_file));
        await tester.pumpAndSettle();

        final stored = controller.value('input');
        expect(stored, isA<FileFormValue>());
        final file = (stored! as FileFormValue).value;
        expect(file.name, 'deck.pptx');
        expect(file.isDir, isFalse);
        expect(file.mime, 'application/zip');
        expect(file.content, CanonicalFileContent.bytes(bytes: bytes));
        // The field reflects the chosen file by name.
        expect(find.text('deck.pptx'), findsOneWidget);
      },
    );

    testWidgets(
      'a_file_field_encodes_to_the_canonical_json_read_file_expects',
      (tester) async {
        final controller = GenericFormController();
        await tester.pumpWidget(
          _harness(
            tool: _fileTool(),
            controller: controller,
            bridge: _FileBridge((
              name: 'a.pdf',
              bytes: Uint8List.fromList(const [255, 0]),
              mime: 'application/pdf',
            )),
          ),
        );

        await tester.tap(find.byIcon(Icons.attach_file));
        await tester.pumpAndSettle();

        // The shape upeg-core's FileContent serde / read_file want.
        final input = _encodedInput(controller);
        expect(input['name'], 'a.pdf');
        expect(input['is_dir'], false);
        expect(input['mime'], 'application/pdf');
        final content = input['content'] as Map<String, dynamic>;
        expect(content['kind'], 'bytes');
        expect(content['bytes'], '/wA=');
      },
    );

    testWidgets('a_file_field_omits_mime_from_the_json_when_absent', (
      tester,
    ) async {
      final controller = GenericFormController();
      await tester.pumpWidget(
        _harness(
          tool: _fileTool(),
          controller: controller,
          bridge: _FileBridge((
            name: 'blob.bin',
            bytes: Uint8List.fromList(const [1, 2]),
            mime: null,
          )),
        ),
      );

      await tester.tap(find.byIcon(Icons.attach_file));
      await tester.pumpAndSettle();

      expect(_encodedInput(controller).containsKey('mime'), isFalse);
    });
  });
}
