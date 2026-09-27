import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/keyboard/keyboard_command_resolver.dart';
import 'package:upeg/src/pages/embed_page.dart';
import 'package:upeg/src/pages/expanded_modal_page.dart';
import 'package:upeg/src/rust/api/embed.dart';
import 'package:upeg/src/rust/api/keyboard.dart';
import 'package:upeg/src/rust/api/palette.dart';
import 'package:upeg/src/rust/api/pegboard.dart' show BoardDto;
import 'package:upeg/src/rust/api/pin_activation.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/embed_resolver_provider.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/state/pin_activation_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';
import 'package:upeg/src/widgets/expanded_modal/webview_panel.dart';
import 'package:upeg/src/widgets/palette_overlay.dart';
import 'package:upeg/src/state/dispatch_stream_provider.dart';

import '../test_helpers/fake_keyboard_resolver.dart';
import '../test_helpers/tool_fixture.dart';
import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/dispatch_stream_fixture.dart';

/// Synchronous fake dispatch used by the harnesses below; the harness
/// wraps it into the streaming [DispatchStreamFn] seam (via
/// [stubDispatchStream]) so call sites keep returning a plain
/// [CanonicalToolResult].
typedef _SyncDispatch =
    CanonicalToolResult Function({
      required ToolId toolId,
      required ToolArgs args,
    });

final _fixtureTool = fixtureToolDto(
  id: 'fixture.echo',
  toolkit: 'fixture',
  label: 'echo',
  description: 'echoes input',
  inputFields: const [
    InputFieldDto(
      key: 'value',
      label: 'value',
      fieldType: InputFieldType_Text(),
      required_: true,
    ),
  ],
  outputFields: const [
    OutputFieldDto(
      key: 'result',
      label: 'Result',
      fieldType: OutputFieldType_Text(),
    ),
  ],
);

const _emptySuccess = CanonicalToolResult(ok: true, outputs: []);

CanonicalToolResult _textSuccess(String value, {String id = 'result'}) {
  return CanonicalToolResult(
    ok: true,
    primaryOutputId: id,
    outputs: [
      CanonicalOutputEntry(
        id: id,
        label: 'Result',
        kind: 'string',
        value: CanonicalOutputValue.string(value: value),
      ),
    ],
  );
}

final _toollessTool = fixtureToolDto(
  id: 'fixture.no_args',
  toolkit: 'fixture',
  label: 'no args',
  description: 'runs without fields',
  inputFields: const [],
);

final _uuidV7Tool = fixtureToolDto(
  id: 'id.uuid_v7',
  toolkit: 'id',
  label: 'uuid v7',
  description: 'generates uuid v7',
  inputFields: const [],
);

final _urlLessEmbedTool = fixtureToolDto(
  id: 'embed.urlless_palette',
  toolkit: 'embed',
  label: 'URL-less Embed',
  description: 'embed without URL',
  pinKind: PinKindDto.embed,
  invoker: InvokerDto.embed,
  pegboardUnits: PegboardUnitsDto.u2,
);

final _urlBackedEmbedTool = fixtureToolDto(
  id: 'embed.url_palette',
  toolkit: 'embed',
  label: 'URL-backed Embed',
  description: 'embed with URL',
  pinKind: PinKindDto.embed,
  invoker: InvokerDto.embed,
  pegboardUnits: PegboardUnitsDto.u2,
);

PaletteHit _paletteHitFor(ToolDto tool) => PaletteHit(
  id: tool.id,
  label: tool.label,
  description: tool.description,
  score: 1,
  pinKind: tool.pinKind,
);

class _RecordingClipboardWriter extends ClipboardWriter {
  String? text;

  @override
  Future<void> write(String text) async {
    this.text = text;
  }
}

final class _SeededCurrentBoard extends CurrentBoardNotifier {
  _SeededCurrentBoard(this.boardKey);

  final BoardKey boardKey;

  @override
  BoardKey? build() => boardKey;
}

