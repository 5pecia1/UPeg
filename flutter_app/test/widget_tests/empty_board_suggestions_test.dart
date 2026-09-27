/// Widget tests for the [`EmptyBoard`] suggestion list.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/state/suggestions_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/empty_board.dart';

import '../test_helpers/tool_fixture.dart';

import '../test_helpers/i18n_test_catalog.dart';

final ToolDto _toolA = fixtureToolDto(
  id: 'num.hex_to_decimal',
  toolkit: 'convert',
  label: 'hex → dec',
  tags: const <String>['convert'],
);

final ToolDto _toolB = fixtureToolDto(
  id: 'id.uuid_v7',
  toolkit: 'id',
  label: 'uuid v7',
  tags: const <String>['id'],
);

Widget _harness({
  required String boardKey,
  required List<ToolDto> suggestions,
  required VoidCallback onOpenPalette,
  ProviderContainer? container,
}) {
  final c =
      container ??
      ProviderContainer(
        overrides: [
          ...i18nTestOverrides,
          suggestionsLoaderProvider.overrideWith(
            (ref) =>
                (_) => suggestions,
          ),
          isPinnedLoaderProvider.overrideWith(
            (ref) =>
                (_, _) => false,
          ),
          addPinMutatorProvider.overrideWith(
            (ref) =>
                (_, _) => 'pin',
          ),
        ],
      );
  return UncontrolledProviderScope(
    container: c,
    child: MaterialApp(
      theme: UpegTheme.darkTheme(),
      home: Scaffold(
        body: EmptyBoard(
          onOpenPalette: onOpenPalette,
          boardKey: BoardKey.parse(boardKey),
        ),
      ),
    ),
  );
}

void main() {
  group('EmptyBoard suggestions', () {
    testWidgets('EmptyBoard_renders_the_suggested_tools', (tester) async {
      await tester.pumpWidget(
        _harness(
          boardKey: 'dev',
          suggestions: [_toolA, _toolB],
          onOpenPalette: () {},
        ),
      );
      await tester.pumpAndSettle();

      expect(
        find.byKey(const Key('empty-board-suggestion-num.hex_to_decimal')),
        findsOneWidget,
      );
      expect(
        find.byKey(const Key('empty-board-suggestion-id.uuid_v7')),
        findsOneWidget,
      );
    });

    testWidgets('tapping_an_EmptyBoard_suggestion_calls_the_pinMutator', (
      tester,
    ) async {
      BoardKey? gotBoard;
      ToolId? gotTool;
      final container = ProviderContainer(
        overrides: [
          ...i18nTestOverrides,
          suggestionsLoaderProvider.overrideWith(
            (ref) =>
                (_) => [_toolA],
          ),
          isPinnedLoaderProvider.overrideWith(
            (ref) =>
                (_, _) => false,
          ),
          addPinMutatorProvider.overrideWith(
            (ref) => (b, t) {
              gotBoard = b;
              gotTool = t;
              return 'pin';
            },
          ),
        ],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(
        _harness(
          boardKey: 'dev',
          suggestions: [_toolA],
          onOpenPalette: () {},
          container: container,
        ),
      );
      await tester.pumpAndSettle();

      await tester.tap(
        find.byKey(const Key('empty-board-suggestion-num.hex_to_decimal')),
      );
      await tester.pumpAndSettle();

      expect(gotBoard, BoardKey.parse('dev'));
      expect(gotTool, ToolId.parse('num.hex_to_decimal'));
    });

    testWidgets('EmptyBoard_hides_suggestions_without_a_boardKey', (
      tester,
    ) async {
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            ...i18nTestOverrides,
            suggestionsLoaderProvider.overrideWith(
              (ref) =>
                  (_) => [_toolA],
            ),
          ],
          child: MaterialApp(
            theme: UpegTheme.darkTheme(),
            home: Scaffold(body: EmptyBoard(onOpenPalette: () {})),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(
        find.byKey(const Key('empty-board-suggestion-num.hex_to_decimal')),
        findsNothing,
      );
    });
  });
}
