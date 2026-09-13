import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/i18n.dart' show LocaleDto;
import 'package:upeg/src/rust/api/palette.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart' show PinKindDto, ToolDto;
import 'package:upeg/src/widgets/expanded_modal/kind_badge.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/focused_pin_provider.dart';
import 'package:upeg/src/state/locale_provider.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/widgets/palette_overlay.dart';

import '../test_helpers/fake_keyboard_resolver.dart';
import '../test_helpers/pegboard_selection_overrides.dart';

const Map<String, Map<LocaleDto, String>> _fakeCatalog = {
  'palette.placeholder': {
    LocaleDto.en: 'search tools, paste, or type a command…',
    LocaleDto.ko: '도구 검색, 붙여넣기 또는 명령 입력…',
  },
  'palette.empty_toolbox': {
    LocaleDto.en: 'toolbox empty.',
    LocaleDto.ko: '툴박스가 비어 있습니다.',
  },
  'palette.footer.navigate': {LocaleDto.en: 'navigate', LocaleDto.ko: '이동'},
  'palette.footer.open': {LocaleDto.en: 'open', LocaleDto.ko: '열기'},
  'palette.footer.open_pin': {
    LocaleDto.en: 'open + pin',
    LocaleDto.ko: '열기 + 핀',
  },
};

String _fakeTranslate(String key, LocaleDto locale) {
  return _fakeCatalog[key]?[locale] ?? key;
}

String _fakeTranslateArgs(
  String key,
  LocaleDto locale,
  List<String> argKeys,
  List<String> argVals,
) {
  if (key == 'palette.no_match') {
    final needleIdx = argKeys.indexOf('needle');
    final needle = needleIdx >= 0 ? argVals[needleIdx] : '';
    return switch (locale) {
      LocaleDto.en => 'no tools match "$needle"',
      LocaleDto.ko => '"$needle"와(과) 일치하는 도구가 없습니다',
    };
  }
  if (key == 'palette.footer.summary') {
    final countIdx = argKeys.indexOf('count');
    final totalIdx = argKeys.indexOf('total');
    final count = countIdx >= 0 ? argVals[countIdx] : '';
    final total = totalIdx >= 0 ? argVals[totalIdx] : '';
    return switch (locale) {
      LocaleDto.en => '$count tools · $total boards',
      LocaleDto.ko => '도구 $count개 · 보드 $total개',
    };
  }
  return key;
}

/// Builds a PaletteOverlay with the FRB `searchTools` boundary swapped for
/// an in-memory matcher so widget tests never load the native dylib.
Widget _harness({
  required List<PaletteHit> hits,
  required void Function(PaletteHit) onPick,
  Widget Function(PaletteHitCallback onPick)? childBuilder,
  List<BoardDto> boards = const <BoardDto>[BoardDto(key: 'dev', title: 'Dev')],
  Set<BoardKey> pinnedBoards = const <BoardKey>{},
  LocaleDto locale = LocaleDto.en,
  String? currentBoardKey,
  PinMutator? pinTool,
  List<ToolDto>? tools,
}) {
  return ProviderScope(
    overrides: [
      ...pegboardSelectionOverrides(boardKey: currentBoardKey),
      paletteSearcherProvider.overrideWith((ref) {
        return (query) {
          final needle = query.toLowerCase();
          return [
            for (final hit in hits)
              if (hit.id.toLowerCase().contains(needle) ||
                  hit.label.toLowerCase().contains(needle))
                hit,
          ];
        };
      }),
      boardsLoaderProvider.overrideWith(
        (ref) =>
            () => boards,
      ),
      pinnedBoardsLoaderProvider.overrideWith(
        (ref) =>
            (toolId) => pinnedBoards,
      ),
      localeProvider.overrideWithValue(locale),
      i18nTranslateOverride.overrideWithValue(_fakeTranslate),
      i18nTranslateArgsOverride.overrideWithValue(_fakeTranslateArgs),
      fakeKeyboardResolverOverride,
      if (pinTool != null) pinToolMutatorProvider.overrideWithValue(pinTool),
      if (tools != null)
        toolsLoaderProvider.overrideWith(
          (ref) =>
              () => tools,
        ),
    ],
    child: MaterialApp(
      home: _PaletteHarnessHost(
        currentBoardKey: currentBoardKey,
        child: childBuilder?.call(onPick) ?? PaletteOverlay(onPick: onPick),
      ),
    ),
  );
}

