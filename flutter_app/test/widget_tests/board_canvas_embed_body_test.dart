import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/features/memos/memo_pin_body.dart';
import 'package:upeg/src/features/memos/memos_provider.dart';
import 'package:upeg/src/features/controlled_embed/providers.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/embed.dart';
import 'package:upeg/src/rust/api/memos.dart' show MemoEntry;
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/capability_provider.dart';
import 'package:upeg/src/state/controlled_embed_settings_provider.dart';
import 'package:upeg/src/state/embed_resolver_provider.dart';
import 'package:upeg/src/state/push_preview_provider.dart';
import 'package:upeg/src/state/selector_bindings_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/board_canvas.dart';
import 'package:upeg/src/widgets/provider_not_configured_body.dart';
import 'package:upeg/src/widgets/embed_iframe_factory.dart';
import 'package:upeg/src/widgets/expanded_modal/webview_panel.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';
import 'package:upeg/src/widgets/pin.dart';

import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/board_canvas_harness.dart';
import '../test_helpers/controlled_embed_tile_harness.dart' as controlled;

void main() {
  group('BoardCanvas', () {
    testWidgets('an_Embed_pin_mounts_WebViewPanel_inline_in_its_body', (
      tester,
    ) async {
      final embedTool = ToolDto(
        id: 'embed.transform_tools',
        toolkit: 'embed',
        label: 'Transform.tools embed',
        description: '',
        tags: const [],
        inputFields: const [],
        outputFields: const [],
        pinKind: PinKindDto.embed,
        invoker: InvokerDto.embed,
        pegboardUnits: PegboardUnitsDto.u2,
        source: const SourceDto.static_(),
        requiresApproval: false,
        approvalSurfaces: const <String>[],
        effect: ToolEffectDto.unknown,
      );

      final originalBuilder = desktopWebViewBuilder;
      final originalTarget = webViewTargetResolver;
      desktopWebViewBuilder = (url, userAgent, onControllerReady) =>
          SizedBox.expand(key: const Key('webview-panel-inappwebview'));
      // Pin the target to inAppWebView so the test exercises the
      // inline-webview path regardless of host platform (Linux test
      // runs would otherwise resolve to externalLauncher).
      webViewTargetResolver = () => const WebViewTarget.inAppWebView();
      embedIframeFactory = const NoopIframeFactory();
      addTearDown(() {
        desktopWebViewBuilder = originalBuilder;
        webViewTargetResolver = originalTarget;
        embedIframeFactory = const DartUiWebIframeFactory();
      });

      var tapped = false;
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            ...i18nTestOverrides,
            toolsLoaderProvider.overrideWith(
              (ref) =>
                  () => [embedTool],
            ),
            resolveEmbedFnProvider.overrideWith(
              (ref) =>
                  ({required ToolId toolId, required ToolArgs args}) =>
                      const EmbedResolutionDto(
                        url: 'https://transform.tools/json-to-typescript',
                      ),
            ),
          ],
          child: MaterialApp(
            home: Scaffold(
              body: debugBoardCanvasGrid(
                snapshot: const LayoutSnapshotDto(
                  boardKey: 'dev',
                  boardCols: 6,
                  placements: [
                    PlacementDto(
                      toolId: 'embed.transform_tools',
                      x: 0,
                      y: 0,
                      w: 2,
                      h: 1,
                    ),
                  ],
                ),
                onPinTap: (_) => tapped = true,
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      // Webview is mounted INSIDE the pin tile.
      expect(
        find.byKey(const Key('webview-panel-inappwebview')),
        findsOneWidget,
      );

      // Tapping the pin does NOT fire onPinTap — embed pins delegate
      // gestures to the webview, the parent activation seam is bypassed.
      await tester.tap(find.byType(Pin));
      expect(tapped, isFalse);
    });

    testWidgets('the_embed_body_receives_taps_and_the_handle_can_be_dragged', (
      tester,
    ) async {
      // Modeless coexistence contract (RISK AREA): an embed pin's live body
      // keeps its pointer events (deferToChild) AND the pin is still movable
      // — but drag originates ONLY from the move handle, so a body tap never
      // starts a drag and a handle drag never disturbs the webview.
      const embedTool = ToolDto(
        id: 'embed.transform_tools',
        toolkit: 'embed',
        label: 'Transform Tools',
        description: '',
        tags: [],
        inputFields: [],
        outputFields: [],
        pinKind: PinKindDto.embed,
        invoker: InvokerDto.embed,
        pegboardUnits: PegboardUnitsDto.u2,
        source: SourceDto.static_(),
        requiresApproval: false,
        approvalSurfaces: <String>[],
        effect: ToolEffectDto.unknown,
      );

      final originalBuilder = desktopWebViewBuilder;
      final originalTarget = webViewTargetResolver;
      var bodyTapped = false;
      desktopWebViewBuilder = (url, userAgent, onControllerReady) =>
          GestureDetector(
            key: const Key('embed-body-tap-target'),
            behavior: HitTestBehavior.opaque,
            onTap: () => bodyTapped = true,
            child: const SizedBox.expand(),
          );
      webViewTargetResolver = () => const WebViewTarget.inAppWebView();
      embedIframeFactory = const NoopIframeFactory();
      addTearDown(() {
        desktopWebViewBuilder = originalBuilder;
        webViewTargetResolver = originalTarget;
        embedIframeFactory = const DartUiWebIframeFactory();
      });

      ToolId? movedTool;
      var tapped = false;
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            ...i18nTestOverrides,
            toolsLoaderProvider.overrideWith(
              (ref) =>
                  () => [embedTool],
            ),
            pushPreviewLoaderProvider.overrideWithValue(
              (boardKey, toolId, anchorX, anchorY) => const <PlacementDto>[],
            ),
            resolveEmbedFnProvider.overrideWith(
              (ref) =>
                  ({required ToolId toolId, required ToolArgs args}) =>
                      const EmbedResolutionDto(url: 'https://example.test/e'),
            ),
          ],
          child: MaterialApp(
            home: Scaffold(
              body: SizedBox(
                width: 2000,
                height: 1000,
                child: debugBoardCanvasGrid(
                  snapshot: const LayoutSnapshotDto(
                    boardKey: 'dev',
                    boardCols: 6,
                    placements: [
                      PlacementDto(
                        toolId: 'embed.transform_tools',
                        x: 0,
                        y: 0,
                        w: 2,
                        h: 1,
                      ),
                    ],
                  ),
                  onPinTap: (_) => tapped = true,
                  onMovePin:
                      ({
                        required boardKey,
                        required toolId,
                        required anchorX,
                        required anchorY,
                      }) {
                        movedTool = toolId;
                      },
                ),
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      // (a) The embed body receives taps; the parent activation seam is
      // bypassed because the live body owns the pointer.
      await tester.tap(find.byKey(const Key('embed-body-tap-target')));
      await tester.pumpAndSettle();
      expect(bodyTapped, isTrue);
      expect(tapped, isFalse);

      // (b) The pin is still draggable — from the move handle.
      final handle = find.byKey(pinMoveHandleKey);
      expect(handle, findsOneWidget);
      final gesture = await tester.startGesture(tester.getCenter(handle));
      final step = Offset(UpegSizing.pinCellWidth + UpegSizing.pinGap, 0);
      for (int i = 1; i <= 6; i++) {
        await gesture.moveBy(step / 6);
        await tester.pump(const Duration(milliseconds: 16));
      }
      await gesture.up();
      await tester.pumpAndSettle();

      expect(movedTool, ToolId.parse('embed.transform_tools'));
    });

    testWidgets(
      'a controlled embed pin on the board shows shared execution results as labels and values',
      (tester) async {
        final controlledTool = ToolDto(
          id: 'embed.example_controlled',
          toolkit: 'embed',
          label: 'Example Controlled Embed',
          description: '',
          tags: const [],
          inputFields: const [],
          outputFields: const [
            OutputFieldDto(
              key: 'intro',
              label: 'Intro',
              fieldType: OutputFieldType.text(),
            ),
          ],
          pinKind: PinKindDto.controlledEmbed,
          invoker: InvokerDto.embed,
          pegboardUnits: PegboardUnitsDto.u2,
          source: const SourceDto.static_(),
          requiresApproval: false,
          approvalSurfaces: const <String>[],
          effect: ToolEffectDto.unknown,
        );

        await tester.pumpWidget(
          ProviderScope(
            overrides: [
              ...i18nTestOverrides,
              controlledEmbedNativeSupportedProvider.overrideWithValue(true),
              controlledEmbedToolExecutorProvider.overrideWithValue(
                ({required toolId, required args, boardKey}) async =>
                    controlledEmbedSuccessResult({'intro': 'board preview'}),
              ),
              toolsLoaderProvider.overrideWith(
                (ref) =>
                    () => [controlledTool],
              ),
              resolveEmbedFnProvider.overrideWith(
                (ref) =>
                    ({required ToolId toolId, required ToolArgs args}) =>
                        const EmbedResolutionDto(
                          url: 'https://example.test/controlled',
                        ),
              ),
              selectorBindingsLoaderProvider.overrideWithValue(
                (ToolId _) => const [],
              ),
              controlledEmbedSettingsLoaderProvider.overrideWithValue(
                (ToolId _) => const ControlledEmbedSettingsDto(),
              ),
            ],
            child: MaterialApp(
              home: Scaffold(
                body: debugBoardCanvasGrid(
                  snapshot: const LayoutSnapshotDto(
                    boardKey: 'dev',
                    boardCols: 6,
                    placements: [
                      PlacementDto(
                        toolId: 'embed.example_controlled',
                        x: 0,
                        y: 0,
                        w: 2,
                        h: 1,
                      ),
                    ],
                  ),
                  onPinTap: (_) {},
                ),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();

        final robot = controlled.ControlledEmbedTileRobot(tester);
        robot.expectRunHint();
        await robot.tapRun();
        robot.expectOutputRow(label: 'Intro', value: 'board preview');
      },
    );

    testWidgets('an_embed_pin_without_a_URL_is_tappable', (tester) async {
      final embedTool = ToolDto(
        id: 'embed.urlless_tool',
        toolkit: 'embed',
        label: 'URL-less Embed',
        description: 'Embed without URL',
        tags: const [],
        inputFields: const [],
        outputFields: const [],
        pinKind: PinKindDto.embed,
        invoker: InvokerDto.embed,
        pegboardUnits: PegboardUnitsDto.u2,
        source: const SourceDto.static_(),
        requiresApproval: false,
        approvalSurfaces: const <String>[],
        effect: ToolEffectDto.unknown,
      );

      var tapped = false;

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            ...i18nTestOverrides,
            toolsLoaderProvider.overrideWith(
              (ref) =>
                  () => [embedTool],
            ),
            // URL fallback behavior is a native rendering contract; the
            // dedicated wasm tests provide a fake capability verdict.
            isWasmRuntimeProvider.overrideWithValue(false),
            resolveEmbedFnProvider.overrideWith(
              (ref) =>
                  ({required ToolId toolId, required ToolArgs args}) => null,
            ),
          ],
          child: MaterialApp(
            home: Scaffold(
              body: debugBoardCanvasGrid(
                snapshot: const LayoutSnapshotDto(
                  boardKey: 'dev',
                  boardCols: 6,
                  placements: [
                    PlacementDto(
                      toolId: 'embed.urlless_tool',
                      x: 0,
                      y: 0,
                      w: 2,
                      h: 1,
                    ),
                  ],
                ),
                onPinTap: (_) => tapped = true,
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.byType(Pin), findsOneWidget);
      expect(find.text('EMBED.URLLESS_TOOL'), findsOneWidget);

      // URL-less embed should be tappable
      await tester.tap(find.byType(Pin));
      expect(tapped, isTrue);
    });

    testWidgets('a_memo_notepad_pin_renders_an_inline_editor_and_saves_edits', (
      tester,
    ) async {
      final memoTool = ToolDto(
        id: 'memo.scratch',
        toolkit: 'memo',
        label: 'Scratch memo',
        description: '',
        tags: const [],
        inputFields: const [],
        outputFields: const [
          OutputFieldDto(
            key: 'memo',
            label: 'memo',
            fieldType: OutputFieldType.markdown(),
          ),
        ],
        pinKind: PinKindDto.live,
        invoker: InvokerDto.function,
        pegboardUnits: PegboardUnitsDto.u1,
        source: const SourceDto.userInput(),
        requiresApproval: false,
        approvalSurfaces: const <String>[],
        effect: ToolEffectDto.unknown,
      );
      final saved = <List<MemoEntry>>[];

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            ...i18nTestOverrides,
            toolsLoaderProvider.overrideWith(
              (ref) =>
                  () => [memoTool],
            ),
            memosLoaderProvider.overrideWithValue(() => const <MemoEntry>[]),
            memosSaverProvider.overrideWithValue(
              (entries) => saved.add(entries),
            ),
          ],
          child: MaterialApp(
            home: Scaffold(
              body: debugBoardCanvasGrid(
                snapshot: const LayoutSnapshotDto(
                  boardKey: 'dev',
                  boardCols: 6,
                  placements: [
                    PlacementDto(
                      toolId: 'memo.scratch',
                      x: 0,
                      y: 0,
                      w: 2,
                      h: 2,
                    ),
                  ],
                ),
                onPinTap: (_) {},
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.byKey(memoPinFieldKey), findsOneWidget);

      await tester.enterText(find.byKey(memoPinFieldKey), 'hello memo');
      await tester.pump();

      expect(saved, isNotEmpty);
      expect(saved.last.single.key, scratchMemoKey);
      expect(saved.last.single.body, 'hello memo');
    });

    testWidgets(
      'an_unconfigured_live_http_pin_shows_that_configuration_is_required',
      (tester) async {
        final ethTool = ToolDto(
          id: 'eth.gas',
          toolkit: 'eth',
          label: 'ETH gas',
          description: 'Live Ethereum gas price (gwei).',
          tags: const [],
          inputFields: const [],
          outputFields: const [
            OutputFieldDto(
              key: 'gwei',
              label: 'gwei',
              fieldType: OutputFieldType.number(),
            ),
          ],
          pinKind: PinKindDto.live,
          invoker: InvokerDto.http,
          pegboardUnits: PegboardUnitsDto.u1,
          source: const SourceDto.static_(),
          requiresApproval: false,
          approvalSurfaces: const <String>[],
          effect: ToolEffectDto.unknown,
        );

        await tester.pumpWidget(
          ProviderScope(
            overrides: [
              // `i18nTestOverrides` serves both the Pin semantics strings
              // and the provider-not-configured body copy from the shared
              // fixture catalog, so no key-echo override is needed.
              ...i18nTestOverrides,
              toolsLoaderProvider.overrideWith(
                (ref) =>
                    () => [ethTool],
              ),
            ],
            child: MaterialApp(
              home: Scaffold(
                body: debugBoardCanvasGrid(
                  snapshot: const LayoutSnapshotDto(
                    boardKey: 'dev',
                    boardCols: 6,
                    placements: [
                      PlacementDto(toolId: 'eth.gas', x: 0, y: 0, w: 1, h: 1),
                    ],
                  ),
                  onPinTap: (_) {},
                ),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();

        expect(find.byKey(providerNotConfiguredBodyKey), findsOneWidget);
        expect(
          find.text(i18nEn(providerNotConfiguredLabelKey)),
          findsOneWidget,
        );
      },
    );
  });
}
