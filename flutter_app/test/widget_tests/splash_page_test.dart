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
    testWidgets('스플래시는_upeg_워드마크와_로딩_인디케이터를_렌더한다', (tester) async {
      await tester.pumpWidget(const MaterialApp(home: SplashPage()));
      await tester.pump();

      expect(find.byKey(splashWordmarkKey), findsOneWidget);
      expect(find.text(splashWordmark), findsOneWidget);
      expect(find.byType(CircularProgressIndicator), findsOneWidget);
    });

    testWidgets('스플래시는_개발자용_문구를_노출하지_않는다', (tester) async {
      await tester.pumpWidget(const MaterialApp(home: SplashPage()));
      await tester.pump();

      expect(find.textContaining('skeleton'), findsNothing);
      expect(find.textContaining('pre-boot'), findsNothing);
    });
  });
}
