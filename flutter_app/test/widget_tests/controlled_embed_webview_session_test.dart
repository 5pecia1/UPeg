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

  testWidgets(
    'the_configured_browser_loads_the_page_once_the_viewport_mounts',
    (tester) async {
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
    },
  );

  testWidgets(
    'reattaching_the_view_for_the_debug_screen_keeps_the_same_page_and_controller',
    (tester) async {
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
    },
  );

  testWidgets('a_failed_page_load_releases_native_resources_once', (
    tester,
  ) async {
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

  testWidgets(
    'commands_issued_while_moving_to_the_debug_screen_wait_for_the_same_page_viewport_to_be_ready_again',
    (tester) async {
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
    },
  );

  testWidgets(
    'closing_while_the_view_is_detached_also_fails_pending_commands',
    (tester) async {
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
    },
  );

  testWidgets(
    'closing_the_session_before_mount_ends_the_ready_wait_and_releases_resources_once',
    (tester) async {
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
    },
  );

  testWidgets(
    'a_ready_timeout_because_the_view_never_mounted_cleans_up_the_session',
    (tester) async {
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
    },
  );
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
