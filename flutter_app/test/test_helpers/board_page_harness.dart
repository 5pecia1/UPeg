/// Shared fixtures for the `board_page_*_test.dart` keyboard family: the
/// `BoardPage` provider scope, the focus/key dispatch probes, and the
/// controlled-embed board those tests type into.
library;

import 'package:flutter/material.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/pages/board_page.dart';
import 'package:upeg/src/platform/app_lifecycle.dart';
import 'package:upeg/src/rust/api/boot.dart';
import 'package:upeg/src/rust/api/embed.dart'
    show ControlledEmbedSettingsDto, EmbedResolutionDto;
import 'package:upeg/src/rust/api/keyboard.dart';
import 'package:upeg/src/rust/api/i18n.dart' show LocaleDto;
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/pause.dart';
import 'package:upeg/src/rust/api/status.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/controlled_embed_settings_provider.dart';
import 'package:upeg/src/state/embed_resolver_provider.dart';
import 'package:upeg/src/state/focused_pin_provider.dart';
import 'package:upeg/src/state/move_pin_commit_provider.dart';
import 'package:upeg/src/state/locale_provider.dart';
import 'package:upeg/src/state/move_pin_slot_provider.dart';
import 'package:upeg/src/state/pin_activation_provider.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/state/reorder_pin_provider.dart';
import 'package:upeg/src/state/push_preview_provider.dart';
import 'package:upeg/src/state/selector_bindings_provider.dart';
import 'package:upeg/src/state/suggestions_provider.dart';
import 'package:upeg/src/state/status_provider.dart' show statusReaderProvider;
import 'package:upeg/src/state/tag_provider.dart';
import 'package:upeg/src/state/window_mode_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart' show ToolArgs;
import 'package:upeg/src/widgets/expanded_modal/webview_panel.dart'
    show desktopWebViewBuilder, webViewTargetResolver, WebViewTarget;
import 'package:upeg/src/widgets/pin.dart';

import 'fake_binding_catalog.dart';
import 'pegboard_selection_overrides.dart';
import 'tool_fixture.dart';
import 'i18n_test_catalog.dart';

// Both page families assert on the same window-mode toggle; the recorder
// they share lives in one file (see window_mode_recorder.dart).
export 'window_mode_recorder.dart';

const fakeReport = AppInitReport(
  launch: DesktopLaunchDto(board: 'dev'),
  hostState: HostStateDto.noHost(),
  version: 'test',
);

const fakeStatus = StatusSnapshotDto(
  network: NetworkStatusDto(
    reachability: NetworkReachabilityDto.offline,
    label: 'offline',
  ),
  paused: PausedStateDto.running,
  mcpImportCount: 0,
  mcpImportPhase: McpImportPhaseDto.notStarted,
  buildVersion: 'test',
);

const Key selectedTagProbeKey = Key('selected-tag-probe');
const Key pinContextEditColorKey = Key('pin-context-edit-color');
const Key pinColorDialogKey = Key('pin-color-dialog');
const String contextMenuBoardKey = 'dev';
const String contextMenuToolId = 'num.hex_to_decimal';
const String contextMenuToolLabel = 'Hex to Dec';
const String editColorMenuText = 'edit color';
const String unpinMenuText = 'unpin';
const contextMenuPlacement = PlacementDto(
  toolId: contextMenuToolId,
  x: 0,
  y: 0,
  w: 1,
  h: 1,
);

class SeededFocusedPinNotifier extends FocusedPinNotifier {
  SeededFocusedPinNotifier(this._seed);

  final String _seed;

  @override
  ToolId? build() => ToolId.parse(_seed);
}

