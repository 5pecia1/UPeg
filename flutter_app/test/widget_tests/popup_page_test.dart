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
    testWidgets('popuppage_shows_the_search_field_and_empty_result_hint', (
      tester,
    ) async {
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

    testWidgets('popuppage_renders_search_results_as_a_list', (tester) async {
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

    testWidgets('the_popup_open_desktop_tab_switches_windowmode_to_full', (
      tester,
    ) async {
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

    testWidgets('popup_hits_render_as_a_two_column_grid', (tester) async {
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

    testWidgets(
      'an_empty_popup_query_shows_the_full_registered_tool_catalogue',
      (tester) async {
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
      },
    );

    testWidgets(
      'a_cold_boot_popup_shows_catalogue_tools_without_a_board_selection',
      (tester) async {
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
      },
    );

    testWidgets(
      'tapping_a_popup_cell_switches_windowmode_to_full_and_queues_the_pin_as_pending',
      (tester) async {
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
              () => RecordingWindowModeNotifier(
                onChange: (m) => observedMode = m,
              ),
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
      },
    );

    testWidgets('the_popup_header_renders_the_board_tab_strip', (tester) async {
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

    testWidgets('tapping_a_popup_board_tab_updates_currentboardkeyprovider', (
      tester,
    ) async {
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

    testWidgets('the_popup_board_tabs_move_the_selection_via_the_keyboard', (
      tester,
    ) async {
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

    testWidgets('popup_enter_activates_the_selected_hit', (tester) async {
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

    testWidgets('popup_arrowdown_moves_to_the_hit_on_the_next_row', (
      tester,
    ) async {
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

    testWidgets(
      'q_while_the_popup_search_field_is_focused_goes_to_text_input_not_close',
      (tester) async {
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
      },
    );

    testWidgets('an_empty_search_shows_every_registered_tool', (tester) async {
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
