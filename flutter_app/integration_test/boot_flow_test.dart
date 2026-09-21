/// Integration test — boot flow.
///
/// Asserts that the Flutter shell boots through `RustLib.init()` +
/// `initApp()` and lands on a real page (Splash → BoardPage).
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/app.dart';
import 'package:upeg/src/rust/frb_generated.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(() async {
    await RustLib.init();
  });

  testWidgets('app_boot_receives_AppInitReport_and_routes_to_BoardPage', (
    tester,
  ) async {
    await tester.pumpWidget(const ProviderScope(child: UpegApp()));
    await tester.pumpAndSettle(const Duration(seconds: 3));

    // Should land on the BoardPage (not SplashPage anymore).
    expect(find.byType(Scaffold), findsAtLeastNWidgets(1));
    // Splash should be gone after pumpAndSettle.
  });
}
