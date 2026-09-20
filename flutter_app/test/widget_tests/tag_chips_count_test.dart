/// G05 — Tag count displayed per chip.
///
/// Each tag chip renders with its tool count appended
/// ("convert 4"). Flutter [`TagChipRow`] keeps that contract
/// through the overridable tag-count provider.
///
/// This test pins the contract: each chip displays the tag label
/// followed by the count returned from the overridable
/// [`countForTagProvider`] family.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/tag_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/tag_chips.dart';

import '../test_helpers/pegboard_selection_overrides.dart';

ProviderContainer _container({
  required List<String> tags,
  required Map<String, int> counts,
  String boardKey = 'dev',
}) {
  return ProviderContainer(
    overrides: [
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
      boardTagCountLoaderProvider.overrideWith(
        (ref) => (board, sel) {
          expect(board, BoardKey.parse(boardKey));
          return counts[sel.frbValue] ?? 0;
        },
      ),
    ],
  );
}

class _SeededCurrentBoardNotifier extends CurrentBoardNotifier {
  _SeededCurrentBoardNotifier(this._seed);

  final String _seed;

  @override
  BoardKey? build() => BoardKey.parse(_seed);
}

Widget _harness({
  required List<String> tags,
  required Map<String, int> counts,
  ProviderContainer? container,
}) {
  final c = container ?? _container(tags: tags, counts: counts);
  addTearDown(c.dispose);
  return UncontrolledProviderScope(
    container: c,
    child: MaterialApp(
      theme: UpegTheme.darkTheme(),
      home: const Scaffold(body: TagChipRow()),
    ),
  );
}

void main() {
  group('TagChipRow tag counts (G05)', () {
    testWidgets('TagChipRow_shows_the_count_for_each_tag', (tester) async {
      await tester.pumpWidget(
        _harness(
          tags: const ['all', 'convert', 'id'],
          counts: const {'all': 7, 'convert': 3, 'id': 2},
        ),
      );
      await tester.pumpAndSettle();

      // Chip labels use the `'<tag> <n>'` format.
      expect(find.text('all 7'), findsOneWidget);
      expect(find.text('convert 3'), findsOneWidget);
      expect(find.text('id 2'), findsOneWidget);
    });

    testWidgets('TagChipRow_still_shows_tags_with_a_zero_count', (
      tester,
    ) async {
      await tester.pumpWidget(
        _harness(
          tags: const ['all', 'empty'],
          counts: const {'all': 1, 'empty': 0},
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text('empty 0'), findsOneWidget);
    });
  });
}
