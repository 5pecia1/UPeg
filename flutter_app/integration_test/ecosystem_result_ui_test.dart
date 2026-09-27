/// Real loader/CLI result presentation at wide and narrow sizes.
/// Uses the isolated lab prepared for presentation_toolkit_test.dart.
library;

import 'dart:async';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:upeg/src/pages/expanded_modal_page.dart';
import 'package:upeg/src/rust/api/boot.dart' as boot;
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/frb_generated.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  final lab = Platform.environment['UPEG_PRESENTATION_LAB'];
  final enabled =
      Platform.isMacOS &&
      lab != null &&
      Platform.environment['UPEG_PROJECT_MANIFEST_PATH'] == 'off';
  testWidgets(
    'real no-change result retains summary, reason, and raw disclosure at both widths',
    (tester) async {
      await RustLib.init();
      try {
        var initialized = false;
        Object? failure;
        unawaited(
          boot.initApp().then(
            (_) {
              initialized = true;
            },
            onError: (Object error) {
              failure = error;
              initialized = true;
            },
          ),
        );
        final deadline = DateTime.now().add(const Duration(seconds: 90));
        while (!initialized && DateTime.now().isBefore(deadline)) {
          await tester.pump(const Duration(milliseconds: 50));
        }
        expect(failure, isNull);
        expect(initialized, isTrue);
        final tool = listTools(
          toolkit: 'ecosystem',
        ).singleWhere((tool) => tool.id == 'ecosystem.update_plan');
        for (final width in [1200.0, 600.0]) {
          await tester.binding.setSurfaceSize(Size(width, 900));
          final capture = GlobalKey();
          await tester.pumpWidget(
            ProviderScope(
              child: RepaintBoundary(
                key: capture,
                child: MaterialApp(
                  theme: UpegTheme.darkTheme(),
                  home: ExpandedModalPage(
                    key: ValueKey(width),
                    tool: tool,
                    initialInput: ToolArgs.fromJsonObject({
                      'project': '$lab/project-empty',
                    }),
                  ),
                ),
              ),
            ),
          );
          await tester.pump();
          await tester.tap(find.byKey(const Key('expanded-modal-run-btn')));
          final result = find.byKey(const Key('rich-presentation'));
          final until = DateTime.now().add(const Duration(seconds: 90));
          while (result.evaluate().isEmpty && DateTime.now().isBefore(until)) {
            await tester.pump(const Duration(milliseconds: 50));
          }
          expect(result, findsOneWidget);
          expect(find.text('변경 없음'), findsWidgets);
          final apply = find.byKey(
            const Key('presentation-result-action-apply'),
          );
          await tester.ensureVisible(apply);
          await tester.pump();
          expect(tester.widget<OutlinedButton>(apply).onPressed, isNull);
          expect(find.text('파일, 실행, 등록에 적용할 변경이 없습니다.'), findsOneWidget);
          expect(
            find.byKey(const Key('rich-presentation-raw')),
            findsOneWidget,
          );
          expect(find.textContaining('engine_fingerprint'), findsNothing);
          expect(tester.takeException(), isNull);
          await tester.pump(const Duration(milliseconds: 200));
          final boundary =
              capture.currentContext!.findRenderObject()!
                  as RenderRepaintBoundary;
          final rendered = await boundary.toImage();
          final bytes = await rendered.toByteData(
            format: ui.ImageByteFormat.png,
          );
          await File(
            '$lab/result-${width.toInt()}.png',
          ).writeAsBytes(bytes!.buffer.asUint8List());
          rendered.dispose();
        }
      } finally {
        boot.shutdown();
        await tester.binding.setSurfaceSize(null);
      }
    },
    skip: !enabled,
    timeout: const Timeout(Duration(minutes: 5)),
  );
}