/// Drives `currentBoardKeyProvider` via a `Notifier.select` call before
/// the PaletteOverlay first paints, so widget tests can exercise the
/// Cmd+Enter pin-and-open path without needing a full BoardPage.
class _PaletteHarnessHost extends ConsumerStatefulWidget {
  const _PaletteHarnessHost({
    required this.currentBoardKey,
    required this.child,
  });

  final String? currentBoardKey;
  final Widget child;

  @override
  ConsumerState<_PaletteHarnessHost> createState() =>
      _PaletteHarnessHostState();
}

class _PaletteHarnessHostState extends ConsumerState<_PaletteHarnessHost> {
  @override
  void initState() {
    super.initState();
    final key = widget.currentBoardKey;
    if (key != null) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        ref.read(currentBoardKeyProvider.notifier).select(BoardKey.parse(key));
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(body: widget.child);
  }
}

class _PaletteDialogButton extends StatelessWidget {
  const _PaletteDialogButton({required this.onPick});

  final PaletteHitCallback onPick;

  @override
  Widget build(BuildContext context) {
    return ElevatedButton(
      key: const Key('open-palette-dialog'),
      onPressed: () => showPaletteOverlay(context, onPick: onPick),
      child: const Text('open'),
    );
  }
}

void main() {
  group('PaletteOverlay', () {
    testWidgets('PaletteOverlay는_빈_쿼리에서도_핀할_도구_목록을_보여준다', (tester) async {
      const hits = [
        PaletteHit(
          id: 'num.hex_to_decimal',
          label: 'hex → dec',
          description: '',
          score: 1.0,
          pinKind: PinKindDto.inline,
        ),
      ];
      await tester.pumpWidget(_harness(hits: hits, onPick: (_) {}));
      await tester.pumpAndSettle();

      expect(
        find.byKey(const ValueKey('palette-hit-num.hex_to_decimal')),
        findsOneWidget,
      );
      expect(
        find.byKey(const Key('palette-board-chips-num.hex_to_decimal')),
        findsOneWidget,
      );
      expect(find.byKey(const Key('palette-empty-hint')), findsNothing);
    });

    testWidgets('PaletteOverlay는_검색어_없이_board_chip_클릭으로_pin한다', (tester) async {
      PaletteHit? picked;
      final pinCalls = <(BoardKey, ToolId)>[];
      const hits = [
        PaletteHit(
          id: 'num.hex_to_decimal',
          label: 'hex → dec',
          description: '',
          score: 1.0,
          pinKind: PinKindDto.inline,
        ),
      ];
      await tester.pumpWidget(
        _harness(
          hits: hits,
          onPick: (hit) => picked = hit,
          currentBoardKey: 'dev',
          pinTool: (board, tool) {
            pinCalls.add((board, tool));
          },
        ),
      );
      await tester.pumpAndSettle();

      await tester.tap(
        find.byKey(const Key('palette-board-chip-num.hex_to_decimal-dev')),
      );
      await tester.pumpAndSettle();

      expect(picked, isNull, reason: 'board chip tap should only pin');
      expect(pinCalls, [
        (BoardKey.parse('dev'), ToolId.parse('num.hex_to_decimal')),
      ]);
    });

    testWidgets('PaletteOverlay는_locale_Ko에서_검색_placeholder가_한국어로_바뀐다', (
      tester,
    ) async {
      await tester.pumpWidget(
        _harness(hits: const [], onPick: (_) {}, locale: LocaleDto.ko),
      );
      await tester.pumpAndSettle();

      expect(
        find.text('search tools, paste, or type a command…'),
        findsNothing,
      );
      expect(find.text('도구 검색, 붙여넣기 또는 명령 입력…'), findsWidgets);
    });

    testWidgets('PaletteOverlay는_쿼리에_맞는_hit를_렌더한다', (tester) async {
      const hits = [
        PaletteHit(
          id: 'num.hex_to_decimal',
          label: 'hex → dec',
          description: 'hex string to decimal',
          score: 1.0,
          pinKind: PinKindDto.inline,
        ),
        PaletteHit(
          id: 'convert.base64_decode',
          label: 'base64 decode',
          description: '',
          score: 0.5,
          pinKind: PinKindDto.inline,
        ),
      ];
      await tester.pumpWidget(_harness(hits: hits, onPick: (_) {}));

      await tester.enterText(find.byType(TextField), 'hex');
      await tester.pumpAndSettle();

      expect(
        find.byKey(const ValueKey('palette-hit-num.hex_to_decimal')),
        findsOneWidget,
      );
      expect(
        find.byKey(const ValueKey('palette-hit-convert.base64_decode')),
        findsNothing,
      );
      expect(find.text('hex → dec'), findsOneWidget);
    });

    testWidgets('PaletteOverlay_row는_pinKind_badge를_표시한다', (tester) async {
      // H11 — each palette row mounts a `KindBadge(pinKind: ...)` so
      // the user can tell at a glance what kind of pin the result is.
      const hits = [
        PaletteHit(
          id: 'num.hex_to_decimal',
          label: 'hex → dec',
          description: '',
          score: 1.0,
          pinKind: PinKindDto.inline,
        ),
        PaletteHit(
          id: 'web.transform_tools',
          label: 'transform.tools',
          description: '',
          score: 0.5,
          pinKind: PinKindDto.embed,
        ),
      ];
      await tester.pumpWidget(_harness(hits: hits, onPick: (_) {}));

      // Query that matches both hits via case-insensitive substring.
      await tester.enterText(find.byType(TextField), 'o');
      await tester.pumpAndSettle();

      // Every visible row should mount a KindBadge — assert at least
      // one per hit.
      expect(find.byType(KindBadge), findsNWidgets(hits.length));
    });

    testWidgets('PaletteOverlay는_매치가_없으면_no_results를_표시한다', (tester) async {
      const hits = [
        PaletteHit(
          id: 'num.hex_to_decimal',
          label: 'hex → dec',
          description: '',
          score: 1.0,
          pinKind: PinKindDto.inline,
        ),
      ];
      await tester.pumpWidget(_harness(hits: hits, onPick: (_) {}));

      await tester.enterText(find.byType(TextField), 'zzzzz');
      await tester.pumpAndSettle();

      expect(find.byKey(const Key('palette-no-results')), findsOneWidget);
      // The no-match cell echoes the trimmed query so the user sees
      // what they typed.
      // Translated through `palette.no_match` with `{needle}` arg.
      expect(find.text('no tools match "zzzzz"'), findsOneWidget);
    });

    testWidgets('PaletteOverlay는_toolbox가_비어있을때_empty_toolbox_메시지를_표시한다', (
      tester,
    ) async {
      // H07 — when no tools are registered at all, the palette must
      // surface the localised `palette.empty_toolbox` copy instead of
      // the generic "type to search" placeholder. The empty-toolbox
      // copy shows only when the toolbox has no items AND the query
      // is empty.
      //
      // Override `toolsLoaderProvider` with an empty list and pass an
      // empty palette hits list so the searcher would never return a
      // match. Empty query → must show the empty-toolbox copy, NOT
      // the placeholder.
      await tester.pumpWidget(
        _harness(hits: const [], tools: const <ToolDto>[], onPick: (_) {}),
      );
      await tester.pumpAndSettle();

      // The empty-toolbox surface must mount with a stable key so the
      // BoardPage / popup can hook telemetry on the same widget.
      expect(
        find.byKey(const Key('palette-empty-toolbox')),
        findsOneWidget,
        reason:
            'Empty toolbox must use the dedicated `palette-empty-toolbox` key',
      );
      expect(find.text('toolbox empty.'), findsWidgets);
      // And the bare placeholder hint MUST NOT appear — the palette
      // distinguishes "type to search" (toolbox has tools) from
      // "toolbox is empty" (no tools registered at all).
      expect(
        find.byKey(const Key('palette-empty-hint')),
        findsNothing,
        reason:
            'Placeholder hint must NOT mount when the toolbox itself is empty',
      );
    });

    testWidgets('PaletteOverlay_CmdEnter는_선택_hit를_open하고_pin한다', (
      tester,
    ) async {
      // H06 — Cmd/Ctrl+Enter must both dispatch the hit (open the
      // expanded modal) AND pin the tool onto the current board. The
      // Cmd/Ctrl modifier is read off the keyboard event itself.
      PaletteHit? picked;
      final pinCalls = <(BoardKey, ToolId)>[];
      const hits = [
        PaletteHit(
          id: 'num.hex_to_decimal',
          label: 'hex → dec',
          description: '',
          score: 1.0,
          pinKind: PinKindDto.inline,
        ),
      ];
      await tester.pumpWidget(
        _harness(
          hits: hits,
          onPick: (hit) => picked = hit,
          currentBoardKey: 'dev',
          pinTool: (board, tool) {
            pinCalls.add((board, tool));
          },
        ),
      );

      await tester.enterText(find.byType(TextField), 'hex');
      await tester.pumpAndSettle();

      // Cmd+Enter (macOS-style metaModifier). The Flutter
      // shortcut layer accepts either meta OR control.
      await tester.sendKeyDownEvent(LogicalKeyboardKey.metaLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.metaLeft);
      await tester.pumpAndSettle();

      expect(picked?.id, 'num.hex_to_decimal', reason: 'open should fire');
      expect(pinCalls, [
        (BoardKey.parse('dev'), ToolId.parse('num.hex_to_decimal')),
      ], reason: 'pin should fire with current board key + tool id');
    });

    testWidgets('PaletteOverlay_ArrowDown_Enter는_highlight된_hit를_선택한다', (
      tester,
    ) async {
      PaletteHit? picked;
      const hits = [
        PaletteHit(
          id: 'num.hex_to_decimal',
          label: 'hex -> dec',
          description: '',
          score: 1.0,
          pinKind: PinKindDto.inline,
        ),
        PaletteHit(
          id: 'convert.base64_decode',
          label: 'base64 decode',
          description: '',
          score: 0.9,
          pinKind: PinKindDto.inline,
        ),
      ];
      await tester.pumpWidget(
        _harness(hits: hits, onPick: (hit) => picked = hit),
      );

      await tester.enterText(find.byType(TextField), 'convert');
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();

      expect(picked?.id, 'convert.base64_decode');
    });

    testWidgets('PaletteOverlay_Escape는_transition없이_dialog를_닫는다', (
      tester,
    ) async {
      const hits = [
        PaletteHit(
          id: 'num.hex_to_decimal',
          label: 'hex -> dec',
          description: '',
          score: 1.0,
          pinKind: PinKindDto.inline,
        ),
      ];
      await tester.pumpWidget(
        _harness(
          hits: hits,
          onPick: (_) {},
          childBuilder: (onPick) => _PaletteDialogButton(onPick: onPick),
        ),
      );

      await tester.tap(find.byKey(const Key('open-palette-dialog')));
      await tester.pump();
      expect(find.byType(PaletteOverlay), findsOneWidget);

      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump();

      expect(find.byType(PaletteOverlay), findsNothing);
    });

    testWidgets('PaletteOverlay_footer는_매치_개수_summary를_표시한다', (tester) async {
      // H09 — footer shows a match-count summary.
      // Two hits → "2 tools" / "2 도구" — body uses the trimmed query
      // match count via the `palette.count` template.
      const hits = [
        PaletteHit(
          id: 'num.hex_to_decimal',
          label: 'hex → dec',
          description: '',
          score: 1.0,
          pinKind: PinKindDto.inline,
        ),
        PaletteHit(
          id: 'convert.base64_decode',
          label: 'base64 decode',
          description: '',
          score: 0.9,
          pinKind: PinKindDto.inline,
        ),
      ];
      await tester.pumpWidget(_harness(hits: hits, onPick: (_) {}));

      await tester.enterText(find.byType(TextField), 'e');
      await tester.pumpAndSettle();

      expect(find.byKey(const Key('palette-count-summary')), findsOneWidget);
      expect(find.text('navigate'), findsOneWidget);
      expect(find.text('open'), findsOneWidget);
      expect(find.text('open + pin'), findsOneWidget);
      // The label text should contain the integer count of visible hits.
      final summaryText =
          (tester.widget(find.byKey(const Key('palette-count-summary')))
                  as Text)
              .data;
      expect(summaryText, contains('2'));
    });

    testWidgets('PaletteOverlay는_9개_초과_결과도_모두_표시한다', (tester) async {
      // The search surface must expose every returned tool. The dialog
      // height stays bounded; the ListView scrolls instead of hiding rows.
      final hits = List<PaletteHit>.generate(
        12,
        (i) => PaletteHit(
          id: 'test.tool_$i',
          label: 'tool $i',
          description: '',
          score: 1.0 - (i * 0.01),
          pinKind: PinKindDto.inline,
        ),
      );
      await tester.pumpWidget(_harness(hits: hits, onPick: (_) {}));

      await tester.enterText(find.byType(TextField), 'tool');
      await tester.pumpAndSettle();

      final list = tester.widget<ListView>(find.byType(ListView));
      final delegate = list.childrenDelegate as SliverChildBuilderDelegate;
      expect(delegate.childCount, 12);
      await tester.scrollUntilVisible(
        find.byKey(const ValueKey('palette-hit-test.tool_11')),
        120,
        scrollable: find.byType(Scrollable).last,
      );
      expect(find.text('tool 11'), findsOneWidget);
    });

    testWidgets('PaletteOverlay_ArrowDown은_결과가_많을때_스크롤하여_highlighted_행을_표시한다', (
      tester,
    ) async {
      // Generate enough hits to overflow the 480px palette height.
      final hits = List<PaletteHit>.generate(
        20,
        (i) => PaletteHit(
          id: 'test.tool_$i',
          label: 'tool $i',
          description: '',
          score: 1.0 - (i * 0.01),
          pinKind: PinKindDto.inline,
        ),
      );
      await tester.pumpWidget(_harness(hits: hits, onPick: (_) {}));

      await tester.enterText(find.byType(TextField), 'tool');
      await tester.pumpAndSettle();

      // Navigate to the 15th item (index 14) which should be off-screen.
      for (var i = 0; i < 14; i++) {
        await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
        await tester.pump();
      }
      // Wait for scroll animation to complete.
      await tester.pumpAndSettle(const Duration(milliseconds: 200));

      // Verify the highlighted item is visible by finding its text.
      expect(find.text('tool 14'), findsOneWidget);
    });

    testWidgets('PaletteOverlay는_hit_탭_시_onPick을_호출한다', (tester) async {
      PaletteHit? picked;
      const hits = [
        PaletteHit(
          id: 'num.hex_to_decimal',
          label: 'hex → dec',
          description: '',
          score: 1.0,
          pinKind: PinKindDto.inline,
        ),
      ];
      await tester.pumpWidget(
        _harness(hits: hits, onPick: (hit) => picked = hit),
      );

      await tester.enterText(find.byType(TextField), 'hex');
      await tester.pumpAndSettle();
      await tester.tap(
        find.byKey(const ValueKey('palette-hit-num.hex_to_decimal')),
      );
      await tester.pumpAndSettle();

      expect(picked?.id, 'num.hex_to_decimal');
    });

    testWidgets('PaletteOverlay_Enter_실행은_핀된_보드로_전환하고_핀에_포커스한다', (
      tester,
    ) async {
      // 팔레트 ↔ 보드 통합: 다른 보드(ops)에만 핀된 tool을 Enter로 실행하면
      // 실행 전에 그 보드로 전환하고 focusedPinProvider를 설정해
      // BoardCanvas의 scroll-into-view가 핀을 드러낸다.
      PaletteHit? picked;
      const hits = [
        PaletteHit(
          id: 'num.hex_to_decimal',
          label: 'hex → dec',
          description: '',
          score: 1.0,
          pinKind: PinKindDto.inline,
        ),
      ];
      await tester.pumpWidget(
        _harness(
          hits: hits,
          onPick: (hit) => picked = hit,
          currentBoardKey: 'dev',
          boards: const [
            BoardDto(key: 'dev', title: 'Dev'),
            BoardDto(key: 'ops', title: 'Ops'),
          ],
          pinnedBoards: {BoardKey.parse('ops')},
        ),
      );
      await tester.pumpAndSettle();

      await tester.enterText(find.byType(TextField), 'hex');
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();

      final container = ProviderScope.containerOf(
        tester.element(find.byType(MaterialApp)),
      );
      expect(picked?.id, 'num.hex_to_decimal');
      expect(
        container.read(currentBoardKeyProvider),
        BoardKey.parse('ops'),
        reason: '핀이 있는 보드로 전환해야 한다',
      );
      expect(
        container.read(focusedPinProvider),
        ToolId.parse('num.hex_to_decimal'),
        reason: '실행한 tool의 핀에 포커스해야 한다',
      );
    });

    testWidgets('PaletteOverlay_Enter_실행은_현재_보드에_핀이_있으면_보드를_유지한다', (
      tester,
    ) async {
      PaletteHit? picked;
      const hits = [
        PaletteHit(
          id: 'num.hex_to_decimal',
          label: 'hex → dec',
          description: '',
          score: 1.0,
          pinKind: PinKindDto.inline,
        ),
      ];
      await tester.pumpWidget(
        _harness(
          hits: hits,
          onPick: (hit) => picked = hit,
          currentBoardKey: 'dev',
          boards: const [
            BoardDto(key: 'dev', title: 'Dev'),
            BoardDto(key: 'ops', title: 'Ops'),
          ],
          pinnedBoards: {BoardKey.parse('dev'), BoardKey.parse('ops')},
        ),
      );
      await tester.pumpAndSettle();

      await tester.enterText(find.byType(TextField), 'hex');
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();

      final container = ProviderScope.containerOf(
        tester.element(find.byType(MaterialApp)),
      );
      expect(picked?.id, 'num.hex_to_decimal');
      expect(
        container.read(currentBoardKeyProvider),
        BoardKey.parse('dev'),
        reason: '현재 보드에 이미 핀이 있으면 보드를 유지한다',
      );
      expect(
        container.read(focusedPinProvider),
        ToolId.parse('num.hex_to_decimal'),
      );
    });

    testWidgets('PaletteOverlay_Enter_실행은_핀이_없으면_점프하지_않는다', (tester) async {
      PaletteHit? picked;
      const hits = [
        PaletteHit(
          id: 'num.hex_to_decimal',
          label: 'hex → dec',
          description: '',
          score: 1.0,
          pinKind: PinKindDto.inline,
        ),
      ];
      await tester.pumpWidget(
        _harness(
          hits: hits,
          onPick: (hit) => picked = hit,
          currentBoardKey: 'dev',
          boards: const [
            BoardDto(key: 'dev', title: 'Dev'),
            BoardDto(key: 'ops', title: 'Ops'),
          ],
        ),
      );
      await tester.pumpAndSettle();

      await tester.enterText(find.byType(TextField), 'hex');
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();

      final container = ProviderScope.containerOf(
        tester.element(find.byType(MaterialApp)),
      );
      expect(picked?.id, 'num.hex_to_decimal', reason: '실행 자체는 기존대로 수행한다');
      expect(container.read(currentBoardKeyProvider), BoardKey.parse('dev'));
      expect(
        container.read(focusedPinProvider),
        isNull,
        reason: '핀이 없으면 포커스할 대상이 없다',
      );
    });

    testWidgets('PaletteOverlay_CmdEnter는_현재_보드에_핀하고_그_핀에_포커스한다', (
      tester,
    ) async {
      // ⌘↵는 pin 쓰기가 아직 in-flight여도 방금 핀한 현재 보드를 점프
      // 대상으로 취급해 focusedPinProvider를 설정해야 한다.
      PaletteHit? picked;
      final pinCalls = <(BoardKey, ToolId)>[];
      const hits = [
        PaletteHit(
          id: 'num.hex_to_decimal',
          label: 'hex → dec',
          description: '',
          score: 1.0,
          pinKind: PinKindDto.inline,
        ),
      ];
      await tester.pumpWidget(
        _harness(
          hits: hits,
          onPick: (hit) => picked = hit,
          currentBoardKey: 'dev',
          pinTool: (board, tool) => pinCalls.add((board, tool)),
        ),
      );

      await tester.enterText(find.byType(TextField), 'hex');
      await tester.pumpAndSettle();
      await tester.sendKeyDownEvent(LogicalKeyboardKey.metaLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.metaLeft);
      await tester.pumpAndSettle();

      final container = ProviderScope.containerOf(
        tester.element(find.byType(MaterialApp)),
      );
      expect(picked?.id, 'num.hex_to_decimal');
      expect(pinCalls, [
        (BoardKey.parse('dev'), ToolId.parse('num.hex_to_decimal')),
      ]);
      expect(container.read(currentBoardKeyProvider), BoardKey.parse('dev'));
      expect(
        container.read(focusedPinProvider),
        ToolId.parse('num.hex_to_decimal'),
        reason: '방금 핀한 보드의 핀에 포커스해야 한다',
      );
    });

    testWidgets('PaletteOverlay는_핀된_board_chip_클릭으로_실행없이_그_보드로_점프한다', (
      tester,
    ) async {
      // 요구 2 — 채워진(핀된) 보드 칩은 점프 버튼: 보드 전환 + 핀 포커스 +
      // 팔레트 닫기. onPick(실행)은 호출되지 않는다.
      PaletteHit? picked;
      const hits = [
        PaletteHit(
          id: 'num.hex_to_decimal',
          label: 'hex → dec',
          description: '',
          score: 1.0,
          pinKind: PinKindDto.inline,
        ),
      ];
      await tester.pumpWidget(
        _harness(
          hits: hits,
          onPick: (hit) => picked = hit,
          currentBoardKey: 'dev',
          boards: const [
            BoardDto(key: 'dev', title: 'Dev'),
            BoardDto(key: 'ops', title: 'Ops'),
          ],
          pinnedBoards: {BoardKey.parse('ops')},
          childBuilder: (onPick) => _PaletteDialogButton(onPick: onPick),
        ),
      );
      await tester.tap(find.byKey(const Key('open-palette-dialog')));
      await tester.pumpAndSettle();

      await tester.tap(
        find.byKey(const Key('palette-board-chip-num.hex_to_decimal-ops')),
      );
      await tester.pumpAndSettle();

      final container = ProviderScope.containerOf(
        tester.element(find.byType(MaterialApp)),
      );
      expect(picked, isNull, reason: '칩 점프는 실행 없이 이동만 한다');
      expect(container.read(currentBoardKeyProvider), BoardKey.parse('ops'));
      expect(
        container.read(focusedPinProvider),
        ToolId.parse('num.hex_to_decimal'),
      );
      expect(
        find.byType(PaletteOverlay),
        findsNothing,
        reason: '점프 후 팔레트는 닫혀야 한다',
      );
    });

    testWidgets('PaletteOverlay_board_chip_라벨은_slug가_아니라_board_title이다', (
      tester,
    ) async {
      const hits = [
        PaletteHit(
          id: 'num.hex_to_decimal',
          label: 'hex → dec',
          description: '',
          score: 1.0,
          pinKind: PinKindDto.inline,
        ),
      ];
      await tester.pumpWidget(
        _harness(
          hits: hits,
          onPick: (_) {},
          boards: const [BoardDto(key: 'dev', title: 'Dev Board')],
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text('Dev Board'), findsOneWidget);
      expect(find.text('dev'), findsNothing, reason: 'slug는 노출하지 않는다');
    });
  });
}
