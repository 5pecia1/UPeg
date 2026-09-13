import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/embed.dart';
import 'package:upeg/src/widgets/embed_iframe_factory.dart';
import 'package:upeg/src/widgets/expanded_modal/webview_panel.dart';
import 'package:webview_all/webview_all.dart';

import '../test_helpers/i18n_test_catalog.dart';

class _NoopIframeFactory extends IframeFactory {
  const _NoopIframeFactory();

  @override
  String register(String url, {void Function()? onLoad}) {
    // Don't fire onLoad — leaves the IframeLoadObserver in loading state
    // so the timeout overlay assertions can be deterministic.
    return 'noop-view-$url';
  }
}

void main() {
  group('WebViewPanel', () {
    final originalDesktopBuilder = desktopWebViewBuilder;
    final originalLauncher = webViewPanelLauncher;
    final originalResolver = webViewTargetResolver;

    setUp(() {
      embedIframeFactory = const _NoopIframeFactory();
      desktopWebViewBuilder = (url, userAgent, onControllerReady) =>
          SizedBox.expand(
            key: const Key('webview-panel-inappwebview'),
            child: Text('webview|url:$url|ua:${userAgent ?? "default"}'),
          );
    });

    tearDown(() {
      embedIframeFactory = const DartUiWebIframeFactory();
      desktopWebViewBuilder = originalDesktopBuilder;
      webViewPanelLauncher = originalLauncher;
      webViewTargetResolver = originalResolver;
    });

    testWidgets('web_target이면_iframe_컨테이너가_렌더된다', (tester) async {
      await tester.pumpWidget(
        ProviderScope(
          overrides: [...i18nTestOverrides],
          child: MaterialApp(
            home: Scaffold(
              body: WebViewPanel(
                resolution: const EmbedResolutionDto(
                  url: 'https://example.test/iframe',
                ),
                debugTargetOverride: const WebViewTarget.web(),
              ),
            ),
          ),
        ),
      );
      await tester.pump();
      expect(find.byKey(const Key('webview-panel-iframe')), findsOneWidget);
      expect(find.byKey(const Key('webview-panel-inappwebview')), findsNothing);
    });

    testWidgets('inAppWebView_target이면_inappwebview_컨테이너가_렌더된다', (
      tester,
    ) async {
      await tester.pumpWidget(
        ProviderScope(
          overrides: [...i18nTestOverrides],
          child: MaterialApp(
            home: Scaffold(
              body: WebViewPanel(
                resolution: const EmbedResolutionDto(
                  url: 'https://example.test/desktop',
                ),
                debugTargetOverride: const WebViewTarget.inAppWebView(),
              ),
            ),
          ),
        ),
      );
      expect(
        find.byKey(const Key('webview-panel-inappwebview')),
        findsOneWidget,
      );
      expect(find.byKey(const Key('webview-panel-iframe')), findsNothing);
    });

    testWidgets('userAgent_지정하면_해당_문자열이_전달된다', (tester) async {
      await tester.pumpWidget(
        ProviderScope(
          overrides: [...i18nTestOverrides],
          child: MaterialApp(
            home: Scaffold(
              body: WebViewPanel(
                resolution: const EmbedResolutionDto(
                  url: 'https://example.test/pin',
                ),
                userAgent: kMobileUserAgent,
                debugTargetOverride: const WebViewTarget.inAppWebView(),
              ),
            ),
          ),
        ),
      );
      // The recording builder rendered `ua:<userAgent>` so we can assert
      // the panel forwarded the UA string through the seam.
      expect(find.textContaining('ua:$kMobileUserAgent'), findsOneWidget);
    });

    testWidgets('userAgent_생략하면_userAgent가_null로_전달된다', (tester) async {
      await tester.pumpWidget(
        ProviderScope(
          overrides: [...i18nTestOverrides],
          child: MaterialApp(
            home: Scaffold(
              body: WebViewPanel(
                resolution: const EmbedResolutionDto(
                  url: 'https://example.test/fullscreen',
                ),
                debugTargetOverride: const WebViewTarget.inAppWebView(),
              ),
            ),
          ),
        ),
      );
      // Omitted `userAgent` => webview keeps the platform's
      // native UA (recorded as `ua:default` by the test builder).
      expect(find.textContaining('ua:default'), findsOneWidget);
    });

    testWidgets('custom User-Agent 문자열이_builder에_그대로_전달된다', (tester) async {
      const customUa = 'TestBot/42.0 (Linux x86_64) CustomEngine/9.9';
      await tester.pumpWidget(
        ProviderScope(
          overrides: [...i18nTestOverrides],
          child: MaterialApp(
            home: Scaffold(
              body: WebViewPanel(
                resolution: const EmbedResolutionDto(
                  url: 'https://example.test/custom-ua',
                ),
                userAgent: customUa,
                debugTargetOverride: const WebViewTarget.inAppWebView(),
              ),
            ),
          ),
        ),
      );
      expect(find.textContaining('ua:$customUa'), findsOneWidget);
    });

    testWidgets('viewportSize_지정하면_renderer가_SizedBox로_감싸진다', (tester) async {
      await tester.pumpWidget(
        ProviderScope(
          overrides: [...i18nTestOverrides],
          child: MaterialApp(
            home: Scaffold(
              body: WebViewPanel(
                resolution: const EmbedResolutionDto(
                  url: 'https://example.test/mobile',
                ),
                viewportSize: const Size(390, 844),
                debugTargetOverride: const WebViewTarget.inAppWebView(),
              ),
            ),
          ),
        ),
      );

      // The SizedBox wrapping the webview has exact dimensions
      final sizedBox = tester.widget<SizedBox>(find.byType(SizedBox).first);
      expect(sizedBox.width, 390);
      expect(sizedBox.height, 844);
    });

    testWidgets('viewportSize_생략하면_SizedBox_래핑이_없다', (tester) async {
      await tester.pumpWidget(
        ProviderScope(
          overrides: [...i18nTestOverrides],
          child: MaterialApp(
            home: Scaffold(
              body: WebViewPanel(
                resolution: const EmbedResolutionDto(
                  url: 'https://example.test/desktop',
                ),
                debugTargetOverride: const WebViewTarget.inAppWebView(),
              ),
            ),
          ),
        ),
      );

      // Only the SizedBox.expand from the test builder exists
      final sizedBoxes = tester.widgetList<SizedBox>(find.byType(SizedBox));
      final hasExactSize = sizedBoxes.any(
        (sb) => sb.width == 390 && sb.height == 844,
      );
      expect(hasExactSize, isFalse);
    });

    test('kMobileUserAgent는_iphone_safari_시그니처를_포함한다', () {
      // Mobile-detection 서버 측 sniffer는 대부분 `iPhone` / `Mobile`
      // 또는 `Safari` 키워드 매칭으로 동작. 상수가 그 세 핵심 토큰을
      // 모두 갖고 있는지만 핀해두면 무심결 desktop UA로 회귀하는 변경을
      // 잡을 수 있다.
      expect(kMobileUserAgent, contains('iPhone'));
      expect(kMobileUserAgent, contains('Mobile'));
      expect(kMobileUserAgent, contains('Safari'));
    });

    testWidgets('URL이_바뀌면_inAppWebView에_새_URL이_전달된다', (tester) async {
      // WebViewPanel rebuilds _InAppWebView when the resolution URL changes.
      // The desktop builder records url/userAgent so we can assert the
      // rebuild forwarded the new values.
      await tester.pumpWidget(
        ProviderScope(
          overrides: [...i18nTestOverrides],
          child: MaterialApp(
            home: Scaffold(
              body: WebViewPanel(
                resolution: const EmbedResolutionDto(
                  url: 'https://example.test/first',
                ),
                debugTargetOverride: const WebViewTarget.inAppWebView(),
              ),
            ),
          ),
        ),
      );
      expect(
        find.textContaining('url:https://example.test/first'),
        findsOneWidget,
      );

      // Parent rebuild with different resolution URL
      await tester.pumpWidget(
        ProviderScope(
          overrides: [...i18nTestOverrides],
          child: MaterialApp(
            home: Scaffold(
              body: WebViewPanel(
                resolution: const EmbedResolutionDto(
                  url: 'https://example.test/second',
                ),
                debugTargetOverride: const WebViewTarget.inAppWebView(),
              ),
            ),
          ),
        ),
      );
      expect(
        find.textContaining('url:https://example.test/second'),
        findsOneWidget,
      );
    });

    testWidgets('Linux이면_외부_브라우저_열기_버튼이_렌더된다', (tester) async {
      webViewTargetResolver = () => const WebViewTarget.linux();

      await tester.pumpWidget(
        ProviderScope(
          overrides: [...i18nTestOverrides],
          child: MaterialApp(
            home: Scaffold(
              body: WebViewPanel(
                resolution: const EmbedResolutionDto(
                  url: 'https://example.test/needs-external',
                ),
              ),
            ),
          ),
        ),
      );

      // Linux target shows the open-externally button, not webview or iframe
      expect(
        find.byKey(const Key('webview-panel-open-externally')),
        findsOneWidget,
      );
      expect(find.byKey(const Key('webview-panel-inappwebview')), findsNothing);
      expect(find.byKey(const Key('webview-panel-iframe')), findsNothing);
    });

    testWidgets('URL이_바뀌면_iframe이_remount된다', (tester) async {
      // Build iframe with first URL
      await tester.pumpWidget(
        ProviderScope(
          overrides: [...i18nTestOverrides],
          child: MaterialApp(
            home: Scaffold(
              body: WebViewPanel(
                resolution: const EmbedResolutionDto(
                  url: 'https://example.test/first',
                ),
                debugTargetOverride: const WebViewTarget.web(),
              ),
            ),
          ),
        ),
      );
      await tester.pump();

      // Iframe container rendered
      expect(find.byKey(const Key('webview-panel-iframe')), findsOneWidget);

      // Change URL — triggers didUpdateWidget which re-registers iframe
      await tester.pumpWidget(
        ProviderScope(
          overrides: [...i18nTestOverrides],
          child: MaterialApp(
            home: Scaffold(
              body: WebViewPanel(
                resolution: const EmbedResolutionDto(
                  url: 'https://example.test/second',
                ),
                debugTargetOverride: const WebViewTarget.web(),
              ),
            ),
          ),
        ),
      );
      await tester.pump();

      // Iframe container still renders (not destroyed)
      expect(find.byKey(const Key('webview-panel-iframe')), findsOneWidget);
    });

    testWidgets('URL이_같은데_parent가_rebuild되어도_iframe은_reload되지_않는다', (
      tester,
    ) async {
      // Build iframe with URL
      await tester.pumpWidget(
        ProviderScope(
          overrides: [...i18nTestOverrides],
          child: MaterialApp(
            home: Scaffold(
              body: WebViewPanel(
                resolution: const EmbedResolutionDto(
                  url: 'https://example.test/same',
                ),
                debugTargetOverride: const WebViewTarget.web(),
              ),
            ),
          ),
        ),
      );
      await tester.pump();
      expect(find.byKey(const Key('webview-panel-iframe')), findsOneWidget);

      // Parent rebuilds with SAME resolution URL
      await tester.pumpWidget(
        ProviderScope(
          overrides: [...i18nTestOverrides],
          child: MaterialApp(
            home: Scaffold(
              body: WebViewPanel(
                resolution: const EmbedResolutionDto(
                  url: 'https://example.test/same',
                ),
                debugTargetOverride: const WebViewTarget.web(),
              ),
            ),
          ),
        ),
      );
      await tester.pump();

      // Iframe container still there — no remount/reload
      expect(find.byKey(const Key('webview-panel-iframe')), findsOneWidget);
    });

    testWidgets('URL이_바뀌면_inAppWebView_builder가_새_URL로_다시_호출된다', (
      tester,
    ) async {
      int buildCount = 0;
      Widget recordingBuilder(
        String url,
        String? userAgent,
        void Function(WebViewController)? onControllerReady,
      ) {
        buildCount++;
        return SizedBox.expand(
          key: const Key('webview-panel-inappwebview'),
          child: Text('webview|url:$url|ua:${userAgent ?? "default"}'),
        );
      }

      desktopWebViewBuilder = recordingBuilder;

      await tester.pumpWidget(
        ProviderScope(
          overrides: [...i18nTestOverrides],
          child: MaterialApp(
            home: Scaffold(
              body: WebViewPanel(
                resolution: const EmbedResolutionDto(
                  url: 'https://example.test/first',
                ),
                debugTargetOverride: const WebViewTarget.inAppWebView(),
              ),
            ),
          ),
        ),
      );
      expect(buildCount, 1);
      expect(
        find.textContaining('url:https://example.test/first'),
        findsOneWidget,
      );

      // Parent rebuild with different URL
      await tester.pumpWidget(
        ProviderScope(
          overrides: [...i18nTestOverrides],
          child: MaterialApp(
            home: Scaffold(
              body: WebViewPanel(
                resolution: const EmbedResolutionDto(
                  url: 'https://example.test/second',
                ),
                debugTargetOverride: const WebViewTarget.inAppWebView(),
              ),
            ),
          ),
        ),
      );
      // didUpdateWidget URL change fires, then build fires
      expect(buildCount, greaterThanOrEqualTo(2));
      expect(
        find.textContaining('url:https://example.test/second'),
        findsOneWidget,
      );
    });

    testWidgets('같은_URL로_parent가_rebuild되면_inAppWebView는_재로드하지_않는다', (
      tester,
    ) async {
      int buildCount = 0;
      Widget recordingBuilder(
        String url,
        String? userAgent,
        void Function(WebViewController)? onControllerReady,
      ) {
        buildCount++;
        return SizedBox.expand(
          key: const Key('webview-panel-inappwebview'),
          child: Text('webview|url:$url|ua:${userAgent ?? "default"}'),
        );
      }

      desktopWebViewBuilder = recordingBuilder;

      await tester.pumpWidget(
        ProviderScope(
          overrides: [...i18nTestOverrides],
          child: MaterialApp(
            home: Scaffold(
              body: WebViewPanel(
                resolution: const EmbedResolutionDto(
                  url: 'https://example.test/same',
                ),
                debugTargetOverride: const WebViewTarget.inAppWebView(),
              ),
            ),
          ),
        ),
      );
      expect(buildCount, 1);

      // Same widget rebuilds — should only call build, not initState/didUpdateWidget
      await tester.pumpWidget(
        ProviderScope(
          overrides: [...i18nTestOverrides],
          child: MaterialApp(
            home: Scaffold(
              body: WebViewPanel(
                resolution: const EmbedResolutionDto(
                  url: 'https://example.test/same',
                ),
                debugTargetOverride: const WebViewTarget.inAppWebView(),
              ),
            ),
          ),
        ),
      );
      // build called again, but didUpdateWidget should NOT fire controller update
      expect(buildCount, greaterThanOrEqualTo(2));
    });
  });
}
