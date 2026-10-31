import 'package:flutter/material.dart';
import 'package:flutter/services.dart' show LogicalKeyboardKey;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/pin.dart';

import 'i18n_test_catalog.dart';
import 'pin_handle_test_overrides.dart';

class PinHeaderOpenRobot {
  PinHeaderOpenRobot(this.tester);

  final WidgetTester tester;
  int openCount = 0;
  int runCount = 0;

  static const placement = PlacementDto(
    toolId: 'convert.base64_encode',
    pinId: 'base64',
    x: 0,
    y: 0,
    w: 1,
    h: 1,
  );

  Future<void> pumpPin() async {
    await tester.pumpWidget(
      ProviderScope(
        overrides: [...i18nTestOverrides, ...pinHandleTestOverrides],
        child: MaterialApp(
          theme: UpegTheme.darkTheme(),
          home: Scaffold(
            body: SizedBox(
              width: 168,
              height: 180,
              child: Pin(
                placement: placement,
                toolLabel: 'Base64 encode',
                toolkit: 'convert',
                showMoveHandle: true,
                onTap: (_) => runCount += 1,
                onOpenModal: (_) => openCount += 1,
                exposeBodySemantics: true,
                bodyOverride: Column(
                  children: [
                    const TextField(
                      decoration: InputDecoration(labelText: 'input value'),
                    ),
                    FilledButton(onPressed: () {}, child: const Text('Run')),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
  }

  void expectOpenTitleVisible() {
    expect(find.byKey(pinHeaderOpenKey), findsOneWidget);
    expect(find.byTooltip('open · ${placement.toolId}'), findsOneWidget);
    final title = tester.widget<Text>(find.text('Base64 encode'));
    expect(title.style?.decoration, TextDecoration.underline);
  }

  Future<void> tapOpenTitle() async {
    await tester.tap(find.byKey(pinHeaderOpenKey));
    await tester.pump();
  }

  Future<void> pressOpenTitle() async {
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
  }

  void expectOpenedWithoutRun() {
    expect(openCount, 1);
    expect(runCount, 0);
  }
}
