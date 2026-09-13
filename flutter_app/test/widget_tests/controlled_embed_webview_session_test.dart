import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/features/controlled_embed/webview_session.dart';
import 'package:upeg/src/widgets/controlled_embed/settings.dart';
import 'package:webview_all/webview_all.dart';

import '../shared/fake_webview_platform.dart';

const _pageUrl = 'https://example.test/controlled-embed';
const _customUserAgent = 'UPeg session test';
const _customViewport = Size(640, 480);

void main() {
  late FakeWebViewPlatform platform;
  setUp(() {
    platform = FakeWebViewPlatform();
    WebViewPlatform.instance = platform;
  });

  testWidgets('뷰포트가 마운트된 뒤 설정한 브라우저에서 페이지를 로드한다', (tester) async {
    final session = await NativeControlledEmbedWebViewSession.create(
      url: _pageUrl,
      settings: const ResolvedBrowserSettings(
        hasUserAgentOverride: true,
        userAgent: _customUserAgent,
        viewportSize: _customViewport,
      ),
    );
    addTearDown(session.close);
    final controller = platform.controllers.single;
    expect(controller.navigationCount, 0);

    final robot = _SessionRobot(tester);
    await robot.mount(session);
    await session.ready;

    robot.expectViewport(_customViewport);
    expect(controller.javaScriptMode, JavaScriptMode.unrestricted);
    expect(controller.userAgent, _customUserAgent);
    expect(controller.navigationCount, 1);
    await session.runCommandJs('window.sessionValue = 1');
    expect(await session.runResultJs('window.sessionValue'), '테스트 결과');
    expect(controller.commands, [
      'window.sessionValue = 1',
      'window.sessionValue',
    ]);
  });

  testWidgets('디버그 화면으로 뷰를 다시 붙여도 같은 페이지와 컨트롤러를 유지한다', (tester) async {
    final session = await NativeControlledEmbedWebViewSession.create(
      url: _pageUrl,
    );
    addTearDown(session.close);
    final robot = _SessionRobot(tester);
    await robot.mount(session);
    await session.ready;

    await robot.unmount();
    await robot.mount(session);

    expect(platform.controllers, hasLength(1));
    expect(platform.controllers.single.navigationCount, 1);
    expect(platform.controllers.single.closeCount, 0);
  });

  testWidgets('페이지 로드가 실패하면 네이티브 자원을 한 번 해제한다', (tester) async {
    platform.failLoading = true;
    final session = await NativeControlledEmbedWebViewSession.create(
      url: _pageUrl,
    );
    final readyFailure = expectLater(session.ready, throwsStateError);

    await _SessionRobot(tester).mount(session);
    await readyFailure;
    await session.close();

    expect(platform.controllers.single.closeCount, 1);
    await expectLater(session.runResultJs('document.title'), throwsStateError);
  });

  testWidgets('디버그 화면으로 옮기는 동안 명령은 같은 페이지의 뷰포트가 다시 준비될 때까지 기다린다', (
    tester,
  ) async {
    final session = await NativeControlledEmbedWebViewSession.create(
      url: _pageUrl,
    );
    addTearDown(session.close);
    final robot = _SessionRobot(tester);
    await robot.mount(session);
    await session.ready;
    await robot.unmount();
    var completed = false;
    final command = session.runCommandJs('window.shared = true').then((_) {
      completed = true;
    });
    await tester.pump();
    expect(completed, isFalse);
    expect(platform.controllers.single.commands, isEmpty);

    await robot.mount(session);
    await command;

    expect(completed, isTrue);
    expect(platform.controllers.single.commands, ['window.shared = true']);
    expect(platform.controllers.single.navigationCount, 1);
  });

  testWidgets('뷰가 분리된 상태에서 닫으면 대기 중인 명령도 실패로 종료된다', (tester) async {
    final session = await NativeControlledEmbedWebViewSession.create(
      url: _pageUrl,
    );
    final robot = _SessionRobot(tester);
    await robot.mount(session);
    await session.ready;
    await robot.unmount();
    final commandFailure = expectLater(
      session.runCommandJs('window.shared = true'),
      throwsStateError,
    );

    await session.close();
    await commandFailure;

    expect(platform.controllers.single.closeCount, 1);
    expect(platform.controllers.single.commands, isEmpty);
  });

  testWidgets('마운트 전에 세션을 닫으면 준비 대기를 종료하고 자원을 한 번 해제한다', (tester) async {
    final session = await NativeControlledEmbedWebViewSession.create(
      url: _pageUrl,
    );
    final readyFailure = expectLater(session.ready, throwsStateError);

    await session.close();
    await session.close();
    await readyFailure;

    expect(platform.controllers.single.closeCount, 1);
    expect(platform.controllers.single.navigationCount, 0);
    expect(session.buildView, throwsStateError);
  });

  testWidgets('화면이 마운트되지 않아 준비 시간이 초과되면 세션을 정리한다', (tester) async {
    const loadTimeout = Duration(milliseconds: 10);
    final session = await NativeControlledEmbedWebViewSession.create(
      url: _pageUrl,
      loadTimeout: loadTimeout,
    );
    final readyFailure = expectLater(
      session.ready,
      throwsA(isA<TimeoutException>()),
    );

    await tester.pump(loadTimeout);
    await readyFailure;

    expect(platform.controllers.single.closeCount, 1);
  });
}

class _SessionRobot {
  _SessionRobot(this.tester);
  final WidgetTester tester;

  Future<void> mount(ControlledEmbedWebViewSession session) =>
      tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: Center(child: session.buildView()),
        ),
      );

  Future<void> unmount() => tester.pumpWidget(const SizedBox.shrink());

  void expectViewport(Size size) {
    expect(tester.getSize(find.byType(WebViewWidget)), size);
  }
}