Widget boardPageHarness({
  required KeyboardCommandResolver resolver,
  List<BoardDto> boards = const <BoardDto>[BoardDto(key: 'dev', title: 'Dev')],
  List<ToolDto> tools = const <ToolDto>[],
  List<String> tagOptions = const <String>['all'],
  String? currentBoardKey,
  String? focusedToolId,
  bool includeSelectedTagProbe = false,
  PinActivationFn? pinActivation,
  IsPinnedLoader? isPinnedLoader,
  PinMutator? pinMutator,
  PinMutator? unpinMutator,
  MovePinSlotFn? movePinSlot,
  ReorderPinFn? reorderPin,
  MovePinCommitFn? movePinCommit,
  QuitAppFn? quitApp,
  LayoutLoader? layoutLoader,
  WindowModeNotifier Function()? windowModeNotifier,
  List<Widget> extraOverlays = const <Widget>[],
  ResolveEmbedFn? resolveEmbed,
  SelectorBindingsLoader? selectorBindings,
  ControlledEmbedSettingsLoader? controlledEmbedSettings,
  List<Override> extraOverrides = const <Override>[],
}) {
  final overlays = <Widget>[
    if (includeSelectedTagProbe)
      Positioned(
        left: 0,
        top: 0,
        child: Consumer(
          builder: (context, ref, _) {
            final selected = ref.watch(selectedTagProvider);
            return Text(selected.frbValue, key: selectedTagProbeKey);
          },
        ),
      ),
    ...extraOverlays,
  ];
  final home = overlays.isEmpty
      ? const BoardPage(report: fakeReport)
      : Stack(
          children: [
            const BoardPage(report: fakeReport),
            ...overlays,
          ],
        );

  return ProviderScope(
    overrides: [
      if (focusedToolId != null)
        focusedPinProvider.overrideWith(
          () => SeededFocusedPinNotifier(focusedToolId),
        ),
      if (pinActivation != null)
        pinActivationProvider.overrideWithValue(pinActivation),
      if (isPinnedLoader != null)
        isPinnedLoaderProvider.overrideWithValue(isPinnedLoader),
      if (pinMutator != null)
        pinToolMutatorProvider.overrideWithValue(pinMutator),
      if (unpinMutator != null)
        unpinToolMutatorProvider.overrideWithValue(unpinMutator),
      if (movePinSlot != null)
        movePinSlotFnProvider.overrideWithValue(movePinSlot),
      if (reorderPin != null)
        reorderPinFnProvider.overrideWithValue(reorderPin),
      if (movePinCommit != null)
        movePinCommitFnProvider.overrideWithValue(movePinCommit),
      if (quitApp != null) quitAppProvider.overrideWithValue(quitApp),
      if (windowModeNotifier != null)
        windowModeProvider.overrideWith(windowModeNotifier),
      if (resolveEmbed != null)
        resolveEmbedFnProvider.overrideWith((_) => resolveEmbed),
      if (selectorBindings != null)
        selectorBindingsLoaderProvider.overrideWithValue(selectorBindings),
      if (controlledEmbedSettings != null)
        controlledEmbedSettingsLoaderProvider.overrideWithValue(
          controlledEmbedSettings,
        ),
      ...pegboardSelectionOverrides(
        boardKey: currentBoardKey ?? 'dev',
        tagOptions: (_) => tagOptions,
      ),
      localeProvider.overrideWithValue(LocaleDto.en),
      ...i18nTestOverrides,
      statusReaderProvider.overrideWithValue(() => fakeStatus),
      boardsLoaderProvider.overrideWith(
        (ref) =>
            () => boards,
      ),
      layoutLoaderProvider.overrideWithValue(
        layoutLoader ??
            (query) => LayoutSnapshotDto(
              boardKey: query.boardKey.value,
              boardCols: 6,
              placements: const <PlacementDto>[],
            ),
      ),
      tagOptionsLoaderProvider.overrideWithValue(() => tagOptions),
      boardTagOptionsLoaderProvider.overrideWithValue((_) => tagOptions),
      tagCountLoaderProvider.overrideWithValue((selection) => 0),
      boardTagCountLoaderProvider.overrideWithValue((_, selection) => 0),
      toolsLoaderProvider.overrideWithValue(() => tools),
      suggestionsLoaderProvider.overrideWithValue(
        (boardKey) => const <ToolDto>[],
      ),
      pushPreviewLoaderProvider.overrideWithValue(
        (boardKey, toolId, anchorX, anchorY) => const <PlacementDto>[],
      ),
      keyboardCommandResolverProvider.overrideWith((_) => resolver),
      // Pin context menu key hints read the binding catalog; widget
      // tests never link the dylib, so serve the shared fixture.
      fakeBindingCatalogOverride,
      ...extraOverrides,
    ],
    child: MaterialApp(theme: UpegTheme.darkTheme(), home: home),
  );
}

/// Minimal Focus widget that wires the same keyboard handler shape as
/// BoardPage but without the full BoardPage tree (BoardPage requires a
/// Rust-opaque AppInitReport that has no Dart-side constructor).
class KeyboardHarness extends ConsumerStatefulWidget {
  const KeyboardHarness({required this.onCommand, super.key});
  final void Function(KeyboardCommandDto?) onCommand;

