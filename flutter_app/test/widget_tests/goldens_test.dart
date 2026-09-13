/// Golden regression tests for the major widgets in the Flutter
/// surface. Pins the reference visuals so future drift fails CI
/// automatically.
///
/// Why one consolidated file?
/// Each major widget gets one `matchesGoldenFile` test pinned to a
/// PNG under `test/goldens/`. The Justfile recipe `ui-parity-check`
/// (and CI) re-run these with `flutter test --tags=golden`; tests
/// without the tag remain in the default Flutter test pass.
///
/// Determinism notes
/// -----------------
/// 1. Default text rendering in `flutter test` uses Ahem (a square
///    fallback font) — every glyph paints as a solid block. That's
///    intentional: it removes antialiasing/hinting drift across host
///    machines so the same widget tree produces a byte-identical
///    PNG everywhere. Re-baseline with `flutter test --update-goldens`
///    after intentional UI edits.
/// 2. Each test pumps the widget under a fixed `Size` via
///    `tester.view.physicalSize` / `devicePixelRatio` so layout is
///    locked. The viewport is restored in `addTearDown`.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/platform/keyboard_label.dart';

import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/pages/expanded_modal_page.dart';
import 'package:upeg/src/pages/popup_page.dart';
import 'package:upeg/src/rust/api/i18n.dart' show LocaleDto;
import 'package:upeg/src/rust/api/palette.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/rust/api/tweaks.dart';
import 'package:upeg/src/rust/api/pause.dart' show PausedStateDto;
import 'package:upeg/src/rust/api/status.dart'
    show McpImportPhaseDto, NetworkReachabilityDto, NetworkStatusDto;
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/state/status_provider.dart';
import 'package:upeg/src/state/tweaks_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/board_canvas.dart';
import 'package:upeg/src/widgets/palette_overlay.dart';
import 'package:upeg/src/widgets/tweaks_form.dart';
import 'package:upeg/src/state/dispatch_stream_provider.dart';

import '../test_helpers/pegboard_selection_overrides.dart';
import '../test_helpers/tool_fixture.dart';
import '../test_helpers/dispatch_stream_fixture.dart';

/// Fixed status snapshot for the popup golden — keeps the build
/// version stable across hosts and avoids the live FRB call.
class _GoldenStatusNotifier extends StatusNotifier {
  @override
  StatusSnapshotDto build() => const StatusSnapshotDto(
    network: NetworkStatusDto(
      reachability: NetworkReachabilityDto.loopbackOnly,
      label: 'loopback-only',
    ),
    paused: PausedStateDto.running,
    mcpImportCount: 0,
    mcpImportPhase: McpImportPhaseDto.notStarted,
    buildVersion: '0.0.0',
  );
}

/// Fixes the widget viewport to a deterministic size and unsets it on
/// teardown so other tests in the same file get a clean slate.
void _useViewport(WidgetTester tester, Size logical) {
  final view = tester.view
    ..devicePixelRatio = 1.0
    ..physicalSize = logical;
  addTearDown(() {
    view
      ..resetDevicePixelRatio()
      ..resetPhysicalSize();
  });
}

Widget _themed(Widget child) => MaterialApp(
  theme: UpegTheme.darkTheme(),
  debugShowCheckedModeBanner: false,
  home: Scaffold(body: child),
);

/// Stable golden-deterministic translation: echo the last dotted path
/// segment so PNGs don't depend on the native catalog being linked.
String _goldenTranslate(String key, LocaleDto locale) => key.split('.').last;

String _goldenTranslateArgs(
  String key,
  LocaleDto locale,
  List<String> argKeys,
  List<String> argVals,
) {
  final base = key.split('.').last;
  if (argKeys.isEmpty) return base;
  return '$base(${argVals.join(",")})';
}

final _fixtureTool = fixtureToolDto(
  id: 'num.hex_to_decimal',
  toolkit: 'convert',
  label: 'hex → dec',
  description: 'hex string to decimal',
  inputFields: const [
    InputFieldDto(
      key: 'value',
      label: 'value',
      fieldType: InputFieldType_Text(),
      required_: true,
    ),
  ],
);

