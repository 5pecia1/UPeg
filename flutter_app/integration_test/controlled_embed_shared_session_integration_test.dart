/// Native WebView lifecycle smoke test. Run on a desktop target with its
/// platform WebView installed; Linux can use Xvfb for its display server.
library;

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:upeg/src/features/controlled_embed/session_service.dart';
import 'package:upeg/src/features/controlled_embed/webview_session.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/widgets/controlled_embed/session_host.dart';
import 'package:upeg/src/widgets/controlled_embed/settings.dart';

import '../test/shared/controlled_embed_browser_fixture.dart';

const _viewport = Size(640, 480);
const _compactDebugPane = Size(320, 240);
const _frameStep = Duration(milliseconds: 20);
const _completionTimeout = Duration(seconds: 45);
const _snapshotDecodeLimit = 2;
const _guiInputKey = Key('shared-session-gui-input');

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets(
    '숨겨진 실제 웹뷰를 디버거로 옮겨도 입력과 페이지 상태와 뷰포트가 유지된다',
    (tester) async {
      final fixture = await ControlledEmbedBrowserFixture.start();
      final service = ControlledEmbedSessionService(
        createSession: NativeControlledEmbedWebViewSession.create,
        normalizeResult: (_, result, _) => result,
      );
      final robot = _SharedSessionRobot(tester, service);
      try {
        await robot.mount();
        final entry = await robot.complete(
          service.ensure(
            ControlledEmbedSessionSpec(
              toolId: ToolId.parse('embed.shared_session_probe'),
              url: fixture.uri.toString(),
              settings: const ResolvedBrowserSettings(
                hasUserAgentOverride: false,
                viewportSize: _viewport,
              ),
            ),
          ),
        );
        robot.expectGuiInputFocused();
        await robot.writeInput(entry, '공유 입력');
        await robot.expectPageState(entry, '공유 입력');
        robot.expectGuiInputFocused();

        await robot.openDebugger(entry);
        await robot.expectPageState(entry, '공유 입력');
        await robot.resizeDebugger(_compactDebugPane);
        await robot.expectPageState(entry, '공유 입력');
        await robot.writeInput(entry, '디버그 입력');

        await robot.closeDebugger();
        await robot.focusGuiInput();
        await robot.focusPageInput(entry);
        await robot.expectPageState(entry, '디버그 입력');
        robot.expectGuiInputFocused();
        robot.expectSinglePageLoad(fixture);
      } finally {
        await robot.unmount();
        await service.close();
        await fixture.close();
      }
    },
    skip: !(Platform.isLinux || Platform.isMacOS || Platform.isWindows),
  );
}

final class _SharedSessionRobot {
  _SharedSessionRobot(this.tester, this.service);

  final WidgetTester tester;
  final ControlledEmbedSessionService service;
  final FocusNode _guiInputFocus = FocusNode();
  final ValueNotifier<Size> _debugPaneSize = ValueNotifier(_viewport);
  final ValueNotifier<ControlledEmbedSessionEntry?> _debugEntry = ValueNotifier(
    null,
  );
  VoidCallback? _releaseDebugger;

  Future<void> mount() => tester.pumpWidget(
    MaterialApp(
      builder: (context, child) => ControlledEmbedSessionHostView(
        sessions: service,
        child: child ?? const SizedBox.shrink(),
      ),
      home: Scaffold(
        body: Column(
          children: [
            TextField(
              key: _guiInputKey,
              focusNode: _guiInputFocus,
              autofocus: true,
            ),
            Expanded(
              child: ValueListenableBuilder<ControlledEmbedSessionEntry?>(
                valueListenable: _debugEntry,
                builder: (context, entry, _) => ValueListenableBuilder<Size>(
                  valueListenable: _debugPaneSize,
                  builder: (context, size, _) => Center(
                    child: SizedBox.fromSize(
                      size: size,
                      child: ClipRect(
                        child: SingleChildScrollView(
                          scrollDirection: Axis.horizontal,
                          child: SingleChildScrollView(
                            child:
                                entry?.browser.buildView() ??
                                const SizedBox.shrink(),
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    ),
  );

  Future<T> complete<T>(Future<T> operation) async {
    var completed = false;
    late T value;
    Object? failure;
    StackTrace? failureTrace;
    unawaited(
      operation.then<void>(
        (result) {
          value = result;
          completed = true;
        },
        onError: (Object error, StackTrace trace) {
          failure = error;
          failureTrace = trace;
          completed = true;
        },
      ),
    );
    final deadline = DateTime.now().add(_completionTimeout);
    while (!completed && DateTime.now().isBefore(deadline)) {
      await tester.pump(_frameStep);
    }
    if (!completed) {
      throw TimeoutException('Native WebView operation did not complete.');
    }
    if (failure case final error?) {
      Error.throwWithStackTrace(error, failureTrace!);
    }
    return value;
  }

  Future<void> writeInput(ControlledEmbedSessionEntry entry, String input) =>
      complete(
        entry.browser.runCommandJs('''
document.getElementById('input').value = ${jsonEncode(input)};
document.getElementById('trigger').click();
window.sessionMarker = 'shared';
'''),
      );

  Future<void> expectPageState(
    ControlledEmbedSessionEntry entry,
    String expectedInput,
  ) async {
    Object? snapshot = await complete(
      entry.browser.runResultJs('''
JSON.stringify({
  input: document.getElementById('input').value,
  output: document.getElementById('output').textContent,
  marker: window.sessionMarker,
  width: window.innerWidth,
  height: window.innerHeight
})
'''),
    );
    for (var step = 0; step < _snapshotDecodeLimit; step++) {
      if (snapshot is String) snapshot = jsonDecode(snapshot);
    }
    expect(snapshot, {
      'input': expectedInput,
      'output': expectedInput,
      'marker': 'shared',
      'width': _viewport.width.toInt(),
      'height': _viewport.height.toInt(),
    });
  }

  Future<void> openDebugger(ControlledEmbedSessionEntry entry) async {
    _releaseDebugger = service.reserveDebugger(entry);
    await tester.pump();
    _debugEntry.value = entry;
    await tester.pump();
  }

  Future<void> closeDebugger() async {
    _debugEntry.value = null;
    await tester.pump();
    _releaseDebugger?.call();
    _releaseDebugger = null;
    await tester.pump();
  }

  Future<void> resizeDebugger(Size size) async {
    _debugPaneSize.value = size;
    await tester.pump();
  }

  void expectSinglePageLoad(ControlledEmbedBrowserFixture fixture) {
    expect(fixture.pageLoads, 1);
  }

  void expectGuiInputFocused() {
    expect(_guiInputFocus.hasPrimaryFocus, isTrue);
  }

  Future<void> focusGuiInput() async {
    await tester.tap(find.byKey(_guiInputKey));
    await tester.pump();
  }

  Future<void> focusPageInput(ControlledEmbedSessionEntry entry) => complete(
    entry.browser.runCommandJs("document.getElementById('input').focus()"),
  );

  Future<void> unmount() async {
    await tester.pumpWidget(const SizedBox.shrink());
    _releaseDebugger?.call();
    _releaseDebugger = null;
    _debugEntry.dispose();
    _debugPaneSize.dispose();
    _guiInputFocus.dispose();
  }
}
