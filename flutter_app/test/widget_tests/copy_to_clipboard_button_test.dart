/// Widget tests for the reusable `CopyToClipboardButton` and the
/// `ClipboardWriter` test seam (Batch K2 / I11).
///
/// The button is mounted by `_OutcomeBlock` on every text outcome
/// (stdout / stderr / error_message). The seam (`ClipboardWriter`)
/// keeps platform-channel `Clipboard.setData` out of the test path —
/// tests use `_RecordingClipboardWriter` instead.
library;

import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/pages/expanded_modal_page.dart';
import 'package:upeg/src/platform/file_picker_bridge.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/rust/canonical_file_value_codec.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';
import 'package:upeg/src/widgets/expanded_modal/structured_output.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';
import 'package:upeg/src/state/dispatch_stream_provider.dart';
import 'package:upeg/src/widgets/expanded_modal/webview_panel.dart'
    show WebViewTarget, desktopWebViewBuilder, webViewTargetResolver;

import '../test_helpers/tool_fixture.dart';

import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/dispatch_stream_fixture.dart';

/// Synchronous fake dispatch; the harness wraps it into the streaming
/// [DispatchStreamFn] seam (via [stubDispatchStream]) so call sites keep
/// returning a plain [CanonicalToolResult].
typedef _SyncDispatch =
    CanonicalToolResult Function({
      required ToolId toolId,
      required ToolArgs args,
    });

class _RecordingClipboardWriter extends ClipboardWriter {
  final List<String> writes = <String>[];

  @override
  Future<void> write(String text) async {
    writes.add(text);
  }
}

class _RecordingFilePickerBridge extends FilePickerBridge {
  String? savePathToReturn;
  final List<String> requestedDefaultNames = <String>[];
  final List<List<int>> savedBytes = <List<int>>[];
  Uint8List? lastSavedBuffer;

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
    lastSavedBuffer = bytes;
    savedBytes.add(bytes.toList(growable: false));
  }
}

final _fixtureTool = fixtureToolDto(
  id: 'fixture.echo',
  toolkit: 'fixture',
  label: 'echo',
  inputFields: const <InputFieldDto>[
    InputFieldDto(
      key: 'value',
      label: 'value',
      fieldType: InputFieldType_Text(),
      required_: true,
    ),
  ],
);

CanonicalToolResult _singleOutputSuccess({
  String id = 'result',
  String? label = 'Result',
  String kind = 'string',
  required CanonicalOutputValue value,
}) {
  return CanonicalToolResult(
    ok: true,
    primaryOutputId: id,
    outputs: [
      CanonicalOutputEntry(id: id, label: label, kind: kind, value: value),
    ],
  );
}

CanonicalToolResult _multiOutputSuccess({
  required String primaryOutputId,
  required List<CanonicalOutputEntry> outputs,
}) {
  return CanonicalToolResult(
    ok: true,
    primaryOutputId: primaryOutputId,
    outputs: outputs,
  );
}

Widget _modalHarness({
  required _SyncDispatch dispatch,
  required ClipboardWriter clipboardWriter,
  ToolDto? tool,
  List<Override> overrides = const <Override>[],
}) {
  return ProviderScope(
    overrides: [
      ...i18nTestOverrides,
      dispatchStreamFnProvider.overrideWithValue(
        stubDispatchStream(
          ({required toolId, required args, required approve}) async =>
              dispatch(toolId: toolId, args: args),
        ),
      ),
      clipboardWriterProvider.overrideWithValue(clipboardWriter),
      ...overrides,
    ],
    child: MaterialApp(
      theme: UpegTheme.darkTheme(),
      home: ExpandedModalPage(tool: tool ?? _fixtureTool),
    ),
  );
}

