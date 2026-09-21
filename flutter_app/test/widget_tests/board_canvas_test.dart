import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/pegboard_selection_provider.dart';
import 'package:upeg/src/state/tag_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/board_canvas.dart';
import 'package:upeg/src/widgets/pin.dart';
import 'package:upeg/src/widgets/tag_chips.dart';

import '../test_helpers/pegboard_selection_overrides.dart';
import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/board_canvas_harness.dart';

void main() {
  group('BoardCanvas', () {
    testWidgets('BoardCanvas_shows_hint_text_for_an_empty_layout', (
      tester,
    ) async {
      await tester.pumpWidget(
        boardCanvasHarness(
          snapshot: const LayoutSnapshotDto(
            boardKey: 'dev',
            boardCols: 6,
            placements: <PlacementDto>[],
          ),
        ),
      );

      expect(find.byKey(const Key('board-canvas-empty')), findsOneWidget);
      expect(find.text(emptyBoardHint), findsOneWidget);
    });

    testWidgets('BoardCanvas_renders_a_Pin_for_each_placement', (tester) async {
      await tester.pumpWidget(
        boardCanvasHarness(
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
              PlacementDto(
                toolId: 'convert.base64_decode',
                x: 1,
                y: 0,
                w: 1,
                h: 1,
              ),
            ],
          ),
        ),
      );

      expect(find.byType(Pin), findsNWidgets(2));
      // The Pin header renders the canonical id in upper case
      // (typography inherited from the retired Dioxus surface). The
      // body label falls back to the id when the catalog is empty.
      expect(find.text('NUM.HEX_TO_DECIMAL'), findsOneWidget);
      expect(find.text('CONVERT.BASE64_DECODE'), findsOneWidget);
    });

    testWidgets('BoardCanvas_renders_a_U2_pin_at_double_width', (tester) async {
      await tester.pumpWidget(
        boardCanvasHarness(
          snapshot: const LayoutSnapshotDto(
            boardKey: 'dev',
            boardCols: 6,
            placements: [
              PlacementDto(toolId: 'fake.u2', x: 0, y: 0, w: 2, h: 1),
            ],
          ),
        ),
      );

      // The Pin renders inside a SizedBox sized from `placement.w` ×
      // pinCellWidth + gap; U2 picks the 2-cell variant.
      final pinFinder = find.byType(Pin);
      expect(pinFinder, findsOneWidget);
      final size = tester.getSize(pinFinder);
      // 168 * 2 + 8 = 344 (cell width + gap).
      expect(size.width, closeTo(344, 0.1));
    });

    testWidgets('BoardCanvas_uses_placement_x_y_as_cell_coordinates_verbatim', (
      tester,
    ) async {
      await tester.pumpWidget(
        boardCanvasHarness(
          snapshot: const LayoutSnapshotDto(
            boardKey: 'dev',
            boardCols: 6,
            placements: [
              PlacementDto(
                toolId: 'num.hex_to_decimal',
                x: 2,
                y: 3,
                w: 1,
                h: 1,
              ),
            ],
          ),
        ),
      );

      final positioned = tester.widget<Positioned>(
        find.ancestor(of: find.byType(Pin), matching: find.byType(Positioned)),
      );

      expect(
        positioned.left,
        2 * (UpegSizing.pinCellWidth + UpegSizing.pinGap),
      );
      expect(
        positioned.top,
        3 * (boardCanvasPinCellHeight + UpegSizing.pinGap),
      );
    });

    testWidgets('the_BoardCanvas_canvas_size_includes_the_trailing_row', (
      tester,
    ) async {
      await tester.pumpWidget(
        boardCanvasHarness(
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
            ],
          ),
        ),
      );

      final size = tester.getSize(
        find.byKey(const Key('board-canvas-absolute-grid')),
      );
      expect(size.width, 6 * UpegSizing.pinCellWidth + 5 * UpegSizing.pinGap);
      const initialRenderedRows = 2;
      expect(
        size.height,
        initialRenderedRows * boardCanvasPinCellHeight +
            (initialRenderedRows - 1) * UpegSizing.pinGap,
      );
    });

    testWidgets('BoardCanvas_passes_selected_tag_to_the_layout_query', (
      tester,
    ) async {
      late LayoutQuery observed;
      final container = ProviderContainer(
        overrides: [
          ...i18nTestOverrides,
          ...pegboardSelectionOverrides(
            tagOptions: (_) => const ['all', 'pure'],
          ),
          layoutLoaderProvider.overrideWithValue((query) {
            observed = query;
            return const LayoutSnapshotDto(
              boardKey: 'dev',
              boardCols: 6,
              placements: [],
            );
          }),
        ],
      );
      addTearDown(container.dispose);
      container
          .read(selectedTagProvider.notifier)
          .setSelection(const TagSpecific('pure'));

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: MaterialApp(
            home: Scaffold(
              body: BoardCanvas(
                boardKey: BoardKey.parse('dev'),
                onPinTap: (_) {},
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(observed.boardKey, BoardKey.parse('dev'));
      expect(observed.tag, const TagSpecific('pure'));
    });

    testWidgets('BoardCanvas_always_renders_at_six_column_width', (
      tester,
    ) async {
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            ...i18nTestOverrides,
            toolsLoaderProvider.overrideWith(
              (ref) =>
                  () => const <ToolDto>[],
            ),
          ],
          child: MaterialApp(
            home: Scaffold(
              body: SizedBox(
                width: 2000,
                height: 2000,
                child: debugBoardCanvasGrid(
                  snapshot: const LayoutSnapshotDto(
                    boardKey: 'dev',
                    boardCols: 6,
                    // Placements that span multiple rows, including a repaired
                    // placement at a lower row. Canvas width must still be
                    // exactly 6 columns.
                    placements: [
                      PlacementDto(
                        toolId: 'num.hex_to_decimal',
                        x: 0,
                        y: 0,
                        w: 1,
                        h: 1,
                      ),
                      PlacementDto(
                        toolId: 'convert.base64_decode',
                        x: 3,
                        y: 2,
                        w: 1,
                        h: 1,
                      ),
                    ],
                  ),
                  onPinTap: (_) {},
                ),
              ),
            ),
          ),
        ),
      );

      expect(find.byType(Pin), findsNWidgets(2));
      expect(find.text('NUM.HEX_TO_DECIMAL'), findsOneWidget);
      expect(find.text('CONVERT.BASE64_DECODE'), findsOneWidget);

      // Canvas width must be exactly 6 columns, NOT wider due to row extent.
      final canvasSize = tester.getSize(
        find.byKey(const Key('board-canvas-absolute-grid')),
      );
      final expectedWidth = 6 * UpegSizing.pinCellWidth + 5 * UpegSizing.pinGap;
      expect(canvasSize.width, expectedWidth);

      // Canvas height includes at least one empty trailing row beyond the
      // highest placement row (row 2 -> rowsForLayout >= 3 + 1 = 4).
      final expectedMinHeight =
          (2 + 1 + 1) * boardCanvasPinCellHeight + (2 + 1) * UpegSizing.pinGap;
      expect(canvasSize.height, greaterThanOrEqualTo(expectedMinHeight));
    });

    testWidgets(
      'clicking_a_TagChipRow_filters_the_visible_BoardCanvas_pins_and_all_restores_them',
      (tester) async {
        const allPlacements = [
          PlacementDto(toolId: 'num.hex_to_decimal', x: 0, y: 0, w: 1, h: 1),
          PlacementDto(toolId: 'text.lowercase', x: 1, y: 0, w: 1, h: 1),
        ];
        const purePlacements = [
          PlacementDto(toolId: 'num.hex_to_decimal', x: 0, y: 0, w: 1, h: 1),
        ];
        final observedTags = <TagSelection>[];
        final container = ProviderContainer(
          overrides: [
            ...i18nTestOverrides,
            ...pegboardSelectionOverrides(
              boardKey: 'dev',
              tagOptions: (_) => const ['all', 'pure'],
            ),
            boardTagOptionsLoaderProvider.overrideWithValue(
              (_) => const ['all', 'pure'],
            ),
            boardTagCountLoaderProvider.overrideWithValue((_, _) => 0),
            layoutLoaderProvider.overrideWithValue((query) {
              observedTags.add(query.tag);
              return LayoutSnapshotDto(
                boardKey: query.boardKey.value,
                boardCols: 6,
                placements: query.tag == const TagSpecific('pure')
                    ? purePlacements
                    : allPlacements,
              );
            }),
          ],
        );
        addTearDown(container.dispose);
        await container.read(pegboardSelectionProvider.notifier).restore(const [
          BoardDto(key: 'dev', title: 'Dev'),
        ]);

        await tester.pumpWidget(
          UncontrolledProviderScope(
            container: container,
            child: MaterialApp(
              theme: UpegTheme.darkTheme(),
              home: Scaffold(
                body: Column(
                  children: [
                    const TagChipRow(),
                    Expanded(
                      child: BoardCanvas(
                        boardKey: BoardKey.parse('dev'),
                        onPinTap: (_) {},
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();
        expect(find.byType(Pin), findsNWidgets(2));

        await tester.tap(find.byKey(const Key('tag-chip-pure')));
        await tester.pumpAndSettle();
        expect(find.byType(Pin), findsOneWidget);

        await tester.tap(find.byKey(const Key('tag-chip-all')));
        await tester.pumpAndSettle();
        expect(find.byType(Pin), findsNWidgets(2));
        expect(container.read(selectedTagProvider), const TagAll());
        expect(observedTags, contains(const TagSpecific('pure')));
        expect(observedTags, contains(const TagAll()));
      },
    );
  });
}
