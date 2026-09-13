import 'package:flutter/widgets.dart';
import 'package:webview_platform_interface/webview_platform_interface.dart';

/// Exercises session ownership without requiring native platform channels.
class FakeWebViewPlatform extends WebViewPlatform {
  final List<FakePlatformWebViewController> controllers = [];
  bool failLoading = false;

  @override
  bool get supportsOffscreenWebViews => true;

  @override
  PlatformWebViewController createPlatformWebViewController(
    PlatformWebViewControllerCreationParams params,
  ) {
    final controller = FakePlatformWebViewController(params, this);
    controllers.add(controller);
    return controller;
  }

  @override
  PlatformNavigationDelegate createPlatformNavigationDelegate(
    PlatformNavigationDelegateCreationParams params,
  ) => FakePlatformNavigationDelegate(params);

  @override
  PlatformWebViewWidget createPlatformWebViewWidget(
    PlatformWebViewWidgetCreationParams params,
  ) => FakePlatformWebViewWidget(params);
}

class FakePlatformWebViewController extends PlatformWebViewController {
  FakePlatformWebViewController(super.params, this.owner)
    : super.implementation();

  final FakeWebViewPlatform owner;
  int closeCount = 0;
  int navigationCount = 0;
  JavaScriptMode? javaScriptMode;
  String? userAgent;
  FakePlatformNavigationDelegate? navigation;
  final List<String> commands = [];

  @override
  Future<bool> isOffscreenWebViewSupported() async => true;

  @override
  Future<void> closeOffscreenWebView() async => closeCount++;

  @override
  Future<void> setJavaScriptMode(JavaScriptMode mode) async {
    javaScriptMode = mode;
  }

  @override
  Future<void> setUserAgent(String? value) async {
    userAgent = value;
  }

  @override
  Future<void> setPlatformNavigationDelegate(
    PlatformNavigationDelegate handler,
  ) async {
    navigation = handler as FakePlatformNavigationDelegate;
  }

  @override
  Future<void> loadRequest(LoadRequestParams params) async {
    navigationCount++;
    if (owner.failLoading) {
      navigation?.onWebResourceError?.call(
        const WebResourceError(
          errorCode: 1,
          description: '테스트 페이지 로드 실패',
          isForMainFrame: true,
        ),
      );
    } else {
      navigation?.onPageFinished?.call(params.uri.toString());
    }
  }

  @override
  Future<void> runJavaScript(String javaScript) async {
    commands.add(javaScript);
  }

  @override
  Future<Object> runJavaScriptReturningResult(String javaScript) async {
    commands.add(javaScript);
    return '테스트 결과';
  }
}

class FakePlatformNavigationDelegate extends PlatformNavigationDelegate {
  FakePlatformNavigationDelegate(super.params) : super.implementation();

  PageEventCallback? onPageFinished;
  WebResourceErrorCallback? onWebResourceError;

  @override
  Future<void> setOnPageFinished(PageEventCallback callback) async {
    onPageFinished = callback;
  }

  @override
  Future<void> setOnWebResourceError(WebResourceErrorCallback callback) async {
    onWebResourceError = callback;
  }
}

class FakePlatformWebViewWidget extends PlatformWebViewWidget {
  FakePlatformWebViewWidget(super.params) : super.implementation();

  @override
  Widget build(BuildContext context) => const SizedBox.expand();
}
