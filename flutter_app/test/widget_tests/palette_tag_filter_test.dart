/// G04 — Palette honours the currently selected tag filter.
///
/// The palette restricts results to the active tag's tool set.
/// The Flutter [`PaletteOverlay`] originally did NOT apply the
/// `selectedTagProvider` value (inventory row G04 — ⚠️ partial).
///
/// This test pins the contract: when `selectedTagProvider != kAllTag`,
/// the palette only renders hits whose `id` is in the tag's tool set.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/i18n/t.dart';
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

String _fakeT(String key, LocaleDto locale) => key;
String _fakeTArgs(
  String key,
  LocaleDto locale,
  List<String> argKeys,
  List<String> argVals,
) => key;

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
];

Widget _harness({required TagSelection selectedTag}) {
  return ProviderScope(
    overrides: [
      paletteSearcherProvider.overrideWith((ref) {
        return (_) => _hits;
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
      i18nTranslateOverride.overrideWithValue(_fakeT),
      i18nTranslateArgsOverride.overrideWithValue(_fakeTArgs),
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
      home: Scaffold(body: PaletteOverlay(onPick: (_) {})),
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
    testWidgets(
      'paletteoverlay_shows_only_convert_tools_when_the_selected_tag_is_convert',
      (tester) async {
        await tester.pumpWidget(
          _harness(selectedTag: const TagSpecific('convert')),
        );
        await tester.pumpAndSettle();

        expect(
          find.byKey(const ValueKey('palette-hit-num.hex_to_decimal')),
          findsOneWidget,
        );
        expect(
          find.byKey(const ValueKey('palette-hit-text.lowercase')),
          findsNothing,
        );
      },
    );

    testWidgets('paletteoverlay_shows_every_hit_when_the_selected_tag_is_all', (
      tester,
    ) async {
      await tester.pumpWidget(_harness(selectedTag: const TagAll()));
      await tester.pumpAndSettle();

      expect(
        find.byKey(const ValueKey('palette-hit-num.hex_to_decimal')),
        findsOneWidget,
      );
      expect(
        find.byKey(const ValueKey('palette-hit-text.lowercase')),
        findsOneWidget,
      );
    });
  });
}