  @override
  ConsumerState<KeyboardHarness> createState() => _KeyboardHarnessState();
}

class _KeyboardHarnessState extends ConsumerState<KeyboardHarness> {
  final FocusNode _node = FocusNode();

  @override
  void dispose() {
    _node.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Focus(
      focusNode: _node,
      autofocus: true,
      onKeyEvent: (_, event) {
        if (event is! KeyDownEvent) return KeyEventResult.ignored;
        final cmd = resolveKeyboardCommand(
          ref,
          event,
          scope: KeyboardScopeDto.board,
        );
        widget.onCommand(cmd);
        return cmd != null ? KeyEventResult.handled : KeyEventResult.ignored;
      },
      child: const SizedBox.expand(),
    );
  }
}

KeyEventResult dispatchBoardPageKey(
  WidgetTester tester, {
  required LogicalKeyboardKey logicalKey,
  required PhysicalKeyboardKey physicalKey,
  String? character,
}) {
  final focus = tester.widget<Focus>(boardPageFocusFinder());
  return focus.onKeyEvent!(
    focus.focusNode!,
    KeyDownEvent(
      physicalKey: physicalKey,
      logicalKey: logicalKey,
      timeStamp: Duration.zero,
      character: character,
    ),
  );
}

Finder boardPageFocusFinder() {
  return find.byWidgetPredicate(
    (widget) => widget is Focus && widget.focusNode?.debugLabel == 'BoardPage',
  );
}

/// Stub mirroring the Rust keymap slice used by the quit flow
/// (`upeg-core/src/keyboard.rs`): board-scope `q` → Quit, and the shared
/// confirm scope resolves F1/Enter/y → Confirm, Esc/n/q → Cancel.
KeyboardCommandDto? quitFlowResolver({
  required String key,
  required bool ctrl,
  required bool meta,
  required bool shift,
  required bool alt,
  required bool hasToolFocus,
  required KeyboardScopeDto scope,
}) {
  if (scope == KeyboardScopeDto.confirmDelete) {
    return switch (key) {
      'F1' || 'Enter' || 'y' => const KeyboardCommandDto.confirm(),
      'Escape' || 'n' || 'q' => const KeyboardCommandDto.cancel(),
      _ => null,
    };
  }
  if (key == 'q') return const KeyboardCommandDto.quit();
  return null;
}

KeyboardCommandDto? noKeyboardCommand({
  required String key,
  required bool ctrl,
  required bool meta,
  required bool shift,
  required bool alt,
  required bool hasToolFocus,
  required KeyboardScopeDto scope,
}) {
  return null;
}

Future<void> pumpBoardPageWithPinnedTool(
  WidgetTester tester, {
  String? focusedToolId,
}) async {
  await tester.pumpWidget(
    boardPageHarness(
      currentBoardKey: contextMenuBoardKey,
      focusedToolId: focusedToolId,
      tools: [
        fixtureToolDto(id: contextMenuToolId, label: contextMenuToolLabel),
      ],
      layoutLoader: (_) => const LayoutSnapshotDto(
        boardKey: contextMenuBoardKey,
        boardCols: 6,
        placements: <PlacementDto>[contextMenuPlacement],
      ),
      resolver: noKeyboardCommand,
    ),
  );
  await tester.pumpAndSettle();
}

Future<void> openPinContextMenu(WidgetTester tester) async {
  final gesture = await tester.startGesture(
    tester.getCenter(find.byType(Pin)),
    kind: PointerDeviceKind.mouse,
    buttons: kSecondaryMouseButton,
  );
  await gesture.up();
  await tester.pumpAndSettle();
}

/// Focuses the inline embed's form field via its `EditableText` focus
/// node. Programmatic focus avoids hit-testing a tile that may render
/// tightly inside a pin cell.
void focusEmbedField(WidgetTester tester) {
  final editable = tester.widget<EditableText>(
    find.descendant(
      of: find.byKey(embedFieldWidgetKey),
      matching: find.byType(EditableText),
    ),
  );
  editable.focusNode.requestFocus();
}

// --- Inline embed keyboard-coexistence fixtures (fixes 1/2/3) ---

const String embedToolId = 'embed.example_controlled';
const String embedFieldKey = 'q';
final Key embedFieldWidgetKey = Key('field-$embedFieldKey');

typedef PlainKeyProbe = ({
  LogicalKeyboardKey logical,
  PhysicalKeyboardKey physical,
  String character,
});

