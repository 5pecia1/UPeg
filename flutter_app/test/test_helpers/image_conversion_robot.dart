import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/platform/file_picker_bridge.dart';
import 'package:upeg/src/pages/expanded_modal_page.dart';
import 'package:upeg/src/rust/api/i18n.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/file_input_policy.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/state/locale_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/expanded_modal/file_output_card.dart';
import 'package:upeg/src/widgets/expanded_modal/form_value.dart';
import 'package:upeg/src/widgets/expanded_modal/file_input_field.dart';
import 'package:upeg/src/widgets/expanded_modal/generic_form.dart';
import 'package:upeg/src/widgets/expanded_modal/image_conversion_form_policy.dart';
import '../shared/fake_image_file_bridge.dart';
import 'fake_keyboard_resolver.dart';
import 'i18n_test_catalog.dart';
import 'pin_persistence_overrides.dart';
import 'tool_fixture.dart';

class ImageConversionRobot {
  ImageConversionRobot(this.tester);
  final WidgetTester tester;
  final bridge = FakeImageFileBridge();
  final controller = GenericFormController();
  bool? valid;

  Future<void> pumpForm({
    bool svg = false,
    bool heicInput = false,
    bool ai = false,
    bool batch = false,
    double width = 375,
    bool dark = false,
  }) async {
    tester.view.physicalSize = Size(width, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    if (svg || ai) {
      controller.set(
        batch ? 'images' : 'input',
        FileFormValue(
          batch
              ? CanonicalFileValue(
                  name: 'images',
                  isDir: true,
                  content: CanonicalFileContent.directory(
                    entries: [
                      CanonicalFileValue(
                        name: ai ? 'shape.AI' : 'shape.svg',
                        isDir: false,
                        content: CanonicalFileContent.bytes(
                          bytes: Uint8List(0),
                        ),
                      ),
                    ],
                  ),
                )
              : CanonicalFileValue(
                  name: ai ? 'shape.AI' : 'shape.svg',
                  isDir: false,
                  content: CanonicalFileContent.bytes(bytes: Uint8List(0)),
                ),
        ),
      );
    }
    final tool = fixtureToolDto(
      id: batch
          ? ImageConversionFields.batchTool
          : ImageConversionFields.singleTool,
      inputFields: [
        if (heicInput)
          InputFieldDto(
            key: 'input',
            label: 'image',
            fieldType: InputFieldType_File(
              policy: FileInputPolicyDto(
                extensions: ['heic', 'heif'],
                maxCount: 1,
                maxFileBytes: null,
                maxTotalBytes: null,
              ),
            ),
            required_: true,
          ),
        InputFieldDto(
          key: 'output_format',
          label: 'format',
          fieldType: InputFieldType_Select(
            options: [
              ChoiceOptionDto(value: 'png', label: 'PNG'),
              ChoiceOptionDto(value: 'jpeg', label: 'JPEG'),
              ChoiceOptionDto(value: 'svg', label: 'SVG'),
            ],
          ),
          required_: true,
        ),
        InputFieldDto(
          key: 'jpeg_quality',
          label: 'quality',
          fieldType: InputFieldType_Integer(),
          required_: false,
          constraints: FieldConstraintsDto(
            number: NumberConstraintsDto(min: 1, max: 100, default_: 90),
          ),
        ),
        InputFieldDto(
          key: 'background',
          label: 'background',
          fieldType: InputFieldType_Text(),
          required_: false,
          constraints: FieldConstraintsDto(
            string: StringConstraintsDto(
              regex: '^#[0-9a-fA-F]{6}\$',
              default_: '#FFFFFF',
            ),
          ),
        ),
        InputFieldDto(
          key: 'ai_dpi',
          label: 'dpi',
          fieldType: InputFieldType_Integer(),
          required_: false,
          constraints: FieldConstraintsDto(
            number: NumberConstraintsDto(min: 36, max: 600, default_: 144),
          ),
        ),
        InputFieldDto(
          key: 'vector_preset',
          label: 'preset',
          required_: false,
          fieldType: InputFieldType_Select(
            options: [
              ChoiceOptionDto(value: 'poster', label: 'Poster'),
              ChoiceOptionDto(value: 'photo', label: 'Photo'),
              ChoiceOptionDto(value: 'bw', label: 'BW'),
            ],
          ),
        ),
        InputFieldDto(
          key: 'svg_width',
          label: 'width',
          fieldType: InputFieldType_Integer(),
          required_: false,
          constraints: FieldConstraintsDto(
            number: NumberConstraintsDto(min: 0, max: 8192, default_: 0),
          ),
        ),
      ],
    );
    await tester.pumpWidget(
      _scope(
        MaterialApp(
          theme: dark ? UpegTheme.darkTheme() : UpegTheme.lightTheme(),
          home: Scaffold(
            body: SingleChildScrollView(
              child: Padding(
                padding: const EdgeInsets.all(16),
                child: GenericFormWidget(
                  tool: tool,
                  controller: controller,
                  onValidationChanged: (value) => valid = value,
                ),
              ),
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
  }

  Future<void> pumpModal({required bool imageConversion}) async {
    final tool = fixtureToolDto(
      id: imageConversion ? ImageConversionFields.singleTool : 'fixture.tool',
      description: 'Original tool description',
      inputFields: [
        InputFieldDto(
          key: 'value',
          label: 'Value',
          fieldType: InputFieldType_Text(),
          required_: false,
        ),
      ],
    );
    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          ...i18nTestOverrides,
          ...pinPersistenceTestOverrides,
          localeProvider.overrideWithValue(LocaleDto.ko),
          fakeKeyboardResolverOverride,
        ],
        child: MaterialApp(
          theme: UpegTheme.darkTheme(),
          home: ExpandedModalPage(tool: tool),
        ),
      ),
    );
    await tester.pumpAndSettle();
  }

  void expectModalDescription({required bool imageConversion}) {
    expect(
      find.text('Original tool description'),
      imageConversion ? findsNothing : findsOneWidget,
    );
    expect(
      find.textContaining('PDF 호환 AI는 첫 페이지만'),
      imageConversion ? findsOneWidget : findsNothing,
    );
  }

  Widget _scope(Widget child) => ProviderScope(
    overrides: [
      ...i18nTestOverrides,
      localeProvider.overrideWithValue(LocaleDto.ko),
      fakeKeyboardResolverOverride,
      filePickerBridgeProvider.overrideWithValue(bridge),
    ],
    child: child,
  );

  Future<void> selectFormat(String label) async {
    await tester.tap(find.byKey(ImageConversionKeys.format));
    await tester.pumpAndSettle();
    await tester.tap(find.text(label).last);
    await tester.pumpAndSettle();
  }

  Future<void> pickHeic() async {
    bridge.pickedFile = (
      name: 'photo.heic',
      bytes: Uint8List.fromList(const [1, 2, 3]),
      mime: 'image/heic',
    );
    await tester.tap(find.byKey(FileInputKeys.pickButton));
    await tester.pumpAndSettle();
  }

  void expectHeicSubmittedAs(String format) {
    expect(bridge.pickerAllowedExtensions, ['heic', 'heif']);
    expect(bridge.pickerAllowsMultiple, isFalse);
    expect(find.text('photo.heic'), findsOneWidget);
    expect(valid, isTrue);
    final args = controller.snapshot().toJsonObject();
    expect(args['output_format'], format);
    final input = args['input'] as Map<String, Object?>;
    expect(input['name'], 'photo.heic');
    expect((input['content'] as Map<String, Object?>)['bytes'], 'AQID');
  }

  void expectJpegVisible() {
    expect(find.byKey(ImageConversionKeys.quality), findsOneWidget);
    expect(find.byKey(ImageConversionKeys.background), findsOneWidget);
  }

  void expectJpegNotVisible() {
    expect(find.byKey(ImageConversionKeys.quality), findsNothing);
    expect(find.byKey(ImageConversionKeys.background), findsNothing);
  }

  void expectSvgVisible() =>
      expect(find.byKey(ImageConversionKeys.svgWidth), findsOneWidget);
  void expectSvgNotVisible() =>
      expect(find.byKey(ImageConversionKeys.svgWidth), findsNothing);
  Future<void> enterInvalidBackground() async {
    await tester.enterText(find.byKey(ImageConversionKeys.background), 'red');
    await tester.pump();
    expect(valid, isFalse);
  }

  void expectValidWithoutHiddenBackground() {
    expect(valid, isTrue);
    expect(
      controller.snapshot().toJsonObject().containsKey('background'),
      isFalse,
    );
    expect(tester.takeException(), isNull);
  }

  void expectVectorOptions({required bool visible, required bool ai}) {
    expect(
      find.byKey(ImageConversionKeys.vectorPreset),
      visible ? findsOneWidget : findsNothing,
    );
    expect(
      find.byKey(ImageConversionKeys.background),
      visible ? findsOneWidget : findsNothing,
    );
    expect(find.byKey(ImageConversionKeys.quality), findsNothing);
    expect(
      find.byKey(ImageConversionKeys.aiDpi),
      ai ? findsOneWidget : findsNothing,
    );
    final args = controller.snapshot().toJsonObject();
    expect(args['vector_preset'], visible ? 'poster' : null);
    expect(args['background'], visible ? '#FFFFFF' : null);
    expect(args['ai_dpi'], ai ? 144 : null);
    expect(find.textContaining('PDF 호환 AI는 첫 페이지만'), findsOneWidget);
    expect(find.textContaining('벡터화하므로 세부 정보가 손실되거나'), findsOneWidget);
  }

  void expectNoLayoutErrors() => expect(tester.takeException(), isNull);

  Future<void> pumpOutput({
    bool fail = false,
    bool cancel = false,
    bool pending = false,
    String mime = 'image/png',
    String name = 'result.png',
    String? svgContent,
  }) async {
    bridge.failSave = fail;
    bridge.cancelSave = cancel;
    if (pending) bridge.writeGate = Completer<void>();
    await tester.pumpWidget(
      _scope(
        MaterialApp(
          theme: UpegTheme.darkTheme(),
          home: Scaffold(
            body: SizedBox(
              width: 375,
              child: FileOutputCard(
                name: name,
                summary: '$name · 70 B',
                bytes: mime == 'image/svg+xml'
                    ? Uint8List.fromList(
                        utf8.encode(
                          svgContent ??
                              '<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1" viewBox="0 0 1 1"><path d="M0 0 L1 1" fill="#000000"/></svg>',
                        ),
                      )
                    : imagePreviewFixture,
                mime: mime,
                bridge: bridge,
              ),
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
  }

  void expectPreviewVisible() {
    expect(find.byKey(FileOutputKeys.preview), findsOneWidget);
    expect(find.text('미리보기를 지원하지 않습니다. 파일을 저장하여 확인하세요.'), findsNothing);
  }

  void expectPreviewNotVisible() =>
      expect(find.byKey(FileOutputKeys.preview), findsNothing);
  Future<void> save() async {
    await tester.tap(find.byKey(FileOutputKeys.save));
    await tester.pumpAndSettle();
  }

  void expectSaving() {
    expect(
      tester.widget<TextButton>(find.byKey(FileOutputKeys.save)).onPressed,
      isNull,
    );
    expect(bridge.writes, 1);
  }

  Future<void> finishSave() async {
    bridge.writeGate!.complete();
    await tester.pumpAndSettle();
  }

  void expectSaved() {
    expect(find.text('저장했습니다.'), findsOneWidget);
    expect(bridge.savedName, 'result.png');
  }

  void expectSvgUnavailable() {
    expect(find.byKey(FileOutputKeys.preview), findsNothing);
    expect(find.text('미리보기를 지원하지 않습니다. 파일을 저장하여 확인하세요.'), findsOneWidget);
  }

  void expectSvgSaved() {
    expect(bridge.savedName, 'result.svg');
  }

  void expectNotSaved() => expect(find.text('저장했습니다.'), findsNothing);
  void expectSaveFailed() {
    expect(find.text('파일을 저장하지 못했습니다. 저장 위치를 확인하고 다시 시도하세요.'), findsOneWidget);
    expect(
      tester.widget<TextButton>(find.byKey(FileOutputKeys.save)).onPressed,
      isNotNull,
    );
  }

  void expectSaveNotFailed() =>
      expect(find.text('파일을 저장하지 못했습니다. 저장 위치를 확인하고 다시 시도하세요.'), findsNothing);
  Future<void> retrySave() async {
    bridge.failSave = false;
    await save();
  }
}
