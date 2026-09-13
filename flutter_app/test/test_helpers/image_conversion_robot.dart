import 'dart:async';
import 'dart:typed_data';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/platform/file_picker_bridge.dart';
import 'package:upeg/src/rust/api/i18n.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/state/locale_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/expanded_modal/file_output_card.dart';
import 'package:upeg/src/widgets/expanded_modal/form_value.dart';
import 'package:upeg/src/widgets/expanded_modal/generic_form.dart';
import 'package:upeg/src/widgets/expanded_modal/image_conversion_form_policy.dart';
import '../shared/fake_image_file_bridge.dart';
import 'fake_keyboard_resolver.dart';
import 'i18n_test_catalog.dart';
import 'tool_fixture.dart';

class ImageConversionRobot {
  ImageConversionRobot(this.tester);
  final WidgetTester tester;
  final bridge = FakeImageFileBridge();
  final controller = GenericFormController();
  bool? valid;

  Future<void> pumpForm({
    bool svg = false,
    double width = 375,
    bool dark = false,
  }) async {
    tester.view.physicalSize = Size(width, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    if (svg) {
      controller.set(
        'input',
        FileFormValue(
          CanonicalFileValue(
            name: 'shape.svg',
            isDir: false,
            content: CanonicalFileContent.bytes(bytes: Uint8List(0)),
          ),
        ),
      );
    }
    final tool = fixtureToolDto(
      id: ImageConversionFields.singleTool,
      inputFields: const [
        InputFieldDto(
          key: 'output_format',
          label: 'format',
          fieldType: InputFieldType_Select(
            options: [
              ChoiceOptionDto(value: 'png', label: 'PNG'),
              ChoiceOptionDto(value: 'jpeg', label: 'JPEG'),
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

  void expectNoLayoutErrors() => expect(tester.takeException(), isNull);

  Future<void> pumpOutput({
    bool fail = false,
    bool cancel = false,
    bool pending = false,
    String mime = 'image/png',
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
                name: 'result.png',
                summary: 'result.png · 70 B',
                bytes: imagePreviewFixture,
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
