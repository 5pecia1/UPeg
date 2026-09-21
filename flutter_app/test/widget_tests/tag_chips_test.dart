/// Widget tests for [TagChipRow].
library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/tag_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/tag_chips.dart';

import '../test_helpers/fake_keyboard_resolver.dart';
import '../test_helpers/pegboard_selection_overrides.dart';

import '../test_helpers/i18n_test_catalog.dart';

class _SeededCurrentBoardNotifier extends CurrentBoardNotifier {
  _SeededCurrentBoardNotifier(this._seed);

  final String _seed;

  @override
  BoardKey? build() => BoardKey.parse(_seed);
}

Widget _harness({
  required List<String> tags,
  ProviderContainer? container,
  String boardKey = 'dev',
}) {
  return UncontrolledProviderScope(
    container: container ?? _container(tags, boardKey: boardKey),
    child: MaterialApp(
      theme: UpegTheme.darkTheme(),
      home: const Scaffold(body: TagChipRow()),
    ),
  );
}

ProviderContainer _container(List<String> tags, {String boardKey = 'dev'}) {
  return ProviderContainer(
    overrides: [
      ...i18nTestOverrides,
      currentBoardKeyProvider.overrideWith(
        () => _SeededCurrentBoardNotifier(boardKey),
      ),
      ...pegboardSelectionOverrides(tagOptions: (_) => tags),
      boardTagOptionsLoaderProvider.overrideWith(
        (ref) => (board) {
          expect(board, BoardKey.parse(boardKey));
          return tags;
        },
      ),
      // Batch Q3: tag chips now read counts from the loader; tests
      // pin a deterministic 0 so the dylib never has to load.
      boardTagCountLoaderProvider.overrideWith(
        (ref) =>
            (_, _) => 0,
      ),
      fakeKeyboardResolverOverride,
    ],
  );
}

void main() {
  group('TagChipRow', () {
    testWidgets('TagChipRow_renders_each_tag_option', (tester) async {
      await tester.pumpWidget(_harness(tags: const ['all', 'convert', 'id']));
      await tester.pumpAndSettle();

      expect(find.byKey(const Key('tag-chip-all')), findsOneWidget);
      expect(find.byKey(const Key('tag-chip-convert')), findsOneWidget);
      expect(find.byKey(const Key('tag-chip-id')), findsOneWidget);
      expect(find.text('TAGS'), findsOneWidget);
    });

    testWidgets('TagChipRow_shows_only_the_tags_pinned_to_the_current_board', (
      tester,
    ) async {
      await tester.pumpWidget(_harness(tags: const ['all', 'convert']));
      await tester.pumpAndSettle();

      expect(find.byKey(const Key('tag-chip-all')), findsOneWidget);
      expect(find.byKey(const Key('tag-chip-convert')), findsOneWidget);
      expect(find.byKey(const Key('tag-chip-text')), findsNothing);
    });

    testWidgets('tapping_a_TagChipRow_chip_updates_selectedTagProvider', (
      tester,
    ) async {
      final container = _container(const ['all', 'convert']);
      addTearDown(container.dispose);

      await tester.pumpWidget(
        _harness(tags: const ['all', 'convert'], container: container),
      );
      await tester.pumpAndSettle();

      expect(container.read(selectedTagProvider), const TagAll());

      await tester.tap(find.byKey(const Key('tag-chip-convert')));
      await tester.pumpAndSettle();

      expect(container.read(selectedTagProvider), const TagSpecific('convert'));
    });

    testWidgets(
      'the_selected_chip_exposes_semantics_selected_and_the_check_icon_together',
      (tester) async {
        final semantics = tester.ensureSemantics();
        try {
          await tester.pumpWidget(_harness(tags: const ['all', 'convert']));
          await tester.pumpAndSettle();

          // 'all' is selected by default — the check icon is drawn only
          // inside the selected chip (the non-color cue: selection is not
          // conveyed by color alone).
          expect(find.byKey(tagChipCheckIconKey), findsOneWidget);
          expect(
            find.descendant(
              of: find.byKey(const Key('tag-chip-all')),
              matching: find.byKey(tagChipCheckIconKey),
            ),
            findsOneWidget,
          );

          expect(
            tester.getSemantics(find.byKey(const Key('tag-chip-all'))),
            isSemantics(isButton: true, isSelected: true),
          );
          expect(
            tester.getSemantics(find.byKey(const Key('tag-chip-convert'))),
            isSemantics(isButton: true, isSelected: false),
          );

          // Moving the selection moves the check icon and the selected
          // flag together.
          await tester.tap(find.byKey(const Key('tag-chip-convert')));
          await tester.pumpAndSettle();

          expect(
            find.descendant(
              of: find.byKey(const Key('tag-chip-convert')),
              matching: find.byKey(tagChipCheckIconKey),
            ),
            findsOneWidget,
          );
          expect(
            tester.getSemantics(find.byKey(const Key('tag-chip-convert'))),
            isSemantics(isButton: true, isSelected: true),
          );
        } finally {
          semantics.dispose();
        }
      },
    );

    testWidgets('TagChipRow_moves_the_selected_tag_with_the_keyboard', (
      tester,
    ) async {
      final container = _container(const ['all', 'convert', 'id']);
      addTearDown(container.dispose);

      await tester.pumpWidget(
        _harness(tags: const ['all', 'convert', 'id'], container: container),
      );
      await tester.pumpAndSettle();

      Focus.of(
        tester.element(find.byKey(const Key('tag-chip-row'))),
      ).requestFocus();
      await tester.pump();

      await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
      await tester.pump();
      expect(container.read(selectedTagProvider), const TagSpecific('convert'));

      await tester.sendKeyEvent(LogicalKeyboardKey.end);
      await tester.pump();
      expect(container.read(selectedTagProvider), const TagSpecific('id'));

      await tester.sendKeyEvent(LogicalKeyboardKey.home);
      await tester.pump();
      expect(container.read(selectedTagProvider), const TagAll());
    });
  });
}
