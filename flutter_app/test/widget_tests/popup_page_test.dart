import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/pages/popup_page.dart';
import 'package:upeg/src/rust/api/palette.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart' show PinKindDto;
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/popup/popup_selection_provider.dart';
import 'package:upeg/src/state/pending_activation_provider.dart';
import 'package:upeg/src/state/window_mode_provider.dart';
import 'package:upeg/src/widgets/popup_board_tabs.dart';

import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/popup_page_harness.dart';

void main() {
  group('PopupPage', () {
    testWidgets('PopupPage_는_검색_필드와_빈_결과_안내를_표시한다', (tester) async {
      await tester.pumpWidget(popupPageHarness());
      await tester.pumpAndSettle();

      // Search field is present and focused.
      expect(find.byKey(const Key('popup-search-field')), findsOneWidget);

      // Empty-query hint shows by default.
      expect(find.byKey(const Key('popup-empty-hint')), findsOneWidget);

      // Typing a query that returns no results swaps to the
      // "no matches" indicator.
      await tester.enterText(
        find.byKey(const Key('popup-search-field')),
        'definitely-not-a-tool',
      );
      await tester.pumpAndSettle();
      expect(find.byKey(const Key('popup-no-results')), findsOneWidget);
    });

    testWidgets('PopupPage_는_검색_결과를_리스트로_렌더한다', (tester) async {
      const hits = [
        PaletteHit(
          id: 'num.hex_to_decimal',
          label: 'Hex → Dec',
          description: '',
          score: 1.0,
          pinKind: PinKindDto.inline,
        ),
        PaletteHit(
          id: 'id.uuid_v7',
          label: 'UUID v7',
          description: '',
          score: 1.0,
          pinKind: PinKindDto.inline,
        ),
      ];
      await tester.pumpWidget(popupPageHarness(hits: hits));
      await tester.pumpAndSettle();

      await tester.enterText(find.byKey(const Key('popup-search-field')), 'h');
      await tester.pumpAndSettle();

      expect(
        find.byKey(const ValueKey('popup-hit-num.hex_to_decimal')),
        findsOneWidget,
      );
      expect(
        find.byKey(const ValueKey('popup-hit-id.uuid_v7')),
        findsOneWidget,
      );
    });

    testWidgets('Popup_open_desktop_탭은_windowMode를_full로_전환한다', (tester) async {
      WindowMode? observed;
      await tester.pumpWidget(
        popupPageHarness(
          extraOverrides: [
            windowModeProvider.overrideWith(
              () => RecordingWindowModeNotifier(onChange: (m) => observed = m),
            ),
          ],
        ),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.text('open desktop'));
      await tester.pump();

      expect(observed, WindowMode.full);
    });

    testWidgets('Popup_hits는_2열_그리드로_렌더된다', (tester) async {
      const hits = [
        PaletteHit(
          id: 'a.one',
          label: 'one',
          description: '',
          score: 1.0,
          pinKind: PinKindDto.inline,
        ),
        PaletteHit(
          id: 'a.two',
          label: 'two',
          description: '',
          score: 1.0,
          pinKind: PinKindDto.inline,
        ),
        PaletteHit(
          id: 'a.three',
          label: 'three',
          description: '',
          score: 1.0,
          pinKind: PinKindDto.inline,
        ),
        PaletteHit(
          id: 'a.four',
          label: 'four',
          description: '',
          score: 1.0,
          pinKind: PinKindDto.inline,
        ),
      ];
      await tester.pumpWidget(popupPageHarness(hits: hits));
      await tester.pumpAndSettle();

      await tester.enterText(find.byKey(const Key('popup-search-field')), 'q');
      await tester.pumpAndSettle();

      expect(find.byType(GridView), findsOneWidget);
      final grid = tester.widget<GridView>(find.byType(GridView));
      final delegate =
          grid.gridDelegate as SliverGridDelegateWithFixedCrossAxisCount;
      expect(delegate.crossAxisCount, popupGridCols);
    });

    testWidgets('Popup_empty_query는_등록된_전체_도구_카탈로그를_표시한다', (tester) async {
      final fakeHits = paletteHits(8);
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            ...i18nTestOverrides,
            ...basePopupOverrides(),
            paletteBrowseOverride(fakeHits),
            boardsLoaderProvider.overrideWith(
              (ref) =>
                  () => const <BoardDto>[],
            ),
          ],
          child: const MaterialApp(home: PopupPage()),
        ),
      );
      await tester.pumpAndSettle();

      // Empty query: shows the full registered catalogue, not a 6-item slice.
      final grid = tester.widget<GridView>(find.byType(GridView));
      final delegate = grid.childrenDelegate as SliverChildBuilderDelegate;
      expect(delegate.childCount, 8);
    });

    testWidgets('Popup_콜드부트는_보드_선택_없이도_카탈로그_도구를_표시한다', (tester) async {
      // After removing the 6-pin cap, empty query uses the desktop-filtered
      // palette browse path instead of board placements. This verifies the
      // popup shows catalogue hits even without a pre-seeded board selection.
      final fakeHits = paletteHits(8);
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            ...i18nTestOverrides,
            ...basePopupOverrides(),
            paletteBrowseOverride(fakeHits),
            boardsLoaderProvider.overrideWith(
              (ref) =>
                  () => const <BoardDto>[],
            ),
            // Deliberately NO currentBoardKeyProvider override — the
            // catalogue does not depend on board selection.
          ],
          child: const MaterialApp(home: PopupPage()),
        ),
      );
      await tester.pumpAndSettle();

      // Catalogue tools rendered (8), no empty hint shown.
      final grid = tester.widget<GridView>(find.byType(GridView));
      final delegate = grid.childrenDelegate as SliverChildBuilderDelegate;
      expect(delegate.childCount, 8);
      expect(find.byKey(const Key('popup-empty-hint')), findsNothing);
    });

    testWidgets('Popup_cell_탭은_windowMode를_full로_바꾸고_pin을_pending에_큐잉한다', (
      tester,
    ) async {
      WindowMode? observedMode;
      final fakeHits = [paletteHit('num.hex_to_decimal', 'Hex → Dec')];

      final container = ProviderContainer(
        overrides: [
          ...i18nTestOverrides,
          ...basePopupOverrides(),
          paletteBrowseOverride(fakeHits),
          boardsLoaderProvider.overrideWith(
            (ref) =>
                () => const <BoardDto>[],
          ),
          windowModeProvider.overrideWith(
            () =>
                RecordingWindowModeNotifier(onChange: (m) => observedMode = m),
          ),
        ],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(home: PopupPage()),
        ),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.byType(PopupHitCell).first);
      await tester.pumpAndSettle();

      expect(observedMode, WindowMode.full);
      expect(
        container.read(pendingActivationProvider),
        ToolId.parse('num.hex_to_decimal'),
      );
    });

    testWidgets('Popup_헤더는_보드_탭_strip을_렌더한다', (tester) async {
      const boards = <BoardDto>[
        BoardDto(key: 'a', title: 'Alpha'),
        BoardDto(key: 'b', title: 'Bravo'),
        BoardDto(key: 'c', title: 'Charlie'),
      ];
      await tester.pumpWidget(popupPageHarness(boards: boards));
      await tester.pumpAndSettle();

      expect(find.byType(PopupBoardTabs), findsOneWidget);
      expect(find.text('Alpha'), findsOneWidget);
      expect(find.text('Bravo'), findsOneWidget);
      expect(find.text('Charlie'), findsOneWidget);
    });

    testWidgets('Popup_보드탭_탭은_currentBoardKeyProvider를_갱신한다', (tester) async {
      const boards = <BoardDto>[
        BoardDto(key: 'a', title: 'Alpha'),
        BoardDto(key: 'b', title: 'Bravo'),
      ];
      final container = ProviderContainer(
        overrides: [
          ...i18nTestOverrides,
          ...basePopupOverrides(),
          paletteSearcherProvider.overrideWith(
            (ref) =>
                (String _) => const <PaletteHit>[],
          ),
          boardsLoaderProvider.overrideWith(
            (ref) =>
                () => boards,
          ),
        ],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(home: PopupPage()),
        ),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.text('Bravo'));
      await tester.pumpAndSettle();

      expect(container.read(currentBoardKeyProvider), BoardKey.parse('b'));
    });

    testWidgets('Popup_보드탭은_키보드로_선택을_이동한다', (tester) async {
      const boards = <BoardDto>[
        BoardDto(key: 'a', title: 'Alpha'),
        BoardDto(key: 'b', title: 'Bravo'),
        BoardDto(key: 'c', title: 'Charlie'),
      ];
      final container = ProviderContainer(
        overrides: [
          ...i18nTestOverrides,
          ...basePopupOverrides(),
          paletteSearcherProvider.overrideWith(
            (ref) =>
                (String _) => const <PaletteHit>[],
          ),
          boardsLoaderProvider.overrideWith(
            (ref) =>
                () => boards,
          ),
        ],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(home: PopupPage()),
        ),
      );
      await tester.pumpAndSettle();

      Focus.of(
        tester.element(find.byKey(const ValueKey('popup-board-tab-a'))),
      ).requestFocus();
      await tester.pump();

      await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
      await tester.pump();
      expect(container.read(currentBoardKeyProvider), BoardKey.parse('b'));

      await tester.sendKeyEvent(LogicalKeyboardKey.end);
      await tester.pump();
      expect(container.read(currentBoardKeyProvider), BoardKey.parse('c'));

      await tester.sendKeyEvent(LogicalKeyboardKey.home);
      await tester.pump();
      expect(container.read(currentBoardKeyProvider), BoardKey.parse('a'));
    });

    testWidgets('Popup_Enter는_선택된_hit을_activate한다', (tester) async {
      final fakeHits = [
        paletteHit('test.tool_zero', 'Tool Zero'),
        paletteHit('test.tool_one', 'Tool One'),
        paletteHit('test.tool_two', 'Tool Two'),
      ];
      WindowMode? observed;
      final container = ProviderContainer(
        overrides: [
          ...i18nTestOverrides,
          ...basePopupOverrides(),
          paletteBrowseOverride(fakeHits),
          boardsLoaderProvider.overrideWith(
            (ref) =>
                () => const <BoardDto>[],
          ),
          windowModeProvider.overrideWith(
            () => RecordingWindowModeNotifier(onChange: (m) => observed = m),
          ),
        ],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(home: PopupPage()),
        ),
      );
      await tester.pumpAndSettle();

      // Default cursor is index 0. Pressing Enter activates the first
      // hit. Arrow-key navigation lives in the M09 cycle.
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();

      expect(observed, WindowMode.full);
      expect(
        container.read(pendingActivationProvider),
        ToolId.parse('test.tool_zero'),
      );
    });

    testWidgets('Popup_arrowDown은_아래_행의_hit으로_이동한다', (tester) async {
      final fakeHits = paletteHits(4);
      final container = ProviderContainer(
        overrides: [
          ...i18nTestOverrides,
          ...basePopupOverrides(),
          paletteBrowseOverride(fakeHits),
          boardsLoaderProvider.overrideWith(
            (ref) =>
                () => const <BoardDto>[],
          ),
        ],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(home: PopupPage()),
        ),
      );
      await tester.pumpAndSettle();

      expect(container.read(popupSelectionProvider).index, 0);

      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      await tester.pump();

      expect(container.read(popupSelectionProvider).index, 2);

      // Press Enter — the wiring should now activate index 2 (not 0).
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();

      expect(
        container.read(pendingActivationProvider),
        ToolId.parse('test.tool_2'),
      );
    });

    testWidgets('Popup_검색_field_focus중_q는_close가_아니라_text_input에_맡긴다', (
      tester,
    ) async {
      final hider = RecordingWindowHider();
      final container = ProviderContainer(
        overrides: [
          ...i18nTestOverrides,
          ...basePopupOverrides(),
          paletteBrowseOverride(paletteHits(3)),
          boardsLoaderProvider.overrideWith(
            (ref) =>
                () => const <BoardDto>[],
          ),
          windowModeProvider.overrideWith(
            () => RecordingWindowModeNotifier(
              initial: WindowMode.popup,
              onChange: (_) {},
            ),
          ),
          popupWindowHiderProvider.overrideWithValue(hider),
        ],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(home: PopupPage()),
        ),
      );
      await tester.pumpAndSettle();
      await tester.showKeyboard(find.byKey(const Key('popup-search-field')));

      final focus = tester.widget<Focus>(popupRootFocusFinder());
      final result = focus.onKeyEvent!(
        focus.focusNode!,
        KeyDownEvent(
          physicalKey: PhysicalKeyboardKey.keyQ,
          logicalKey: LogicalKeyboardKey.keyQ,
          character: 'q',
          timeStamp: Duration.zero,
        ),
      );

      expect(result, KeyEventResult.ignored);
      expect(hider.hideCount, 0);
      expect(container.read(windowModeProvider), WindowMode.popup);
    });

    testWidgets('빈_검색은_등록된_모든_도구를_보여준다', (tester) async {
      // Register more tools than the old 6-item cap.
      final fakeHits = paletteHits(10, prefix: 'test.tool_');
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            ...i18nTestOverrides,
            ...basePopupOverrides(),
            paletteBrowseOverride(fakeHits),
            boardsLoaderProvider.overrideWith(
              (ref) =>
                  () => const <BoardDto>[],
            ),
          ],
          child: const MaterialApp(home: PopupPage()),
        ),
      );
      await tester.pumpAndSettle();

      // GridView itemCount (via its delegate) should reflect all 10 tools.
      final grid = tester.widget<GridView>(find.byType(GridView));
      final delegate = grid.childrenDelegate as SliverChildBuilderDelegate;
      expect(delegate.childCount, greaterThan(6));
      expect(delegate.childCount, 10);
    });
  });
}