void main() {
  group('CopyToClipboardButton', () {
    testWidgets('CopyToClipboardButton_탭은_writer에_텍스트를_전달한다', (tester) async {
      final writer = _RecordingClipboardWriter();
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: CopyToClipboardButton(textToCopy: 'hello', writer: writer),
          ),
        ),
      );
      await tester.tap(find.byType(CopyToClipboardButton));
      await tester.pumpAndSettle();
      expect(writer.writes, ['hello']);
    });

    testWidgets('CopyToClipboardButton은_빈_텍스트면_비활성화된다', (tester) async {
      final writer = _RecordingClipboardWriter();
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: CopyToClipboardButton(textToCopy: '', writer: writer),
          ),
        ),
      );
      await tester.tap(find.byType(CopyToClipboardButton));
      await tester.pumpAndSettle();
      expect(writer.writes, isEmpty);
    });
  });

  group('File structured output', () {
    test('File copy 텍스트는 canonical base64가 아닌 raw byte 크기를 표시한다', () {
      const field = OutputFieldDto(
        key: 'file',
        label: 'File',
        fieldType: OutputFieldType_File(),
      );
      const value = <String, Object?>{
        'name': 'report.txt',
        'is_dir': false,
        'mime': 'text/plain',
        'content': <String, Object?>{'kind': 'bytes', 'bytes': 'AQID'},
      };

      expect(
        formatStructuredOutputValue(field, value),
        'report.txt · text/plain · 3 bytes',
      );
    });
  });

  group('ExpandedModalPage 텍스트 outcome', () {
    testWidgets('ExpandedModalPage_텍스트_outcome은_copy_버튼을_노출한다', (tester) async {
      final writer = _RecordingClipboardWriter();
      await tester.pumpWidget(
        _modalHarness(
          dispatch: ({required toolId, required args}) => _singleOutputSuccess(
            value: const CanonicalOutputValue.string(value: '255'),
          ),
          clipboardWriter: writer,
        ),
      );
      await tester.enterText(find.byKey(const Key('field-value')), 'ff');
      await tester.pump();
      await tester.tap(find.byKey(const Key('expanded-modal-run-btn')));
      await tester.pumpAndSettle();
      expect(find.byType(CopyToClipboardButton), findsOneWidget);
    });

    testWidgets(
      'ExpandedModalPage_텍스트_outcome의_copy_버튼은_stdout을_clipboard에_복사한다',
      (tester) async {
        final writer = _RecordingClipboardWriter();
        await tester.pumpWidget(
          _modalHarness(
            dispatch: ({required toolId, required args}) =>
                _singleOutputSuccess(
                  value: const CanonicalOutputValue.string(value: '255'),
                ),
            clipboardWriter: writer,
          ),
        );
        await tester.enterText(find.byKey(const Key('field-value')), 'ff');
        await tester.pump();
        await tester.tap(find.byKey(const Key('expanded-modal-run-btn')));
        await tester.pumpAndSettle();
        await tester.tap(find.byType(CopyToClipboardButton).first);
        await tester.pumpAndSettle();
        expect(writer.writes, ['255']);
      },
    );

    testWidgets(
      'ExpandedModalPage_structured_content_outcome은_copy_버튼을_추가로_노출한다',
      (tester) async {
        final writer = _RecordingClipboardWriter();
        await tester.pumpWidget(
          _modalHarness(
            dispatch: ({required toolId, required args}) =>
                _singleOutputSuccess(
                  kind: 'json',
                  value: const CanonicalOutputValue.json(
                    value: '{"result":255}',
                  ),
                ),
            clipboardWriter: writer,
          ),
        );
        await tester.enterText(find.byKey(const Key('field-value')), 'ff');
        await tester.pump();
        await tester.tap(find.byKey(const Key('expanded-modal-run-btn')));
        await tester.pumpAndSettle();

        expect(find.text('Result'), findsOneWidget);
        expect(find.byType(CopyToClipboardButton), findsOneWidget);

        await tester.tap(find.byType(CopyToClipboardButton).last);
        await tester.pumpAndSettle();
        expect(writer.writes, ['{"result":255}']);
      },
    );

    testWidgets('ExpandedModalPage_structured_content는_outputFields_라벨로_렌더한다', (
      tester,
    ) async {
      final writer = _RecordingClipboardWriter();
      final tool = fixtureToolDto(
        id: 'fixture.hex',
        toolkit: 'fixture',
        label: 'hex',
        inputFields: _fixtureTool.inputFields,
        outputFields: const <OutputFieldDto>[
          OutputFieldDto(
            key: 'result',
            label: 'Decimal',
            fieldType: OutputFieldType_Number(),
          ),
        ],
      );
      await tester.pumpWidget(
        _modalHarness(
          tool: tool,
          dispatch: ({required toolId, required args}) => _singleOutputSuccess(
            value: const CanonicalOutputValue.number(value: 255),
          ),
          clipboardWriter: writer,
        ),
      );

      await tester.enterText(find.byKey(const Key('field-value')), 'ff');
      await tester.pump();
      await tester.tap(find.byKey(const Key('expanded-modal-run-btn')));
      await tester.pumpAndSettle();

      expect(find.text('Decimal'), findsOneWidget);
      expect(find.text('structured_content'), findsNothing);
    });

    testWidgets('ExpandedModalPage_multi_options_output은_쉼표로_구분해_렌더한다', (
      tester,
    ) async {
      final writer = _RecordingClipboardWriter();
      final tool = fixtureToolDto(
        id: 'fixture.multi',
        toolkit: 'fixture',
        label: 'multi',
        inputFields: _fixtureTool.inputFields,
        outputFields: const <OutputFieldDto>[
          OutputFieldDto(
            key: 'choices',
            label: 'Choices',
            fieldType: OutputFieldType_MultiOptions(
              options: <String>['alpha', 'beta'],
            ),
          ),
        ],
      );
      await tester.pumpWidget(
        _modalHarness(
          tool: tool,
          dispatch: ({required toolId, required args}) => _singleOutputSuccess(
            id: 'choices',
            label: 'Choices',
            kind: 'multi_options',
            value: const CanonicalOutputValue.multiOptions(
              value: <String>['alpha', 'beta'],
            ),
          ),
          clipboardWriter: writer,
        ),
      );

      await tester.enterText(find.byKey(const Key('field-value')), 'ff');
      await tester.pump();
      await tester.tap(find.byKey(const Key('expanded-modal-run-btn')));
      await tester.pumpAndSettle();

      expect(find.text('Choices'), findsOneWidget);
      expect(find.text('alpha, beta'), findsOneWidget);
    });

    testWidgets('ExpandedModalPage_choice_boolean_output은_타입별_칩으로_렌더한다', (
      tester,
    ) async {
      final writer = _RecordingClipboardWriter();
      final tool = fixtureToolDto(
        id: 'fixture.typed',
        toolkit: 'fixture',
        label: 'typed',
        inputFields: _fixtureTool.inputFields,
        outputFields: const <OutputFieldDto>[
          OutputFieldDto(
            key: 'mode',
            label: 'Mode',
            fieldType: OutputFieldType_Select(
              options: <String>['fast', 'safe'],
            ),
          ),
          OutputFieldDto(
            key: 'flags',
            label: 'Flags',
            fieldType: OutputFieldType_MultiOptions(
              options: <String>['alpha', 'beta', 'gamma'],
            ),
          ),
          OutputFieldDto(
            key: 'ok',
            label: 'Accepted',
            fieldType: OutputFieldType_Boolean(),
          ),
        ],
      );
      await tester.pumpWidget(
        _modalHarness(
          tool: tool,
          dispatch: ({required toolId, required args}) => _multiOutputSuccess(
            primaryOutputId: 'mode',
            outputs: const [
              CanonicalOutputEntry(
                id: 'mode',
                label: 'Mode',
                kind: 'options',
                value: CanonicalOutputValue.options(value: 'safe'),
              ),
              CanonicalOutputEntry(
                id: 'flags',
                label: 'Flags',
                kind: 'multi_options',
                value: CanonicalOutputValue.multiOptions(
                  value: <String>['alpha', 'gamma'],
                ),
              ),
              CanonicalOutputEntry(
                id: 'ok',
                label: 'Accepted',
                kind: 'boolean',
                value: CanonicalOutputValue.boolean(value: true),
              ),
            ],
          ),
          clipboardWriter: writer,
        ),
      );

      await tester.enterText(find.byKey(const Key('field-value')), 'ff');
      await tester.pump();
      await tester.tap(find.byKey(const Key('expanded-modal-run-btn')));
      await tester.pumpAndSettle();

      expect(
        find.byKey(const Key('structured-output-select-safe')),
        findsOneWidget,
      );
      expect(
        find.byKey(const Key('structured-output-multi-alpha')),
        findsOneWidget,
      );
      expect(
        find.byKey(const Key('structured-output-multi-gamma')),
        findsOneWidget,
      );
      expect(
        find.byKey(const Key('structured-output-boolean-true')),
        findsOneWidget,
      );
    });

    testWidgets(
      'ExpandedModalPage의 File, URL, EmbeddedView 결과는 host 플랫폼과 무관하게 전용 UI로 렌더링된다',
      (tester) async {
        final writer = _RecordingClipboardWriter();
        final bridge = _RecordingFilePickerBridge()
          ..savePathToReturn = '/tmp/report.txt';
        final sourceBytes = Uint8List.fromList(<int>[1, 2, 3]);
        final originalBuilder = desktopWebViewBuilder;
        final originalTarget = webViewTargetResolver;
        desktopWebViewBuilder = (url, userAgent, onControllerReady) =>
            SizedBox.expand(key: const Key('webview-panel-inappwebview'));
        // Pin the target so this StructuredOutput affordance test stays
        // independent of host-specific webview selection.
        webViewTargetResolver = () => const WebViewTarget.inAppWebView();
        addTearDown(() {
          desktopWebViewBuilder = originalBuilder;
          webViewTargetResolver = originalTarget;
        });
        final tool = fixtureToolDto(
          id: 'fixture.assets',
          toolkit: 'fixture',
          label: 'assets',
          inputFields: _fixtureTool.inputFields,
          outputFields: const <OutputFieldDto>[
            OutputFieldDto(
              key: 'path',
              label: 'Path',
              fieldType: OutputFieldType_FilePath(),
            ),
            OutputFieldDto(
              key: 'link',
              label: 'Link',
              fieldType: OutputFieldType_Url(),
            ),
            OutputFieldDto(
              key: 'file',
              label: 'File',
              fieldType: OutputFieldType_File(),
            ),
            OutputFieldDto(
              key: 'embed',
              label: 'Embed',
              fieldType: OutputFieldType_EmbeddedView(
                url: 'https://example.com/embed',
              ),
            ),
          ],
        );
        await tester.pumpWidget(
          _modalHarness(
            tool: tool,
            dispatch: ({required toolId, required args}) => _multiOutputSuccess(
              primaryOutputId: 'path',
              outputs: [
                const CanonicalOutputEntry(
                  id: 'path',
                  label: 'Path',
                  kind: 'file_path',
                  value: CanonicalOutputValue.filePath(
                    value: '/tmp/report.txt',
                  ),
                ),
                const CanonicalOutputEntry(
                  id: 'link',
                  label: 'Link',
                  kind: 'url',
                  value: CanonicalOutputValue.url(
                    value: 'https://example.com/report',
                  ),
                ),
                CanonicalOutputEntry(
                  id: 'file',
                  label: 'File',
                  kind: 'file',
                  value: CanonicalOutputValue.file(
                    value: CanonicalFileValue(
                      name: 'report.txt',
                      isDir: false,
                      mime: 'text/plain',
                      content: CanonicalFileContent.bytes(bytes: sourceBytes),
                    ),
                  ),
                ),
                const CanonicalOutputEntry(
                  id: 'embed',
                  label: 'Embed',
                  kind: 'embedded_view',
                  value: CanonicalOutputValue.embeddedView(
                    value: 'https://example.com/embed',
                  ),
                ),
              ],
            ),
            clipboardWriter: writer,
            overrides: [filePickerBridgeProvider.overrideWithValue(bridge)],
          ),
        );

        await tester.enterText(find.byKey(const Key('field-value')), 'ff');
        await tester.pump();
        await tester.tap(find.byKey(const Key('expanded-modal-run-btn')));
        await tester.pumpAndSettle();

        expect(
          find.byKey(const Key('structured-output-file-path')),
          findsOneWidget,
        );
        expect(
          find.byKey(const Key('structured-output-url-open')),
          findsOneWidget,
        );
        expect(
          find.byKey(const Key('structured-output-file-summary')),
          findsOneWidget,
        );
        expect(
          find.byKey(const Key('structured-output-file-save')),
          findsOneWidget,
        );
        await tester.ensureVisible(
          find.byKey(const Key('structured-output-file-save')),
        );
        await tester.pumpAndSettle();
        await tester.tap(find.byKey(const Key('structured-output-file-save')));
        await tester.pumpAndSettle();
        expect(bridge.requestedDefaultNames, ['report.txt']);
        expect(bridge.savedBytes, [
          [1, 2, 3],
        ]);
        expect(identical(bridge.lastSavedBuffer, sourceBytes), isTrue);
        expect(
          find.byKey(const Key('webview-panel-inappwebview')),
          findsOneWidget,
        );
      },
    );

    testWidgets('File 출력 렌더링은 typed 값에 codec을 적용하지 않는다', (tester) async {
      final writer = _RecordingClipboardWriter();
      final tool = fixtureToolDto(
        id: 'fixture.directory',
        toolkit: 'fixture',
        label: 'directory',
        inputFields: _fixtureTool.inputFields,
        outputFields: const <OutputFieldDto>[
          OutputFieldDto(
            key: 'file',
            label: 'File',
            fieldType: OutputFieldType_File(),
          ),
        ],
      );
      final entries = List<CanonicalFileValue>.generate(
        canonicalFileMaximumNodes,
        (index) => CanonicalFileValue(
          name: 'entry-$index.bin',
          isDir: false,
          content: CanonicalFileContent.bytes(bytes: Uint8List(0)),
        ),
        growable: false,
      );

      await tester.pumpWidget(
        _modalHarness(
          tool: tool,
          dispatch: ({required toolId, required args}) => _singleOutputSuccess(
            id: 'file',
            label: 'File',
            kind: 'file',
            value: CanonicalOutputValue.file(
              value: CanonicalFileValue(
                name: 'bundle',
                isDir: true,
                content: CanonicalFileContent.directory(entries: entries),
              ),
            ),
          ),
          clipboardWriter: writer,
        ),
      );

      await tester.enterText(find.byKey(const Key('field-value')), 'value');
      await tester.pump();
      await tester.tap(find.byKey(const Key('expanded-modal-run-btn')));
      await tester.pumpAndSettle();

      expect(
        find.text('bundle · directory · $canonicalFileMaximumNodes entries'),
        findsOneWidget,
      );
      expect(tester.takeException(), isNull);
    });

    testWidgets(
      'ExpandedModalPage_markdown_json_datetime_output은_전용_renderer로_렌더한다',
      (tester) async {
        final writer = _RecordingClipboardWriter();
        final tool = fixtureToolDto(
          id: 'fixture.rich',
          toolkit: 'fixture',
          label: 'rich',
          inputFields: _fixtureTool.inputFields,
          outputFields: const <OutputFieldDto>[
            OutputFieldDto(
              key: 'summary',
              label: 'Summary',
              fieldType: OutputFieldType_Markdown(),
            ),
            OutputFieldDto(
              key: 'payload',
              label: 'Payload',
              fieldType: OutputFieldType_Json(),
            ),
            OutputFieldDto(
              key: 'when',
              label: 'When',
              fieldType: OutputFieldType_DateTime(),
            ),
          ],
        );
        await tester.pumpWidget(
          _modalHarness(
            tool: tool,
            dispatch: ({required toolId, required args}) => _multiOutputSuccess(
              primaryOutputId: 'summary',
              outputs: const [
                CanonicalOutputEntry(
                  id: 'summary',
                  label: 'Summary',
                  kind: 'markdown',
                  value: CanonicalOutputValue.markdown(
                    value: '# Title\n- one\n```txt\ncode\n```',
                  ),
                ),
                CanonicalOutputEntry(
                  id: 'payload',
                  label: 'Payload',
                  kind: 'json',
                  value: CanonicalOutputValue.json(
                    value: '{"ok":true,"count":2}',
                  ),
                ),
                CanonicalOutputEntry(
                  id: 'when',
                  label: 'When',
                  kind: 'date_time',
                  value: CanonicalOutputValue.dateTime(
                    value: '2026-05-25T10:30:00.000Z',
                  ),
                ),
              ],
            ),
            clipboardWriter: writer,
          ),
        );

        await tester.enterText(find.byKey(const Key('field-value')), 'ff');
        await tester.pump();
        await tester.tap(find.byKey(const Key('expanded-modal-run-btn')));
        await tester.pumpAndSettle();

        expect(
          find.byKey(const Key('structured-output-markdown')),
          findsOneWidget,
        );
        expect(
          find.byKey(const Key('structured-output-markdown-heading')),
          findsOneWidget,
        );
        expect(
          find.byKey(const Key('structured-output-markdown-list-item')),
          findsOneWidget,
        );
        expect(
          find.byKey(const Key('structured-output-markdown-code')),
          findsOneWidget,
        );
        expect(find.byKey(const Key('structured-output-json')), findsOneWidget);
        expect(
          find.textContaining('"ok": true', findRichText: true),
          findsOneWidget,
        );
        expect(
          find.byKey(const Key('structured-output-datetime')),
          findsOneWidget,
        );
        expect(find.textContaining('2026-05-25T10:30:00.000Z'), findsOneWidget);
      },
    );
  });
}
