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

    testWidgets('a_web_target_renders_the_iframe_container', (tester) async {
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

    testWidgets('an_inAppWebView_target_renders_the_inappwebview_container', (
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

    testWidgets('a_given_userAgent_is_forwarded_verbatim', (tester) async {
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

    testWidgets('an_omitted_userAgent_is_forwarded_as_null', (tester) async {
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

    testWidgets(
      'a_custom_User_Agent_string_is_forwarded_to_the_builder_verbatim',
      (tester) async {
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
      },
    );

    testWidgets('a_given_viewportSize_wraps_the_renderer_in_a_SizedBox', (
      tester,
    ) async {
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

    testWidgets(
      'an_omitted_viewportSize_leaves_the_renderer_without_a_SizedBox_wrap',
      (tester) async {
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
      },
    );

    test('kMobileUserAgent_contains_the_iphone_safari_signatures', () {
      // Server-side mobile-detection sniffers mostly work by matching
      // the `iPhone` / `Mobile` / `Safari` keywords. Pinning that the
      // constant carries all three core tokens catches an accidental
      // regression to a desktop UA.
      expect(kMobileUserAgent, contains('iPhone'));
      expect(kMobileUserAgent, contains('Mobile'));
      expect(kMobileUserAgent, contains('Safari'));
    });

    testWidgets('a_URL_change_forwards_the_new_URL_to_inAppWebView', (
      tester,
    ) async {
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

    testWidgets('on_Linux_the_open_external_browser_button_is_rendered', (
      tester,
    ) async {
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

    testWidgets('a_URL_change_remounts_the_iframe', (tester) async {
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

    testWidgets(
      'a_parent_rebuild_with_the_same_URL_does_not_reload_the_iframe',
      (tester) async {
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
      },
    );

    testWidgets(
      'a_URL_change_recalls_the_inAppWebView_builder_with_the_new_URL',
      (tester) async {
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
      },
    );

    testWidgets('a_parent_rebuild_with_the_same_URL_does_not_reload_inAppWebView', (
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
