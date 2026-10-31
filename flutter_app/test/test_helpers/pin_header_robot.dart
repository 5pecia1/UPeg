import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/pin/pin_header.dart';
import 'package:upeg/src/widgets/pin_renderers/registry.dart';

import 'pin_handle_test_overrides.dart';

class PinHeaderRobot {
  PinHeaderRobot(this.tester);
  final WidgetTester tester;

  Future<void> pumpHeader({
    required double width,
    required String label,
    required String id,
    bool light = false,
    bool showMoveHandle = false,
    VoidCallback? onOpen,
  }) async {
    await tester.pumpWidget(
      ProviderScope(
        overrides: pinHandleTestOverrides,
        child: MaterialApp(
          theme: light ? UpegTheme.lightTheme() : UpegTheme.darkTheme(),
          home: Scaffold(
            body: Center(
              child: SizedBox(
                width: width,
                child: Builder(
                  builder: (context) => PinHeader(
                    tokens: context.upeg,
                    accentColor: context.upeg.accent,
                    chrome: const PinKindChrome(
                      icon: Icons.code,
                      variant: PinChromeVariant.output,
                    ),
                    toolId: id,
                    toolLabel: label,
                    toolkit: 'convert',
                    showMoveHandle: showMoveHandle,
                    onOpen: onOpen,
                    openLabel: onOpen == null ? null : 'open',
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
  }

  void expectTitleFits(String label) {
    final paragraph = tester.renderObject<RenderParagraph>(find.text(label));
    expect(paragraph.didExceedMaxLines, isFalse);
    expect(tester.takeException(), isNull);
  }

  void expectToolkitNotVisible(String id) {
    expect(find.text('CONVERT'), findsNothing);
    expect(find.byTooltip('$id · CONVERT'), findsOneWidget);
  }

  void expectToolkitVisible(String id) {
    expect(find.text('CONVERT'), findsOneWidget);
    expect(find.byTooltip(id), findsOneWidget);
  }
}