/// Plain-letter/digit keys that map to board commands when the board owns
/// focus, but must become text while an inline embed body is focused.
const List<PlainKeyProbe> plainKeyProbes = <PlainKeyProbe>[
  (
    logical: LogicalKeyboardKey.keyE,
    physical: PhysicalKeyboardKey.keyE,
    character: 'e',
  ),
  (
    logical: LogicalKeyboardKey.keyM,
    physical: PhysicalKeyboardKey.keyM,
    character: 'm',
  ),
  (
    logical: LogicalKeyboardKey.keyP,
    physical: PhysicalKeyboardKey.keyP,
    character: 'p',
  ),
  (
    logical: LogicalKeyboardKey.digit1,
    physical: PhysicalKeyboardKey.digit1,
    character: '1',
  ),
  (
    logical: LogicalKeyboardKey.keyH,
    physical: PhysicalKeyboardKey.keyH,
    character: 'h',
  ),
];

/// Resolver that records every consulted key and maps the probe keys to
/// real board commands. If the guard works, the plain-key branch is never
/// reached (the resolver is not consulted for those keys); Cmd/Ctrl+K and
/// Escape still resolve globally.
KeyboardCommandResolver embedProbeResolver(void Function(String key) onKey) {
  return ({
    required String key,
    required bool ctrl,
    required bool meta,
    required bool shift,
    required bool alt,
    required KeyboardScopeDto scope,
    required bool hasToolFocus,
  }) {
    onKey(key);
    if ((ctrl || meta) && key == 'k') return const KeyboardCommandDto.search();
    if (key == 'Escape') return const KeyboardCommandDto.close();
    return switch (key) {
      'm' => const KeyboardCommandDto.startMove(),
      'p' => const KeyboardCommandDto.togglePin(),
      'h' => const KeyboardCommandDto.move(direction: DirectionDto.left),
      '1' => const KeyboardCommandDto.switchBoard(slot: 1),
      _ => null,
    };
  };
}

/// Pumps a BoardPage carrying a single inline ControlledEmbed pin whose
/// form field can take focus. Webview seams are stubbed so no real
/// platform view is spun up.
Future<void> pumpBoardWithControlledEmbed(
  WidgetTester tester, {
  required void Function(String key) onResolverKey,
  WindowModeNotifier Function()? windowModeNotifier,
  String? focusedToolId,
}) async {
  final originalBuilder = desktopWebViewBuilder;
  final originalTarget = webViewTargetResolver;
  desktopWebViewBuilder = (url, ua, onReady) => const SizedBox(
    key: Key('board-embed-webview-placeholder'),
    width: 100,
    height: 100,
  );
  webViewTargetResolver = () => const WebViewTarget.inAppWebView();
  addTearDown(() {
    desktopWebViewBuilder = originalBuilder;
    webViewTargetResolver = originalTarget;
  });

  await tester.pumpWidget(
    boardPageHarness(
      currentBoardKey: 'dev',
      focusedToolId: focusedToolId,
      tools: [
        fixtureToolDto(
          id: embedToolId,
          toolkit: 'embed',
          label: 'Example Controlled Embed',
          pinKind: PinKindDto.controlledEmbed,
          invoker: InvokerDto.embed,
          inputFields: const <InputFieldDto>[
            InputFieldDto(
              key: embedFieldKey,
              label: 'Query',
              fieldType: InputFieldType.text(),
              required_: false,
            ),
          ],
          outputFields: const <OutputFieldDto>[
            OutputFieldDto(
              key: 'intro',
              label: 'Intro',
              fieldType: OutputFieldType.text(),
            ),
          ],
        ),
      ],
      layoutLoader: (query) => LayoutSnapshotDto(
        boardKey: query.boardKey.value,
        boardCols: 6,
        // h:2 gives the inline form + Run row + output strip enough
        // vertical room that the tile does not overflow its pin cell.
        placements: const <PlacementDto>[
          PlacementDto(toolId: embedToolId, x: 0, y: 0, w: 2, h: 2),
        ],
      ),
      resolveEmbed: ({required ToolId toolId, required ToolArgs args}) =>
          const EmbedResolutionDto(url: 'https://example.test/controlled'),
      selectorBindings: (ToolId _) => const [],
      controlledEmbedSettings: (ToolId _) => const ControlledEmbedSettingsDto(),
      windowModeNotifier: windowModeNotifier,
      resolver: embedProbeResolver(onResolverKey),
    ),
  );
  await tester.pumpAndSettle();
}
