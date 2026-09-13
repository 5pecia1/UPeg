/// Widget tests for [EmptyBoard].
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/widgets/empty_board.dart';

import '../test_helpers/i18n_test_catalog.dart';

Widget _harness(VoidCallback onOpenPalette) {
  return ProviderScope(
    overrides: [...i18nTestOverrides],
    child: MaterialApp(
      home: Scaffold(body: EmptyBoard(onOpenPalette: onOpenPalette)),
    ),
  );
}

void main() {
  group('EmptyBoard', () {
    testWidgets('EmptyBoard_는_안내_텍스트와_CTA_버튼을_표시한다', (tester) async {
      await tester.pumpWidget(_harness(() {}));

      expect(find.byKey(const Key('empty-board-card')), findsOneWidget);
      expect(find.text(i18nEn(emptyBoardHintTextKey)), findsOneWidget);
      expect(find.byKey(const Key('empty-board-cta')), findsOneWidget);
      expect(find.text(i18nEn(emptyBoardCtaLabelKey)), findsOneWidget);
    });

    testWidgets('EmptyBoard_의_CTA는_콜백을_호출한다', (tester) async {
      int taps = 0;
      await tester.pumpWidget(_harness(() => taps += 1));

      await tester.tap(find.byKey(const Key('empty-board-cta')));
      await tester.pumpAndSettle();

      expect(taps, 1);
    });
  });
}
