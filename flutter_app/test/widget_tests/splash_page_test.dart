/// Widget tests for [SplashPage].
///
/// Tiny surface — we guard the user-facing wordmark + the progress
/// indicator, and pin that no developer jargon leaks into the pre-boot
/// frame.
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/pages/splash_page.dart';

void main() {
  group('SplashPage', () {
    testWidgets('splash_renders_the_upeg_wordmark_and_loading_indicator', (
      tester,
    ) async {
      await tester.pumpWidget(const MaterialApp(home: SplashPage()));
      await tester.pump();

      expect(find.byKey(splashWordmarkKey), findsOneWidget);
      expect(find.text(splashWordmark), findsOneWidget);
      expect(find.byType(CircularProgressIndicator), findsOneWidget);
    });

    testWidgets('splash_does_not_expose_developer_wording', (tester) async {
      await tester.pumpWidget(const MaterialApp(home: SplashPage()));
      await tester.pump();

      expect(find.textContaining('skeleton'), findsNothing);
      expect(find.textContaining('pre-boot'), findsNothing);
    });
  });
}
