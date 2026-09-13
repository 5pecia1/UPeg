import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/pages/popup_page.dart';
import 'package:upeg/src/rust/api/palette.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart' show PinKindDto, ToolDto;
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/status_provider.dart';
import 'package:upeg/src/state/window_mode_provider.dart';

import '../test_helpers/pegboard_selection_overrides.dart';

import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/popup_page_harness.dart';

void main() {
  group('PopupPage', () {
    testWidgets('esc는_popup_close를_즉시_처리한다', (tester) async {
      // Verifies that pressing Esc returns KeyEventResult.handled and
      // triggers the close path (popup mode → hide window).
      final hider = RecordingWindowHider();
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
                () => const <BoardDto>[],
          ),
          toolsLoaderProvider.overrideWith(
            (ref) =>
                () => const <ToolDto>[],
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

      // Esc should be handled (not ignored) and close the popup.
      // Send Escape through the search field's TextField since it owns focus.
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump();

      // Close was processed — the popup should have requested hide.
      // The window mode stays popup (hide is async, we just verify
      // the intent was dispatched, not the platform result).
      expect(container.read(windowModeProvider), WindowMode.popup);
      expect(hider.hideCount, 1);
    });

    testWidgets('full_gui에서_close는_popup으로_즉시_돌아간다', (tester) async {
      // When in full GUI mode, close (via Esc) immediately sets mode to popup.
      WindowMode? finalMode;

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
                () => const <BoardDto>[],
          ),
          toolsLoaderProvider.overrideWith(
            (ref) =>
                () => const <ToolDto>[],
          ),
          windowModeProvider.overrideWith(
            () => RecordingWindowModeNotifier(
              initial: WindowMode.full,
              onChange: (m) => finalMode = m,
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

      // Pressing Esc in full mode should switch to popup immediately.
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump();

      expect(finalMode, WindowMode.popup);
    });

    testWidgets('명시적_close는_esc와_같은_경로를_사용한다', (tester) async {
      // Both Esc key and explicit popupCloseProvider invocation should
      // dispatch through the same _handleClose() method. We verify by
      // using a single container: fire Esc (which registers the callback),
      // then fire explicit close and confirm identical state transition.
      final closeModes = <WindowMode>[];

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
                () => const <BoardDto>[],
          ),
          toolsLoaderProvider.overrideWith(
            (ref) =>
                () => const <ToolDto>[],
          ),
          windowModeProvider.overrideWith(
            () => RecordingWindowModeNotifier(
              initial: WindowMode.full,
              onChange: (m) => closeModes.add(m),
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

      // Path 1: Esc key → closes once (full → popup).
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump();

      // Reset to full so we can test Path 2 independently.
      container.read(windowModeProvider.notifier).set(WindowMode.full);

      // Wait a frame, then Path 2: explicit close via popupCloseProvider.
      await tester.pump();
      container.read(popupCloseProvider.notifier).handleClose();
      await tester.pump();

      // Both paths must produce the same state transition (full → popup).
      // closeModes contains: [Esc transition, manual reset, explicit transition]
      // We verify the two close transitions (index 0 and 2) are identical.
      expect(closeModes[0], WindowMode.popup);
      expect(closeModes[2], WindowMode.popup);
      expect(closeModes[0], closeModes[2]);
    });

    testWidgets('close는_지연이나_애니메이션을_사용하지_않는다', (tester) async {
      final closeModes = <WindowMode>[];
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
                () => const <BoardDto>[],
          ),
          toolsLoaderProvider.overrideWith(
            (ref) =>
                () => const <ToolDto>[],
          ),
          windowModeProvider.overrideWith(
            () => RecordingWindowModeNotifier(
              initial: WindowMode.full,
              onChange: closeModes.add,
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

      container.read(popupCloseProvider.notifier).handleClose();

      expect(closeModes, [WindowMode.popup]);
      expect(container.read(windowModeProvider), WindowMode.popup);
    });

    testWidgets('빈_검색은_6개로_자르지_않는다', (tester) async {
      // Explicitly verify the absence of a 6-item ceiling.
      final fakeHits = paletteHits(15, prefix: 'test.item_');
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

      final grid = tester.widget<GridView>(find.byType(GridView));
      final delegate = grid.childrenDelegate as SliverChildBuilderDelegate;
      expect(delegate.childCount, 15);
    });

    testWidgets('검색어가_있으면_기존_검색_결과를_사용한다', (tester) async {
      // Non-empty query must still flow through paletteResultsProvider.
      const searchHits = [
        PaletteHit(
          id: 'only.match',
          label: 'Only Match',
          description: '',
          score: 0.9,
          pinKind: PinKindDto.inline,
        ),
      ];
      await tester.pumpWidget(popupPageHarness(hits: searchHits));
      await tester.pumpAndSettle();

      await tester.enterText(
        find.byKey(const Key('popup-search-field')),
        'match',
      );
      await tester.pumpAndSettle();

      // Search result rendered, not the empty-query browse list.
      expect(
        find.byKey(const ValueKey('popup-hit-only.match')),
        findsOneWidget,
      );
    });

    testWidgets('popup을_다시_열면_이전_palette_query를_비우고_카탈로그를_표시한다', (
      tester,
    ) async {
      // Simulate a search session that leaves stale query in
      // `paletteQueryProvider`, then "reopen" the popup. The new mount
      // must clear the query and show the full catalogue.
      final fakeHits = paletteHits(8);

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

      // Phase 1: pump PopupPage, type a query to leave stale state.
      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(home: PopupPage()),
        ),
      );
      await tester.pumpAndSettle();

      await tester.enterText(
        find.byKey(const Key('popup-search-field')),
        'tool_',
      );
      await tester.pumpAndSettle();

      // Verify stale query is in the provider.
      expect(container.read(paletteQueryProvider), 'tool_');

      // Phase 2: "close" the popup by disposing the widget tree
      // (simulates hide-to-tray in popup-only mode).
      await tester.pumpWidget(Container());

      // Phase 3: "reopen" the popup — new PopupPage, same container.
      // A correct implementation clears paletteQueryProvider in initState,
      // so the catalogue (all 8 tools) should appear, not filtered results.
      container.read(paletteQueryProvider.notifier).state = 'tool_';

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(home: PopupPage()),
        ),
      );

      // Wait for async catalogue resolution.
      await tester.pump();
      await tester.pump();
      await tester.pumpAndSettle();

      // Assertion: query was cleared, full catalogue rendered.
      final grid = tester.widget<GridView>(find.byType(GridView));
      final delegate = grid.childrenDelegate as SliverChildBuilderDelegate;
      expect(
        delegate.childCount,
        8,
        reason:
            'popup reopen must show full catalogue, not stale query results',
      );
    });

    testWidgets('검색_진입은_async_로딩_후_카탈로그_도구를_보여준다', (tester) async {
      // Verify the async empty-state: when the user opens the popup/search
      // with an empty query, popupCatalogueHitsProvider resolves and shows
      // palette browse hits instead of an "empty hint" or "no tools match".
      final fakeHits = paletteHits(10, prefix: 'cat.tool_');

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

      // First pump: catalogue future is pending — shows loading spinner.
      await tester.pump();

      // Resolve the microtask, wait for the future to settle.
      await tester.pumpAndSettle();

      // Catalogue hits rendered as grid, no empty hint or no-results.
      expect(find.byType(GridView), findsOneWidget);
      final grid = tester.widget<GridView>(find.byType(GridView));
      final delegate = grid.childrenDelegate as SliverChildBuilderDelegate;
      expect(
        delegate.childCount,
        10,
        reason: 'should show all catalogue hits after async resolve',
      );
      expect(find.byKey(const Key('popup-empty-hint')), findsNothing);
      expect(find.byKey(const Key('popup-no-results')), findsNothing);
    });

    testWidgets('Popup_footer는_buildVersion을_렌더한다', (tester) async {
      // Use a dedicated container so we can override the same
      // statusSnapshotProvider basePopupOverrides covers — overriding the
      // provider twice in one container is a Riverpod-level error.
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            ...i18nTestOverrides,
            ...pegboardSelectionOverrides(boardKey: 'dev'),
            paletteSearcherProvider.overrideWith(
              (ref) =>
                  (String _) => const <PaletteHit>[],
            ),
            // R8: PopupPage restores the board selection on mount, which
            // reads `boardsProvider`; seed it so the restore is dylib-free.
            boardsLoaderProvider.overrideWith(
              (ref) =>
                  () => const <BoardDto>[],
            ),
            statusSnapshotProvider.overrideWith(
              () => FixedStatusNotifier(statusWithVersion('0.42.1')),
            ),
          ],
          child: const MaterialApp(home: PopupPage()),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text('v0.42.1'), findsOneWidget);
    });
  });
}
