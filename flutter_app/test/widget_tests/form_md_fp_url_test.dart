/// RED test for Markdown / FilePath / Url field kinds.
///
/// - Markdown → multiline `TextField` (no preview pane; render is a
///   separate row, out of scope here).
/// - FilePath → `TextField` + `Icons.folder_open` `IconButton` that
///   asks the [FilePickerBridge] for a path.
/// - Url → `TextField` that surfaces `validateUrl` errors inline.
library;

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

import '../test_helpers/tool_fixture.dart';

import '../test_helpers/i18n_test_catalog.dart';

class _FakeBridge extends FilePickerBridge {
  _FakeBridge(this.pathToReturn, {this.directoryToReturn, this.fileToReturn});
  final String? pathToReturn;
  final String? directoryToReturn;
  final PickedFileData? fileToReturn;

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
  }) async => fileToReturn;

  @override
  Future<List<PickedFileData>?> pickOpenFiles({
    String dialogTitle = 'Pick files',
    List<String>? allowedExtensions,
    bool allowMultiple = false,
    int? maxCount,
    int? maxFileBytes,
    int? maxTotalBytes,
  }) async {
    final selected = fileToReturn;
    return selected == null ? null : [selected];
  }

  @override
  Future<void> writeBytesTo(String path, Uint8List bytes) async {}
  @override
  Future<String?> pickOpenPath() async => pathToReturn;

  @override
  Future<String?> pickDirectoryPath({
    String dialogTitle = 'Pick a folder',
  }) async => directoryToReturn;
}

Widget _formHarness({
  required ToolDto tool,
  required GenericFormController controller,
  required FilePickerBridge bridge,
  required int revision,
}) {
  return ProviderScope(
    overrides: [
      ...i18nTestOverrides,
      filePickerBridgeProvider.overrideWithValue(bridge),
    ],
    child: MaterialApp(
      home: Scaffold(
        body: Column(
          children: [
            Text('revision-$revision'),
            GenericFormWidget(tool: tool, controller: controller),
          ],
        ),
      ),
    ),
  );
}

