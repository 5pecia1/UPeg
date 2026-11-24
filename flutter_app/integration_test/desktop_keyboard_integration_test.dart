/// Boots the desktop entry point and exercises the production keyboard wiring.
///
/// Set UPEG_HOME to a fresh temporary directory, then run from flutter_app:
///   UPEG_PROJECT_MANIFEST_PATH=off flutter test
///     integration_test/desktop_keyboard_integration_test.dart -d linux
/// On a headless Linux host, run the command under xvfb-run -a.
library;

import 'dart:io';
import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:upeg/main.dart' as app;
import 'package:upeg/src/pages/board_page.dart';
import 'package:upeg/src/widgets/palette_overlay.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets(
    'should restore board focus after keyboard-only settings and palette use',
    (tester) async {
      final robot = _DesktopKeyboardRobot(tester);
      await robot.boot();

      await robot.openSettings();
      robot.expectSettings(true);
      await robot.press(LogicalKeyboardKey.escape);
      robot.expectSettings(false);
      robot.expectBoardFocus();

      await robot.openPaletteFromToolbar();
      await robot.typePaletteQuery('hex');
      robot.expectPaletteQuery('hex');
      await robot.press(LogicalKeyboardKey.escape);
      robot.expectPaletteClosedAndToolbarFocused();
    },
    skip: _environmentProblem() != null,
  );
}

String? _environmentProblem() {
  if (!(Platform.isLinux || Platform.isMacOS || Platform.isWindows)) {
    return 'Requires a desktop Flutter runner.';
  }
  if ((Platform.environment['UPEG_HOME'] ?? '').isEmpty ||
      Platform.environment['UPEG_PROJECT_MANIFEST_PATH'] != 'off') {
    return 'Set UPEG_HOME to a fresh temporary directory and '
        'UPEG_PROJECT_MANIFEST_PATH=off.';
  }
  return null;
}

class _DesktopKeyboardRobot {
  _DesktopKeyboardRobot(this.tester);

  static const _paletteButton = Key('open-palette-btn');
  static const _paletteQuery = Key('palette-query-field');
  static const _settingsClose = Key('settings-close-btn');
  final WidgetTester tester;

  Future<void> boot() async {
    final priorFrameworkHandler = FlutterError.onError;
    final dispatcher = PlatformDispatcher.instance;
    final priorPlatformHandler = dispatcher.onError;

    tester.testTextInput.register();
    addTearDown(tester.testTextInput.unregister);

    final launch = app.main();
    await tester.pump();
    try {
      await launch;
    } finally {
      // Keep forwarding framework errors through the test binding.
      FlutterError.onError = priorFrameworkHandler;
      dispatcher.onError = priorPlatformHandler;
    }
    await tester.pumpAndSettle();
    expect(find.byType(BoardPage), findsOneWidget);
    expectBoardFocus();
  }

  Future<void> openSettings() async {
    final primary = Platform.isMacOS
        ? LogicalKeyboardKey.metaLeft
        : LogicalKeyboardKey.controlLeft;
    await tester.sendKeyDownEvent(primary);
    await tester.sendKeyEvent(LogicalKeyboardKey.comma);
    await tester.sendKeyUpEvent(primary);
    await tester.pumpAndSettle();
  }

  Future<void> openPaletteFromToolbar() async {
    for (var i = 0; i < 20 && !_toolbarButtonFocused; i++) {
      await press(LogicalKeyboardKey.tab);
    }
    expect(_toolbarButtonFocused, isTrue);
    await press(LogicalKeyboardKey.enter);
    expect(find.byType(PaletteOverlay), findsOneWidget);
  }

  Future<void> typePaletteQuery(String query) async {
    final input = tester.widget<EditableText>(
      find.descendant(
        of: find.byKey(_paletteQuery),
        matching: find.byType(EditableText),
      ),
    );
    expect(input.focusNode.hasPrimaryFocus, isTrue);
    tester.testTextInput.enterText(query);
    await tester.pumpAndSettle();
  }

  Future<void> press(LogicalKeyboardKey key) async {
    await tester.sendKeyEvent(key);
    await tester.pumpAndSettle();
  }

  void expectSettings(bool expected) {
    expect(
      find.byKey(_settingsClose),
      expected ? findsOneWidget : findsNothing,
    );
  }

  void expectBoardFocus() {
    expect(FocusManager.instance.primaryFocus?.debugLabel, 'Board grid');
  }

  void expectPaletteQuery(String query) {
    final field = tester.widget<TextField>(find.byKey(_paletteQuery));
    expect(field.controller!.text, query);
  }

  void expectPaletteClosedAndToolbarFocused() {
    expect(find.byType(PaletteOverlay), findsNothing);
    expect(_toolbarButtonFocused, isTrue);
  }

  bool get _toolbarButtonFocused {
    final focus = FocusManager.instance.primaryFocus?.context;
    return find
        .ancestor(
          of: find.byElementPredicate((element) => element == focus),
          matching: find.byKey(_paletteButton),
        )
        .evaluate()
        .isNotEmpty;
  }
}