final _popupCatalogueTools = <ToolDto>[
  _fixtureTool,
  fixtureToolDto(
    id: 'id.uuid_v7',
    toolkit: 'id',
    label: 'uuid v7',
    description: 'generate uuid v7',
  ),
];

const _fixtureHits = <PaletteHit>[
  PaletteHit(
    id: 'num.hex_to_decimal',
    label: 'hex → dec',
    description: 'hex string to decimal',
    score: 1.0,
    pinKind: PinKindDto.inline,
  ),
  PaletteHit(
    id: 'id.uuid_v7',
    label: 'uuid v7',
    description: 'generate uuid v7',
    score: 0.9,
    pinKind: PinKindDto.inline,
  ),
];

void main() {
  group('Goldens', () {
    testWidgets('BoardCanvas_골든', tags: <String>['golden'], (tester) async {
      _useViewport(tester, const Size(800, 600));
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            toolsLoaderProvider.overrideWith(
              (ref) =>
                  () => const <ToolDto>[],
            ),
            // Pin semantics strings resolve via t(); deterministic echo
            // keeps the golden independent of the native catalog (the
            // strings live in the semantics tree, not in pixels).
            i18nTranslateOverride.overrideWithValue(_goldenTranslate),
            i18nTranslateArgsOverride.overrideWithValue(_goldenTranslateArgs),
            keyboardPlatformProvider.overrideWithValue(TargetPlatform.macOS),
          ],
          child: _themed(
            debugBoardCanvasGrid(
              snapshot: const LayoutSnapshotDto(
                boardKey: 'dev',
                boardCols: 6,
                placements: [
                  PlacementDto(
                    toolId: 'num.hex_to_decimal',
                    x: 0,
                    y: 0,
                    w: 1,
                    h: 1,
                  ),
                  PlacementDto(toolId: 'id.uuid_v7', x: 1, y: 0, w: 1, h: 1),
                ],
              ),
              onPinTap: (_) {},
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      await expectLater(
        find.byType(MaterialApp),
        matchesGoldenFile('../goldens/board_canvas.png'),
      );
    });

    testWidgets('PaletteOverlay_골든', tags: <String>['golden'], (tester) async {
      _useViewport(tester, const Size(800, 600));
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            paletteSearcherProvider.overrideWith(
              (ref) =>
                  (query) => _fixtureHits,
            ),
            paletteQueryProvider.overrideWith((ref) => 'hex'),
            boardsLoaderProvider.overrideWith(
              (ref) =>
                  () => const <BoardDto>[
                    BoardDto(key: 'dev', title: 'Dev'),
                    BoardDto(key: 'media', title: 'Media'),
                  ],
            ),
            pinnedBoardsLoaderProvider.overrideWith(
              (ref) =>
                  (toolId) => toolId.value == 'num.hex_to_decimal'
                  ? {BoardKey.parse('dev')}
                  : const <BoardKey>{},
            ),
            i18nTranslateOverride.overrideWithValue(_goldenTranslate),
            i18nTranslateArgsOverride.overrideWithValue(_goldenTranslateArgs),
            keyboardPlatformProvider.overrideWithValue(TargetPlatform.macOS),
          ],
          child: _themed(PaletteOverlay(onPick: (_) {})),
        ),
      );
      await tester.pumpAndSettle();
      await expectLater(
        find.byType(MaterialApp),
        matchesGoldenFile('../goldens/palette_overlay.png'),
      );
    });

    testWidgets('ExpandedModalPage_골든', tags: <String>['golden'], (
      tester,
    ) async {
      _useViewport(tester, const Size(900, 700));
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            dispatchStreamFnProvider.overrideWithValue(
              stubDispatchStream(
                ({required toolId, required args, required approve}) async =>
                    const CanonicalToolResult(ok: true, outputs: []),
              ),
            ),
            i18nTranslateOverride.overrideWithValue(_goldenTranslate),
            i18nTranslateArgsOverride.overrideWithValue(_goldenTranslateArgs),
            keyboardPlatformProvider.overrideWithValue(TargetPlatform.macOS),
          ],
          child: MaterialApp(
            theme: UpegTheme.darkTheme(),
            debugShowCheckedModeBanner: false,
            home: ExpandedModalPage(tool: _fixtureTool),
          ),
        ),
      );
      await tester.pumpAndSettle();
      await expectLater(
        find.byType(MaterialApp),
        matchesGoldenFile('../goldens/expanded_modal_page.png'),
      );
    });

    testWidgets('TweaksForm_골든', tags: <String>['golden'], (tester) async {
      _useViewport(tester, const Size(420, 520));
      const initial = TweaksDto(
        theme: 'Dark',
        accent: 'Green',
        showHoles: true,
        locale: 'En',
        localHttpHost: false,
      );
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            tweaksLoaderProvider.overrideWith(
              (ref) =>
                  () => initial,
            ),
            tweaksSaverProvider.overrideWith((ref) => (TweaksDto _) {}),
            supportedLocalesProvider.overrideWith(
              (ref) =>
                  () => const <String>['En', 'Ko'],
            ),
            supportedThemesProvider.overrideWith(
              (ref) =>
                  () => const <String>['Light', 'Dark'],
            ),
            supportedAccentsProvider.overrideWith(
              (ref) =>
                  () => const <String>['Green', 'Amber', 'Cyan', 'Pink'],
            ),
            i18nTranslateOverride.overrideWithValue(_goldenTranslate),
            i18nTranslateArgsOverride.overrideWithValue(_goldenTranslateArgs),
            statusSnapshotProvider.overrideWith(_GoldenStatusNotifier.new),
            keyboardPlatformProvider.overrideWithValue(TargetPlatform.macOS),
          ],
          // The expanded section list now overflows the 420x520 golden
          // viewport — host it inside a SingleChildScrollView so the
          // golden stays stable on the visible top portion.
          child: _themed(const SingleChildScrollView(child: TweaksForm())),
        ),
      );
      await tester.pumpAndSettle();
      await expectLater(
        find.byType(MaterialApp),
        matchesGoldenFile('../goldens/tweaks_form.png'),
      );
    });

    testWidgets('PopupPage_골든', tags: <String>['golden'], (tester) async {
      _useViewport(tester, const Size(400, 540));
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            // R8: PopupPage restores the board selection on mount, which
            // reads `boardsProvider` / the persisted selection. Seed both
            // with realistic data so `PopupBoardTabs` renders a proper
            // tab strip (the pre-R8 golden baked in the FRB-init *error*
            // string because nothing seeded the loader).
            ...pegboardSelectionOverrides(boardKey: 'dev'),
            i18nTranslateOverride.overrideWithValue(_goldenTranslate),
            i18nTranslateArgsOverride.overrideWithValue(_goldenTranslateArgs),
            boardsLoaderProvider.overrideWith(
              (ref) =>
                  () => const <BoardDto>[
                    BoardDto(key: 'dev', title: 'Dev'),
                    BoardDto(key: 'media', title: 'Media'),
                  ],
            ),
            paletteSearcherProvider.overrideWith(
              (ref) =>
                  (q) => _fixtureHits,
            ),
            toolsLoaderProvider.overrideWith(
              (ref) =>
                  () => _popupCatalogueTools,
            ),
            paletteQueryProvider.overrideWith((ref) => ''),
            // Pin the status snapshot so the footer renders a stable
            // version label across hosts (M10 added the footer).
            statusSnapshotProvider.overrideWith(_GoldenStatusNotifier.new),
            keyboardPlatformProvider.overrideWithValue(TargetPlatform.macOS),
          ],
          child: MaterialApp(
            theme: UpegTheme.darkTheme(),
            debugShowCheckedModeBanner: false,
            home: const PopupPage(),
          ),
        ),
      );
      await tester.pumpAndSettle();
      await expectLater(
        find.byType(MaterialApp),
        matchesGoldenFile('../goldens/popup_page.png'),
      );
    });
  });
}