void main() {
  group('GenericForm Markdown field', () {
    testWidgets('GenericForm_renders_a_markdown_field_as_a_textarea', (
      tester,
    ) async {
      final tool = fixtureToolDto(
        id: 'fixture.md',
        inputFields: const [
          InputFieldDto(
            key: 'doc',
            label: 'Doc',
            fieldType: InputFieldType_Markdown(),
            required_: false,
          ),
        ],
      );
      final controller = GenericFormController();
      await tester.pumpWidget(
        ProviderScope(
          child: MaterialApp(
            home: Scaffold(
              body: GenericFormWidget(tool: tool, controller: controller),
            ),
          ),
        ),
      );
      // The Markdown field renders as a multiline TextField (maxLines: null).
      final inner = tester.widget<TextField>(
        find.descendant(
          of: find.byKey(const Key('field-doc')),
          matching: find.byType(TextField),
        ),
      );
      expect(inner.maxLines, isNull);
    });
  });

  group('GenericForm FilePath field', () {
    testWidgets('GenericForm_renders_a_filepath_field_with_a_picker_button', (
      tester,
    ) async {
      final tool = fixtureToolDto(
        id: 'fixture.fp',
        inputFields: const [
          InputFieldDto(
            key: 'path',
            label: 'Path',
            fieldType: InputFieldType_FilePath(),
            required_: false,
          ),
        ],
      );
      final controller = GenericFormController();
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            ...i18nTestOverrides,
            filePickerBridgeProvider.overrideWithValue(_FakeBridge(null)),
          ],
          child: MaterialApp(
            home: Scaffold(
              body: GenericFormWidget(tool: tool, controller: controller),
            ),
          ),
        ),
      );
      expect(find.byIcon(Icons.folder_open), findsOneWidget);
    });

    testWidgets(
      'GenericForm_filepath_picker_reflects_the_picked_file_into_the_form',
      (tester) async {
        final tool = fixtureToolDto(
          id: 'fixture.fp',
          inputFields: const [
            InputFieldDto(
              key: 'path',
              label: 'Path',
              fieldType: InputFieldType_FilePath(),
              required_: false,
            ),
          ],
        );
        final controller = GenericFormController();
        const String stubPath = '/tmp/picked.txt';
        await tester.pumpWidget(
          ProviderScope(
            overrides: [
              ...i18nTestOverrides,
              filePickerBridgeProvider.overrideWithValue(_FakeBridge(stubPath)),
            ],
            child: MaterialApp(
              home: Scaffold(
                body: GenericFormWidget(tool: tool, controller: controller),
              ),
            ),
          ),
        );
        await tester.tap(find.byKey(const Key('field-path-pick-file')));
        await tester.pumpAndSettle();
        final stored = controller.value('path');
        expect(stored, isA<TextValue>());
        expect((stored as TextValue).value, equals(stubPath));
      },
    );

    testWidgets(
      'GenericForm_filepath_folder_picker_reflects_the_picked_folder',
      (tester) async {
        final tool = fixtureToolDto(
          id: 'fixture.fp.folder',
          inputFields: const [
            InputFieldDto(
              key: 'path',
              label: 'Path',
              fieldType: InputFieldType_FilePath(),
              required_: false,
            ),
          ],
        );
        final controller = GenericFormController();
        const folder = '/tmp/project-folder';
        await tester.pumpWidget(
          ProviderScope(
            overrides: [
              ...i18nTestOverrides,
              filePickerBridgeProvider.overrideWithValue(
                _FakeBridge(null, directoryToReturn: folder),
              ),
            ],
            child: MaterialApp(
              home: Scaffold(
                body: GenericFormWidget(tool: tool, controller: controller),
              ),
            ),
          ),
        );

        await tester.tap(find.byKey(const Key('field-path-pick-directory')));
        await tester.pumpAndSettle();

        expect((controller.value('path') as TextValue).value, folder);
      },
    );

    testWidgets('GenericForm_filepath_keeps_cursor_and_input_across_rebuilds', (
      tester,
    ) async {
      final tool = fixtureToolDto(
        id: 'fixture.fp',
        inputFields: const [
          InputFieldDto(
            key: 'path',
            label: 'Path',
            fieldType: InputFieldType_FilePath(),
            required_: false,
          ),
        ],
      );
      final controller = GenericFormController();
      const pathText = '/tmp/output.txt';
      const cursorOffset = 5;

      await tester.pumpWidget(
        _formHarness(
          tool: tool,
          controller: controller,
          bridge: _FakeBridge(null),
          revision: 1,
        ),
      );
      await tester.tap(find.byKey(const Key('field-path')));
      await tester.enterText(find.byKey(const Key('field-path')), pathText);
      tester.testTextInput.updateEditingValue(
        const TextEditingValue(
          text: pathText,
          selection: TextSelection.collapsed(offset: cursorOffset),
        ),
      );
      await tester.pump();

      await tester.pumpWidget(
        _formHarness(
          tool: tool,
          controller: controller,
          bridge: _FakeBridge(null),
          revision: 2,
        ),
      );
      await tester.pump();

      final editable = tester.widget<EditableText>(
        find.descendant(
          of: find.byKey(const Key('field-path')),
          matching: find.byType(EditableText),
        ),
      );
      expect(editable.controller.text, equals(pathText));
      expect(editable.controller.selection.baseOffset, equals(cursorOffset));
      expect(controller.snapshot()['path'], equals(pathText));
    });
  });

  group('GenericForm File field', () {
    testWidgets(
      'GenericForm_file_picker_reflects_name_and_bytes_not_the_path_into_the_form',
      (tester) async {
        final tool = fixtureToolDto(
          id: 'fixture.file',
          inputFields: const [
            InputFieldDto(
              key: 'input_file',
              label: 'Input file',
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
        final controller = GenericFormController();
        bool? isValid;
        await tester.pumpWidget(
          ProviderScope(
            overrides: [
              ...i18nTestOverrides,
              filePickerBridgeProvider.overrideWithValue(
                _FakeBridge(
                  '/tmp/should-not-be-used.bin',
                  fileToReturn: (
                    name: 'input.bin',
                    bytes: Uint8List.fromList(const <int>[1, 2, 255]),
                    mime: null,
                  ),
                ),
              ),
            ],
            child: MaterialApp(
              home: Scaffold(
                body: GenericFormWidget(
                  tool: tool,
                  controller: controller,
                  onValidationChanged: (value) => isValid = value,
                ),
              ),
            ),
          ),
        );
        await tester.pump();

        expect(find.byKey(FileInputKeys.pickButton), findsOneWidget);
        expect(find.text(i18nEn('modal.file.empty_prompt')), findsOneWidget);
        expect(isValid, isFalse);

        await tester.tap(find.byIcon(Icons.attach_file));
        await tester.pumpAndSettle();

        final stored = controller.value('input_file');
        expect(stored, isA<FileFormValue>());
        final file = (stored as FileFormValue).value;
        expect(file.name, 'input.bin');
        expect(file.isDir, isFalse);
        expect(file.mime, isNull);
        expect(
          file.content,
          CanonicalFileContent.bytes(
            bytes: Uint8List.fromList(const <int>[1, 2, 255]),
          ),
        );
        expect(controller.snapshot()['input_file'], <String, Object?>{
          'name': 'input.bin',
          'is_dir': false,
          'content': <String, Object?>{'kind': 'bytes', 'bytes': 'AQL/'},
        });
        expect(find.text('input.bin'), findsOneWidget);
        expect(isValid, isTrue);
      },
    );
  });

  group('GenericForm Url field', () {
    testWidgets('GenericForm_shows_an_error_for_a_bad_url_in_a_url_field', (
      tester,
    ) async {
      final tool = fixtureToolDto(
        id: 'fixture.url',
        inputFields: const [
          InputFieldDto(
            key: 'link',
            label: 'Link',
            fieldType: InputFieldType_Url(),
            required_: false,
          ),
        ],
      );
      final controller = GenericFormController();
      await tester.pumpWidget(
        ProviderScope(
          overrides: [...i18nTestOverrides],
          child: MaterialApp(
            home: Scaffold(
              body: GenericFormWidget(tool: tool, controller: controller),
            ),
          ),
        ),
      );
      await tester.enterText(
        find.byKey(const Key('field-link')),
        'totally bogus',
      );
      await tester.pump();
      // error message comes from validateUrl (cycle N2 — `'not a url'`).
      expect(find.text('not a url'), findsOneWidget);
    });
  });
}
