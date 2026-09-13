/// Integration test — palette search (Cmd-K shortcut).
///
/// When the user types Cmd-K (or Ctrl-K on non-mac), the search palette
/// appears with a focused TextField. The actual filtering logic is
/// covered by widget tests; this test guards the route from key event
/// → overlay mount.
library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
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

  testWidgets('Cmd_K_단축키는_PaletteOverlay를_연다', (tester) async {
    await tester.pumpWidget(const ProviderScope(child: UpegApp()));
    await tester.pumpAndSettle(const Duration(seconds: 3));

    await tester.sendKeyDownEvent(LogicalKeyboardKey.metaLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyK);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.metaLeft);
    await tester.pumpAndSettle();

    // PaletteOverlay should appear (look for its search TextField).
    expect(find.byType(TextField), findsAtLeastNWidgets(1));
  });
}
