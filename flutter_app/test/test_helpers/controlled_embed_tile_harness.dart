/// Shared provider fixtures and robots for Controlled Embed widgets.
library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/embed.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/state/controlled_embed_settings_provider.dart';
import 'package:upeg/src/state/selector_bindings_provider.dart';
import 'package:upeg/src/widgets/controlled_embed/debug_modal.dart';
import 'package:upeg/src/widgets/controlled_embed/runner.dart';
import 'package:upeg/src/widgets/controlled_embed/settings.dart';
import 'package:upeg/src/widgets/controlled_embed/tile.dart';
import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';

import '../shared/fake_controlled_embed_widget.dart';
import 'controlled_embed_runner_harness.dart';
import 'i18n_test_catalog.dart';
export '../shared/fake_controlled_embed_widget.dart';

late ControlledEmbedWidgetFixture controlledEmbedFixture;

void useStubbedControlledEmbedSeams() {
  useStubbedExecutionScripts();
  setUp(() {
    executionScriptsBuilder = eventExecutionScripts;
    bindingWaitScriptBuilder = eventBindingWaitScript;
    controlledEmbedFixture = ControlledEmbedWidgetFixture();
  });
  tearDown(() => controlledEmbedFixture.sessions.close());
}

const kOpaqueObservedError = 'javascript -> ERROR';
const kRecoveredOutput = 'recovered';
const kWebViewNotReady = 'webview not ready';
const kRunButtonKey = Key('controlled-embed-run-btn');
const kDebugButtonKey = Key('controlled-embed-debug-open');
const kErrorKey = Key('controlled-embed-error');
const kDebugLauncherKey = Key('test-controlled-embed-debug-launcher');
const kLongControlledEmbedOutput =
    '첫 줄은 공백이 많은 긴 Controlled Embed 출력입니다 🙂 '
    'wrap 동작을 확인하기 위해 충분히 길게 작성합니다.\n'
    '둘째 줄도 한국어와 emoji 🙂 그리고 spaces 를 포함합니다.';
const kSelectorMissControlledEmbedError =
    'selector miss: #intro; controlled embed output could not be read after trigger';
const kOmittedSettingsDto = ControlledEmbedSettingsDto();

final class ControlledEmbedTileRobot {
  ControlledEmbedTileRobot(this.tester);
  final WidgetTester tester;

  Future<void> pump({
    ToolDto? tool,
    ControlledEmbedSettingsDto settings = kOmittedSettingsDto,
    double width = 800,
    double height = 500,
    ClipboardWriter? clipboard,
  }) async {
    controlledEmbedFixture.settings = resolveBrowserSettings(settings);
    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          ...i18nTestOverrides,
          ...controlledEmbedFixture.overrides,
          selectorBindingsLoaderProvider.overrideWithValue(
            (ToolId _) => controlledEmbedFixture.bindings,
          ),
          controlledEmbedSettingsLoaderProvider.overrideWithValue(
            (ToolId _) => settings,
          ),
          if (clipboard != null)
            clipboardWriterProvider.overrideWithValue(clipboard),
        ],
        child: MaterialApp(
          home: Scaffold(
            body: Align(
              alignment: Alignment.topLeft,
              child: SizedBox(
                width: width,
                height: height,
                child: ControlledEmbedTile(
                  tool: tool ?? fixtureTool(),
                  resolution: fixtureResolution(),
                ),
              ),
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
  }

  Future<void> tapRun() async {
    await tester.tap(find.byKey(kRunButtonKey));
    await tester.pumpAndSettle();
  }

  Future<void> tapDebug() async {
    await tester.tap(find.byKey(kDebugButtonKey));
    await tester.pumpAndSettle();
  }

  Future<void> pressRunShortcut() async {
    await tester.tap(find.byKey(kRunButtonKey));
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.f4);
    await tester.pumpAndSettle();
  }

  Future<void> enterField(String field, String value) async {
    await tester.enterText(find.byKey(Key('field-$field')), value);
    await tester.pumpAndSettle();
  }

  Future<void> copyOutput(String id) async {
    await tester.tap(find.byKey(Key('controlled-embed-output-copy-$id')));
    await tester.pumpAndSettle();
  }

  void expectRunEnabled(bool enabled) {
    expect(
      tester.widget<FilledButton>(find.byKey(kRunButtonKey)).onPressed,
      enabled ? isNotNull : isNull,
    );
  }

  void expectDebugEnabled(bool enabled) {
    expect(
      tester.widget<OutlinedButton>(find.byKey(kDebugButtonKey)).onPressed,
      enabled ? isNotNull : isNull,
    );
  }

  void expectUnsupported(bool visible) {
    expect(
      find.byKey(kControlledEmbedInlineUnsupportedKey),
      visible ? findsOneWidget : findsNothing,
    );
    expect(
      find.byKey(kControlledEmbedOpenExternallyKey),
      visible ? findsOneWidget : findsNothing,
    );
  }

  void expectRunHint() {
    expect(find.text(i18nEn('controlled_embed.press_run')), findsOneWidget);
  }

  void expectErrorContaining(String text) {
    expect(find.byKey(kErrorKey), findsOneWidget);
    expect(find.textContaining(text), findsOneWidget);
  }

  void expectNoTextContaining(String text) {
    expect(find.textContaining(text), findsNothing);
  }

  void expectNoError() {
    expect(find.byKey(kErrorKey), findsNothing);
  }

  void expectOutputContaining(String text) {
    expect(find.byKey(const Key('controlled-embed-outputs')), findsOneWidget);
    expect(find.textContaining(text), findsOneWidget);
  }

  void expectOutputRow({required String label, required String value}) {
    expect(find.byKey(const Key('controlled-embed-outputs')), findsOneWidget);
    expect(find.text(label), findsOneWidget);
    expect(find.text(value), findsOneWidget);
  }

  void expectSelectableOutput(String id, String value) {
    final output = tester.widget<SelectableText>(
      find.byKey(Key('controlled-embed-output-value-$id')),
    );
    expect(output.data, value);
    expect(output.maxLines, isNull);
  }

  void expectSelectableError(String value) {
    final output = tester.widget<SelectableText>(find.byKey(kErrorKey));
    expect(output.data, value);
    expect(output.maxLines, isNull);
  }

  void expectNoFrameworkError() => expect(tester.takeException(), isNull);
}

final class ControlledEmbedDebugRobot {
  ControlledEmbedDebugRobot(this.tester);
  final WidgetTester tester;

