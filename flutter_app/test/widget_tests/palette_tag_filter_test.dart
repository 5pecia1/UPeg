/// Search starts across the catalogue even when the board has a tag filter.
/// The user can opt into that tag as a temporary palette search scope.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/i18n.dart' show LocaleDto;
import 'package:upeg/src/rust/api/palette.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart' show PinKindDto, ToolDto;
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/locale_provider.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/state/tag_provider.dart';
import 'package:upeg/src/widgets/palette_overlay.dart';

import '../test_helpers/tool_fixture.dart';
import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/ui_workflow_robot.dart';

const _hits = [
  PaletteHit(
    id: 'num.hex_to_decimal',
    label: 'hex → dec',
    description: '',
    score: 1.0,
    pinKind: PinKindDto.inline,
  ),
  PaletteHit(
    id: 'text.lowercase',
    label: 'lowercase',
    description: '',
    score: 0.9,
    pinKind: PinKindDto.inline,
  ),
  PaletteHit(
    id: 'hash.sha256',
    label: 'SHA-256',
    description: '',
    score: 0.8,
    pinKind: PinKindDto.inline,
  ),
];

Widget _harness({required TagSelection selectedTag}) {
  return ProviderScope(
    overrides: [
      paletteSearcherProvider.overrideWith((ref) {
        return (query) => [
          for (final hit in _hits)
            if (query.isEmpty || hit.id.contains(query)) hit,
        ];
      }),
      boardsLoaderProvider.overrideWith(
        (ref) =>
            () => const <BoardDto>[BoardDto(key: 'dev', title: 'Dev')],
      ),
      pinnedBoardsLoaderProvider.overrideWith(
        (ref) =>
            (_) => const <BoardKey>{},
      ),
      localeProvider.overrideWithValue(LocaleDto.en),
      ...i18nTestOverrides,
      tagOptionsLoaderProvider.overrideWith(
        (ref) =>
            () => const ['all', 'convert', 'text'],
      ),
      tagCountLoaderProvider.overrideWith(
        (ref) =>
            (_) => 0,
      ),
      selectedTagProvider.overrideWith(() {
        return _SeededTag(selectedTag);
      }),
      // The palette's tag filter intersects palette hits with the
      // `toolsForTagProvider(tag)` id set. Override the loader so
      // tests are deterministic without loading the dylib.
      toolsForTagLoaderProvider.overrideWith(
        (ref) =>
            (tag) => switch (tag) {
              'convert' => [
                fixtureToolDto(id: 'num.hex_to_decimal', toolkit: 'convert'),
              ],
              'text' => [fixtureToolDto(id: 'text.lowercase', toolkit: 'text')],
              _ => const <ToolDto>[],
            },
      ),
    ],
    child: MaterialApp(
      home: Scaffold(body: PaletteOverlay(onPick: (_, _) {})),
    ),
  );
}

class _SeededTag extends SelectedTagNotifier {
  _SeededTag(this._seed);
  final TagSelection _seed;
  @override
  TagSelection build() {
    super.build();
    return _seed;
  }
}

void main() {
  group('PaletteOverlay tag filter (G04)', () {
    testWidgets('should find SHA-256 globally from a convert filtered board', (
      tester,
    ) async {
      final robot = UiWorkflowRobot(tester);
      await robot.pumpScreen(
        _harness(selectedTag: const TagSpecific('convert')),
      );
      robot.expectCurrentTagSearchVisible('convert', selected: false);
      await robot.searchPalette('sha256');
      robot.expectPaletteToolVisible('hash.sha256');

      await robot.toggleCurrentTagSearch();
      robot.expectCurrentTagSearchVisible('convert', selected: true);
      robot.expectPaletteToolNotVisible('hash.sha256');

      await robot.toggleCurrentTagSearch();
      robot.expectCurrentTagSearchVisible('convert', selected: false);
      robot.expectPaletteToolVisible('hash.sha256');

      await robot.searchPalette('');
      robot.expectPaletteToolVisible('num.hex_to_decimal');
      robot.expectPaletteToolVisible('text.lowercase');
      robot.expectPaletteToolVisible('hash.sha256');
    });
    testWidgets('should omit scope chip when the board shows all tags', (
      tester,
    ) async {
      final robot = UiWorkflowRobot(tester);
      await robot.pumpScreen(_harness(selectedTag: const TagAll()));
      robot.expectCurrentTagSearchNotVisible();
      robot.expectPaletteToolVisible('num.hex_to_decimal');
      robot.expectPaletteToolVisible('text.lowercase');
      robot.expectPaletteToolVisible('hash.sha256');
    });
  });
}