Widget _harness({
  required _SyncDispatch dispatch,
  SettingsOverlayLauncher? settingsLauncher,
  ClipboardWriter? clipboardWriter,
  KeyboardCommandResolver? keyboardResolver,
  ToolDto? tool,
  List<ToolDto>? tools,
  List<PaletteHit> paletteHits = const <PaletteHit>[],
  PinActivationFn? activation,
  BoardKey? currentBoardKey,
  AddPinMutator? addPin,
}) {
  final ToolDto effectiveTool = tool ?? _fixtureTool;
  final catalog = tools ?? <ToolDto>[effectiveTool];
  return ProviderScope(
    overrides: [
      dispatchStreamFnProvider.overrideWithValue(
        stubDispatchStream(
          ({required toolId, required args, required approve}) async =>
              dispatch(toolId: toolId, args: args),
        ),
      ),
      ...i18nTestOverrides,
      boardsLoaderProvider.overrideWith(
        (ref) =>
            () => const <BoardDto>[BoardDto(key: 'dev', title: 'Dev')],
      ),
      if (currentBoardKey != null)
        currentBoardKeyProvider.overrideWith(
          () => _SeededCurrentBoard(currentBoardKey),
        ),
      if (addPin != null) addPinMutatorProvider.overrideWithValue(addPin),
      if (keyboardResolver == null)
        fakeKeyboardResolverOverride
      else
        keyboardCommandResolverProvider.overrideWithValue(keyboardResolver),
      toolsLoaderProvider.overrideWith(
        (ref) =>
            () => catalog,
      ),
      paletteSearcherProvider.overrideWith(
        (ref) =>
            (String _) => paletteHits,
      ),
      pinnedBoardsLoaderProvider.overrideWith(
        (ref) =>
            (ToolId toolId) => const <BoardKey>{},
      ),
      pinActivationProvider.overrideWith(
        (ref) =>
            activation ??
            ({required toolId, required argsJson}) =>
                PinActivationDto_OpenModal(toolId: toolId.value),
      ),
      resolveEmbedFnProvider.overrideWith(
        (ref) =>
            ({pinKey, required toolId, required args}) =>
                const EmbedResolutionDto(url: 'https://example.test/embed'),
      ),
      if (settingsLauncher != null)
        settingsOverlayLauncherProvider.overrideWithValue(settingsLauncher),
      if (clipboardWriter != null)
        clipboardWriterProvider.overrideWithValue(clipboardWriter),
    ],
    child: MaterialApp(
      theme: UpegTheme.darkTheme(),
      home: ExpandedModalPage(tool: effectiveTool),
    ),
  );
}

Finder _expandedModalFocusFinder() {
  return find.byWidgetPredicate(
    (widget) =>
        widget is Focus && widget.autofocus && widget.onKeyEvent != null,
  );
}

Widget _routeHarness({required _SyncDispatch dispatch}) {
  return ProviderScope(
    overrides: [
      ...i18nTestOverrides,
      dispatchStreamFnProvider.overrideWithValue(
        stubDispatchStream(
          ({required toolId, required args, required approve}) async =>
              dispatch(toolId: toolId, args: args),
        ),
      ),
      resolveEmbedFnProvider.overrideWithValue(
        ({pinKey, required toolId, required args}) => null,
      ),
      fakeKeyboardResolverOverride,
    ],
    child: MaterialApp(
      theme: UpegTheme.darkTheme(),
      home: Builder(
        builder: (context) => Scaffold(
          body: Center(
            child: ElevatedButton(
              key: const Key('open-expanded-modal-route'),
              onPressed: () => ExpandedModalPage.open(context, _fixtureTool),
              child: const Text('open'),
            ),
          ),
        ),
      ),
    ),
  );
}