  Future<void> pump({
    Map<String, Object?> inputs = const {},
    ResolvedBrowserSettings? settings,
    bool settle = true,
  }) async {
    controlledEmbedFixture.settings = settings;
    await tester.pumpWidget(
      ProviderScope(
        overrides: [...i18nTestOverrides, ...controlledEmbedFixture.overrides],
        child: MaterialApp(
          home: Scaffold(
            body: Builder(
              builder: (context) {
                return TextButton(
                  key: kDebugLauncherKey,
                  onPressed: () => showDialog<void>(
                    context: context,
                    builder: (_) => ControlledEmbedDebugModal(
                      tool: fixtureTool(),
                      resolution: fixtureResolution(),
                      bindings: controlledEmbedFixture.bindings,
                      initialInputs: inputs,
                      resolvedSettings: settings,
                      boardKey: controlledEmbedFixture.boardKey,
                    ),
                  ),
                  child: const Text('Open debugger'),
                );
              },
            ),
          ),
        ),
      ),
    );
    await tester.tap(find.byKey(kDebugLauncherKey));
    if (settle) {
      await tester.pumpAndSettle();
    } else {
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 300));
    }
  }

  Future<void> finishLoading() => tester.pumpAndSettle();

  Future<void> tapRun() async {
    await tester.tap(find.byKey(kDebugRunKey));
    await tester.pumpAndSettle();
  }

  Future<void> tapRerun() async {
    await tester.tap(find.byKey(kDebugRerunKey));
    await tester.pumpAndSettle();
  }

  Future<void> tapClear() async {
    await tester.tap(find.byKey(kDebugClearKey));
    await tester.pumpAndSettle();
  }

  Future<void> tapClose() async {
    await tester.tap(find.byKey(kDebugCloseKey));
    await tester.pumpAndSettle();
  }

  void expectVisible(bool visible) {
    expect(find.byKey(kDebugModalKey), visible ? findsOneWidget : findsNothing);
  }

  void expectPanesVisible() {
    expect(find.byKey(kDebugWebViewPaneKey), findsOneWidget);
    expect(find.byKey(kDebugConsolePaneKey), findsOneWidget);
  }

  void expectRunEnabled(bool enabled) {
    expect(
      tester.widget<ButtonStyleButton>(find.byKey(kDebugRunKey)).onPressed,
      enabled ? isNotNull : isNull,
    );
    expect(
      tester.widget<ButtonStyleButton>(find.byKey(kDebugRerunKey)).onPressed,
      enabled ? isNotNull : isNull,
    );
  }

  void expectInputsContaining(String text) {
    expect(
      find.descendant(
        of: find.byKey(kDebugInputsKey),
        matching: find.textContaining(text),
      ),
      findsOneWidget,
    );
  }

  void expectOutputContaining(String text) {
    expect(
      find.descendant(
        of: find.byKey(kDebugOutputsKey),
        matching: find.textContaining(text),
      ),
      findsOneWidget,
    );
  }

  void expectNoOutput() {
    expect(find.byKey(kDebugOutputsKey), findsNothing);
  }

  void expectConsoleContaining(String text) {
    expect(
      find.descendant(
        of: find.byKey(kDebugConsolePaneKey),
        matching: find.textContaining(text),
      ),
      findsWidgets,
    );
  }
}

ToolDto fixtureTool({List<InputFieldDto> inputFields = const []}) => ToolDto(
  id: 'embed.example_controlled',
  toolkit: 'embed',
  label: 'Example Controlled Embed',
  description: 'Test fixture for ControlledEmbedTile',
  tags: <String>[],
  pinKind: PinKindDto.controlledEmbed,
  pegboardUnits: PegboardUnitsDto.u2,
  invoker: InvokerDto.embed,
  inputFields: inputFields,
  outputFields: <OutputFieldDto>[
    OutputFieldDto(
      key: 'intro',
      label: 'Intro',
      fieldType: OutputFieldType.text(),
    ),
  ],
  source: SourceDto.manual(),
  requiresApproval: false,
  approvalSurfaces: <String>[],
  effect: ToolEffectDto.unknown,
);

EmbedResolutionDto fixtureResolution() =>
    const EmbedResolutionDto(url: 'https://example.test/');

CanonicalToolResult canonicalSuccess(Map<String, String> outputs) {
  final entries = [
    for (final entry in outputs.entries)
      CanonicalOutputEntry(
        id: entry.key,
        kind: kControlledEmbedStringOutputKind,
        value: CanonicalOutputValue.string(value: entry.value),
      ),
  ];
  return CanonicalToolResult(
    ok: true,
    primaryOutputId: entries.isEmpty ? null : entries.first.id,
    outputs: entries,
  );
}

CanonicalToolResult canonicalError(String message) {
  return CanonicalToolResult(
    ok: false,
    outputs: const <CanonicalOutputEntry>[],
    error: CanonicalToolError(
      code: kControlledEmbedExecutionErrorCode,
      message: message,
    ),
  );
}