void main() {
  group('ExpandedModalPage', () {
    testWidgets('ExpandedModalPage_shows_the_tool_header_and_run_button', (
      tester,
    ) async {
      await tester.pumpWidget(
        _harness(
          dispatch: ({pinKey, required toolId, required args}) {
            return _emptySuccess;
          },
        ),
      );

      expect(find.text('fixture.echo'), findsOneWidget);
      expect(find.text('echo'), findsOneWidget);
      expect(find.text('echoes input'), findsOneWidget);
      expect(find.byKey(const Key('expanded-modal-run-btn')), findsOneWidget);
      expect(find.byKey(const Key('generic-form')), findsOneWidget);
      expect(find.byKey(const Key('expanded-modal-close-btn')), findsOneWidget);
    });

    testWidgets(
      'tapping_the_Run_button_calls_dispatchTool_and_shows_the_result',
      (tester) async {
        String? capturedToolId;
        String? capturedArgs;
        await tester.pumpWidget(
          _harness(
            dispatch: ({pinKey, required toolId, required args}) {
              capturedToolId = toolId.value;
              capturedArgs = args.encodeJson();
              return _textSuccess('255');
            },
          ),
        );

        await tester.enterText(find.byKey(const Key('field-value')), '0xff');
        await tester.pump();
        await tester.tap(find.byKey(const Key('expanded-modal-run-btn')));
        await tester.pumpAndSettle();

        expect(capturedToolId, equals('fixture.echo'));
        expect(capturedArgs, contains('"value":"0xff"'));
        expect(find.byKey(const Key('expanded-modal-outcome')), findsOneWidget);
        expect(
          find.byKey(const ValueKey('expanded-modal-output-result')),
          findsOneWidget,
        );
        expect(find.text('Result'), findsOneWidget);
        expect(find.text('255'), findsOneWidget);
        expect(find.text('primary'), findsOneWidget);
        expect(find.text('ok'), findsOneWidget);
      },
    );

    testWidgets('ExpandedModalPage_F1_runs_the_generic_form', (tester) async {
      String? capturedToolId;
      String? capturedArgs;
      await tester.pumpWidget(
        _harness(
          dispatch: ({pinKey, required toolId, required args}) {
            capturedToolId = toolId.value;
            capturedArgs = args.encodeJson();
            return _emptySuccess;
          },
        ),
      );

      await tester.enterText(find.byKey(const Key('field-value')), '0xff');
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.f1);
      await tester.pumpAndSettle();

      expect(capturedToolId, 'fixture.echo');
      expect(capturedArgs, contains('"value":"0xff"'));
    });

    testWidgets('ExpandedModalPage_Enter_runs_a_valid_generic_form', (
      tester,
    ) async {
      String? capturedArgs;
      await tester.pumpWidget(
        _harness(
          dispatch: ({pinKey, required toolId, required args}) {
            capturedArgs = args.encodeJson();
            return _emptySuccess;
          },
        ),
      );

      await tester.enterText(find.byKey(const Key('field-value')), 'hello');
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();

      expect(capturedArgs, contains('"value":"hello"'));
    });

    testWidgets('ExpandedModalPage_F2_copies_the_latest_generic_output', (
      tester,
    ) async {
      final clipboard = _RecordingClipboardWriter();
      await tester.pumpWidget(
        _harness(
          clipboardWriter: clipboard,
          dispatch: ({pinKey, required toolId, required args}) {
            return _textSuccess('copied output');
          },
        ),
      );

      await tester.enterText(find.byKey(const Key('field-value')), 'hello');
      await tester.pump();
      await tester.tap(find.byKey(const Key('expanded-modal-run-btn')));
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.f2);
      await tester.pump();

      expect(clipboard.text, 'copied output');
    });

    testWidgets(
      'ExpandedModalPage_Escape_closes_the_modal_route_without_a_transition',
      (tester) async {
        await tester.pumpWidget(
          _routeHarness(
            dispatch: ({pinKey, required toolId, required args}) =>
                _emptySuccess,
          ),
        );

        await tester.tap(find.byKey(const Key('open-expanded-modal-route')));
        await tester.pump();
        expect(find.byType(ExpandedModalPage), findsOneWidget);

        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await tester.pump();

        expect(find.byType(ExpandedModalPage), findsNothing);
      },
    );

    testWidgets('ExpandedModalPage_r_runs_when_text_is_not_focused', (
      tester,
    ) async {
      String? capturedToolId;
      await tester.pumpWidget(
        _harness(
          tool: _toollessTool,
          dispatch: ({pinKey, required toolId, required args}) {
            capturedToolId = toolId.value;
            return _emptySuccess;
          },
        ),
      );

      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.keyR);
      await tester.pumpAndSettle();

      expect(capturedToolId, 'fixture.no_args');
    });

    testWidgets(
      'ExpandedModalPage_q_closes_the_route_without_a_transition_when_text_is_not_focused',
      (tester) async {
        await tester.pumpWidget(
          _routeHarness(
            dispatch: ({pinKey, required toolId, required args}) =>
                _emptySuccess,
          ),
        );

        await tester.tap(find.byKey(const Key('open-expanded-modal-route')));
        await tester.pump();
        expect(find.byType(ExpandedModalPage), findsOneWidget);

        await tester.sendKeyEvent(LogicalKeyboardKey.keyQ);
        await tester.pump();

        expect(find.byType(ExpandedModalPage), findsNothing);
      },
    );

    testWidgets(
      'ExpandedModalPage_r_q_are_not_treated_as_aliases_while_a_text_field_is_focused',
      (tester) async {
        var dispatchCount = 0;
        await tester.pumpWidget(
          _harness(
            dispatch: ({pinKey, required toolId, required args}) {
              dispatchCount += 1;
              return _emptySuccess;
            },
          ),
        );

        await tester.showKeyboard(find.byKey(const Key('field-value')));
        await tester.sendKeyEvent(LogicalKeyboardKey.keyR);
        await tester.sendKeyEvent(LogicalKeyboardKey.keyQ);
        await tester.pumpAndSettle();

        expect(dispatchCount, 0);
        expect(find.byType(ExpandedModalPage), findsOneWidget);
      },
    );

    testWidgets('ExpandedModalPage_s_opens_Settings_when_text_is_not_focused', (
      tester,
    ) async {
      var openedSettings = false;
      await tester.pumpWidget(
        _harness(
          tool: _toollessTool,
          settingsLauncher: (_) async {
            openedSettings = true;
          },
          dispatch: ({pinKey, required toolId, required args}) => _emptySuccess,
        ),
      );

      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.keyS);
      await tester.pumpAndSettle();

      expect(openedSettings, isTrue);
    });

    testWidgets(
      'ExpandedModalPage_s_does_not_open_Settings_while_a_text_field_is_focused',
      (tester) async {
        var openedSettings = false;
        await tester.pumpWidget(
          _harness(
            settingsLauncher: (_) async {
              openedSettings = true;
            },
            dispatch: ({pinKey, required toolId, required args}) =>
                _emptySuccess,
          ),
        );

        await tester.showKeyboard(find.byKey(const Key('field-value')));
        await tester.sendKeyEvent(LogicalKeyboardKey.keyS);
        await tester.pumpAndSettle();

        expect(openedSettings, isFalse);
        expect(find.byType(ExpandedModalPage), findsOneWidget);
      },
    );

    // `id.uuid_v7` lost its bespoke form: a zero-input generator is just
    // the generic path (Run button → result panel → copy), so this pins
    // that the generic path really covers it end to end.
    testWidgets(
      'ExpandedModalPage_runs_an_inputless_tool_via_the_generic_run_button_and_can_copy_the_result',
      (tester) async {
        const generated = '01900000-0000-7000-8000-000000000abc';
        final clipboard = _RecordingClipboardWriter();

        await tester.pumpWidget(
          _harness(
            tool: _uuidV7Tool,
            clipboardWriter: clipboard,
            dispatch: ({pinKey, required toolId, required args}) {
              expect(toolId.value, 'id.uuid_v7');
              expect(args.isEmpty, isTrue);
              return _textSuccess(generated);
            },
          ),
        );

        await tester.tap(find.byKey(const Key('expanded-modal-run-btn')));
        await tester.pumpAndSettle();

        final outcome = find.byKey(const Key('expanded-modal-outcome'));
        expect(outcome, findsOneWidget);
        expect(
          find.descendant(of: outcome, matching: find.text(generated)),
          findsOneWidget,
        );

        await tester.tap(
          find.descendant(
            of: outcome,
            matching: find.byType(CopyToClipboardButton),
          ),
        );
        await tester.pumpAndSettle();
        expect(clipboard.text, generated);
      },
    );

    testWidgets(
      'ExpandedModalPage_slash_opens_the_palette_when_text_is_not_focused',
      (tester) async {
        await tester.pumpWidget(
          _harness(
            tool: _toollessTool,
            dispatch: ({pinKey, required toolId, required args}) =>
                _emptySuccess,
          ),
        );

        await tester.pump();
        expect(find.byType(PaletteOverlay), findsNothing);

        await tester.sendKeyEvent(LogicalKeyboardKey.slash);
        await tester.pumpAndSettle();

        expect(find.byType(PaletteOverlay), findsOneWidget);
      },
    );

    testWidgets(
      'ExpandedModalPage_slash_does_not_open_the_palette_while_a_text_field_is_focused',
      (tester) async {
        await tester.pumpWidget(
          _harness(
            dispatch: ({pinKey, required toolId, required args}) =>
                _emptySuccess,
          ),
        );

        await tester.showKeyboard(find.byKey(const Key('field-value')));
        await tester.sendKeyEvent(LogicalKeyboardKey.slash);
        await tester.pumpAndSettle();

        expect(find.byType(PaletteOverlay), findsNothing);
        expect(find.byType(ExpandedModalPage), findsOneWidget);
      },
    );

    testWidgets(
      'ExpandedModalPage_CtrlK_opens_the_palette_even_while_a_text_field_is_focused',
      (tester) async {
        await tester.pumpWidget(
          _harness(
            dispatch: ({pinKey, required toolId, required args}) =>
                _emptySuccess,
          ),
        );

        await tester.showKeyboard(find.byKey(const Key('field-value')));
        await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
        await tester.sendKeyEvent(LogicalKeyboardKey.keyK);
        await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
        await tester.pumpAndSettle();

        expect(find.byType(PaletteOverlay), findsOneWidget);
      },
    );

    testWidgets(
      'picking_and_pinning_a_palette_tool_opens_its_modal_for_the_new_pin',
      (tester) async {
        final boardKey = BoardKey.parse('dev');
        final pinKey = (boardKey, PinId.parse('new-pin'));
        await tester.pumpWidget(
          _harness(
            tool: _toollessTool,
            tools: <ToolDto>[_toollessTool, _fixtureTool],
            paletteHits: <PaletteHit>[_paletteHitFor(_fixtureTool)],
            currentBoardKey: boardKey,
            addPin: (board, tool) {
              expect(board, boardKey);
              expect(tool, ToolId.parse(_fixtureTool.id));
              return pinKey.$2.value;
            },
            dispatch: ({pinKey, required toolId, required args}) =>
                _emptySuccess,
          ),
        );

        await tester.sendKeyEvent(LogicalKeyboardKey.slash);
        await tester.pumpAndSettle();
        await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
        await tester.sendKeyEvent(LogicalKeyboardKey.enter);
        await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
        await tester.pumpAndSettle();

        final pages = tester.widgetList<ExpandedModalPage>(
          find.byType(ExpandedModalPage),
        );
        expect(pages, hasLength(2));
        expect(pages.last.tool.id, _fixtureTool.id);
        expect(pages.last.pinKey, pinKey);
      },
    );

    testWidgets(
      'picking_an_embed_without_a_URL_in_the_Palette_opens_a_nested_modal',
      (tester) async {
        await tester.pumpWidget(
          _harness(
            dispatch: ({pinKey, required toolId, required args}) =>
                _emptySuccess,
            tools: [_fixtureTool, _urlLessEmbedTool],
            paletteHits: [_paletteHitFor(_urlLessEmbedTool)],
            activation: ({required toolId, required argsJson}) =>
                PinActivationDto_OpenModal(toolId: toolId.value),
          ),
        );

        await tester.sendKeyEvent(LogicalKeyboardKey.slash);
        await tester.pumpAndSettle();
        await tester.tap(
          find.byKey(const ValueKey('palette-hit-embed.urlless_palette')),
        );
        await tester.pumpAndSettle();

        expect(find.byType(EmbedPage), findsNothing);
        expect(find.byType(ExpandedModalPage), findsNWidgets(2));
        expect(find.text('URL-less Embed'), findsOneWidget);
      },
    );

    testWidgets(
      'picking_an_embed_with_a_URL_in_the_Palette_opens_the_EmbedPage',
      (tester) async {
        final originalTargetResolver = webViewTargetResolver;
        webViewTargetResolver = () => const WebViewTarget.linux();
        addTearDown(() => webViewTargetResolver = originalTargetResolver);

        await tester.pumpWidget(
          _harness(
            dispatch: ({pinKey, required toolId, required args}) =>
                _emptySuccess,
            tools: [_fixtureTool, _urlBackedEmbedTool],
            paletteHits: [_paletteHitFor(_urlBackedEmbedTool)],
            activation: ({required toolId, required argsJson}) =>
                PinActivationDto_OpenEmbed(toolId: toolId.value),
          ),
        );

        await tester.sendKeyEvent(LogicalKeyboardKey.slash);
        await tester.pumpAndSettle();
        await tester.tap(
          find.byKey(const ValueKey('palette-hit-embed.url_palette')),
        );
        await tester.pumpAndSettle();

        expect(find.byType(EmbedPage), findsOneWidget);
        expect(find.text('URL-backed Embed'), findsOneWidget);
      },
    );

    testWidgets(
      'ExpandedModalPage_CtrlK_opens_the_palette_even_without_a_character',
      (tester) async {
        await tester.pumpWidget(
          _harness(
            keyboardResolver:
                ({
                  required String key,
                  required bool ctrl,
                  required bool meta,
                  required bool shift,
                  required bool alt,
                  required KeyboardScopeDto scope,
                  required bool hasToolFocus,
                }) => null,
            dispatch: ({pinKey, required toolId, required args}) =>
                _emptySuccess,
          ),
        );

        await tester.showKeyboard(find.byKey(const Key('field-value')));
        final focus = tester.widget<Focus>(_expandedModalFocusFinder());
        final focusNode = Focus.of(tester.element(_expandedModalFocusFinder()));
        await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
        final result = focus.onKeyEvent!(
          focusNode,
          KeyDownEvent(
            physicalKey: PhysicalKeyboardKey.keyK,
            logicalKey: LogicalKeyboardKey.keyK,
            timeStamp: Duration.zero,
          ),
        );
        await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
        await tester.pumpAndSettle();

        expect(result, KeyEventResult.handled);
        expect(find.byType(PaletteOverlay), findsOneWidget);
      },
    );
  });
}
