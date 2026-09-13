/// Platform-branching webview panel.
///
/// Sealed `WebViewTarget` selects exactly one renderer per platform from
/// a single switch:
///   * `web`          → `_IframeWithTimeout` (`<iframe>` + 5s fallback)
///   * `inAppWebView` → `_InAppWebView` (webview_all on supported native
///                      desktop/mobile targets)
///   * `linux`        → `_LinuxFallback` (copy/open externally, no
///                      embedded webview plugin dependency)
///
/// Mobile user-agent is OPT-IN per call site, not a hardcoded global.
/// Narrow pin-tile mounts pass `userAgent: kMobileUserAgent` so embed
/// sites render as mobile (single column, large hit targets). The
/// full-screen `EmbedPage` keeps the default desktop UA so wide layouts
/// use their full grid.
library;

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/rust/api/embed.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/_iframe_platform_view_stub.dart'
    if (dart.library.html) 'package:upeg/src/widgets/_iframe_platform_view_web.dart';
import 'package:upeg/src/widgets/embed_iframe_factory.dart';
import 'package:upeg/src/widgets/iframe_load_observer.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:webview_all/webview_all.dart';

/// Single source for the iframe load timeout. Five seconds is generous
/// enough that a healthy page never trips it, short enough that the
/// fallback banner appears quickly when CSP/X-Frame-Options blocks render.
const Duration kIframeLoadTimeout = Duration(seconds: 5);

/// iOS Safari signature used when a caller opts into mobile rendering.
/// `iPhone` + `Mobile` + `Safari` tokens cover the widest range of
/// server-side mobile-detection sniffers including legacy ones that
/// only check the substring `iPhone`.
const String kMobileUserAgent =
    'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) '
    'AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 '
    'Mobile/15E148 Safari/604.1';

/// Test seam for the system-browser launcher (web fallback button).
@visibleForTesting
typedef WebViewUrlLauncher = Future<bool> Function(Uri uri);

@visibleForTesting
WebViewUrlLauncher webViewPanelLauncher = (uri) =>
    launchUrl(uri, mode: LaunchMode.externalApplication);

/// Test seam for the desktop / mobile in-app webview. Production wires
/// the real `webview_all` widget; widget tests overwrite this with a
/// sizedplaceholder because `WebViewWidget` needs a real platform
/// channel. Receives the URL + resolved UA + an optional controller
/// callback so callers (e.g. Controlled Embed) can capture the live
/// `WebViewController` for selector-adapter driving. Mirrors the
/// [`embedIframeFactory`] pattern.
@visibleForTesting
typedef DesktopWebViewBuilder =
    Widget Function(
      String url,
      String? userAgent,
      void Function(WebViewController)? onControllerReady,
    );

@visibleForTesting
DesktopWebViewBuilder desktopWebViewBuilder = _defaultDesktopWebViewBuilder;

Widget _defaultDesktopWebViewBuilder(
  String url,
  String? userAgent,
  void Function(WebViewController)? onControllerReady,
) {
  final controller = WebViewController();
  unawaited(
    _configureAndLoadController(
      controller: controller,
      url: url,
      userAgent: userAgent,
      onPageFinished: () => onControllerReady?.call(controller),
    ),
  );
  return WebViewWidget(
    key: const Key('webview-panel-inappwebview'),
    controller: controller,
  );
}

Future<void> _configureAndLoadController({
  required WebViewController controller,
  required String url,
  required String? userAgent,
  required void Function()? onPageFinished,
  bool Function()? isCurrent,
}) async {
  bool current() => isCurrent?.call() ?? true;

  await controller.setJavaScriptMode(JavaScriptMode.unrestricted);
  if (!current()) return;

  if (onPageFinished != null) {
    await controller.setNavigationDelegate(
      NavigationDelegate(
        onPageFinished: (_) {
          if (current()) onPageFinished();
        },
      ),
    );
    if (!current()) return;
  }

  if (userAgent != null) {
    await controller.setUserAgent(userAgent);
    if (!current()) return;
  }

  await controller.loadRequest(Uri.parse(url));
}

/// Sealed enum: which renderer should `WebViewPanel` mount.
///
/// Drives the [`WebViewPanel`] switch. Computed by [`webViewTargetResolver`]
/// from `kIsWeb`; widget tests overwrite the result via
/// [`WebViewPanel.debugTargetOverride`].
sealed class WebViewTarget {
  const WebViewTarget();
  const factory WebViewTarget.web() = _WebViewTargetWeb;
  const factory WebViewTarget.inAppWebView() = _WebViewTargetInApp;
  const factory WebViewTarget.linux() = _WebViewTargetLinux;
}

final class _WebViewTargetWeb extends WebViewTarget {
  const _WebViewTargetWeb();
}

final class _WebViewTargetInApp extends WebViewTarget {
  const _WebViewTargetInApp();
}

final class _WebViewTargetLinux extends WebViewTarget {
  const _WebViewTargetLinux();
}

WebViewTarget _defaultWebViewTargetFor() {
  if (kIsWeb) return const WebViewTarget.web();
  if (defaultTargetPlatform == TargetPlatform.linux) {
    return const WebViewTarget.linux();
  }
  return const WebViewTarget.inAppWebView();
}

/// Test seam for the platform branch. Production reads
/// [`_defaultWebViewTargetFor`]; widget tests overwrite this with a
/// fixed-result closure (mirrors the [`desktopWebViewBuilder`] /
/// [`embedIframeFactory`] / [`webViewPanelLauncher`] seams).
@visibleForTesting
WebViewTarget Function() webViewTargetResolver = _defaultWebViewTargetFor;

/// Whether the resolved platform target can supply a live
/// [`WebViewController`] for inline JavaScript driving.
///
/// Only the in-app webview target (native desktop/mobile) can hand back
/// a controller. The web iframe path (`<iframe>` has no controller) and
/// the Linux external-launcher fallback both run without one. Callers
/// that need controller-backed execution — e.g. `ControlledEmbedTile`'s
/// inline Run — use this to make the no-controller state first-class:
/// disable Run and show an "open externally" affordance instead of
/// always failing with a controller-missing error.
///
/// Pass [target] to test a specific branch; omit it to consult the live
/// [`webViewTargetResolver`] seam.
bool webViewTargetSupportsController([WebViewTarget? target]) {
  final resolved = target ?? webViewTargetResolver();
  return switch (resolved) {
    _WebViewTargetInApp() => true,
    _WebViewTargetWeb() => false,
    _WebViewTargetLinux() => false,
  };
}

/// Renders the resolved embed URL, picking the platform-appropriate
/// renderer. Stateless — state lives in the chosen child widget.
class WebViewPanel extends StatelessWidget {
  const WebViewPanel({
    required this.resolution,
    this.userAgent,
    this.viewportSize,
    this.onControllerReady,
    this.debugTargetOverride,
    super.key,
  });

  final EmbedResolutionDto resolution;

  /// Explicit user-agent string for the in-app webview.
  /// `null` means the browser default. Pass [kMobileUserAgent] for mobile
  /// rendering. Ignored on the web target — iframes inherit the host UA.
  final String? userAgent;

  /// When non-null, the selected webview renderer is wrapped in a
  /// `SizedBox(width: viewportSize.width, height: viewportSize.height)`.
  /// This pins the layout viewport to a specific dimension, useful for
  /// mobile/tablet emulation. Ignored on the web target.
  final Size? viewportSize;

  /// Capture the live `WebViewController` (in-app webview path only).
  /// Controlled Embed uses this to drive selectors via JavaScript.
  /// Ignored on web (iframe has no controller).
  final void Function(WebViewController)? onControllerReady;

  /// Forces a specific renderer in widget tests. `null` (production
  /// default) defers to [`webViewTargetResolver`].
  final WebViewTarget? debugTargetOverride;

  @override
  Widget build(BuildContext context) {
    final target = debugTargetOverride ?? webViewTargetResolver();
    return switch (target) {
      _WebViewTargetWeb() => _IframeWithTimeout(url: resolution.url),
      _WebViewTargetInApp() => _InAppWebView(
        url: resolution.url,
        userAgent: userAgent,
        viewportSize: viewportSize,
        onControllerReady: onControllerReady,
      ),
      _WebViewTargetLinux() => _LinuxFallback(
        url: resolution.url,
        userAgent: userAgent,
        onControllerReady: onControllerReady,
      ),
    };
  }
}

class _InAppWebView extends StatefulWidget {
  const _InAppWebView({
    required this.url,
    required this.userAgent,
    required this.viewportSize,
    required this.onControllerReady,
  });
  final String url;
  final String? userAgent;
  final Size? viewportSize;
  final void Function(WebViewController)? onControllerReady;

  @override
  State<_InAppWebView> createState() => _InAppWebViewState();
}

class _InAppWebViewState extends State<_InAppWebView> {
  WebViewController? _controller;
  String _lastUrl = '';
  String? _lastUserAgent;
  Size? _lastViewportSize;

  bool get _isProduction =>
      identical(desktopWebViewBuilder, _defaultDesktopWebViewBuilder);

  Future<void> _loadRequest({required bool resetController}) async {
    if (resetController || _controller == null) {
      _controller = WebViewController();
    }
    final controller = _controller!;
    final url = widget.url;
    final userAgent = widget.userAgent;
    final onControllerReady = widget.onControllerReady;
    await _configureAndLoadController(
      controller: controller,
      url: url,
      userAgent: userAgent,
      onPageFinished: onControllerReady == null
          ? null
          : () => onControllerReady(controller),
      isCurrent: () => mounted && _controller == controller && _lastUrl == url,
    );
  }

  @override
  void initState() {
    super.initState();
    _lastUrl = widget.url;
    _lastUserAgent = widget.userAgent;
    _lastViewportSize = widget.viewportSize;
    if (_isProduction) {
      unawaited(_loadRequest(resetController: true));
    }
  }

  @override
  void didUpdateWidget(_InAppWebView oldWidget) {
    super.didUpdateWidget(oldWidget);
    final urlChanged = widget.url != _lastUrl;
    final uaChanged = widget.userAgent != _lastUserAgent;
    if (urlChanged || uaChanged) {
      _lastUrl = widget.url;
      _lastUserAgent = widget.userAgent;
      _lastViewportSize = widget.viewportSize;
      if (_isProduction) {
        unawaited(_loadRequest(resetController: urlChanged || uaChanged));
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final child = _isProduction && _controller != null
        ? WebViewWidget(
            key: const Key('webview-panel-inappwebview'),
            controller: _controller!,
          )
        : desktopWebViewBuilder(
            _lastUrl,
            _lastUserAgent,
            widget.onControllerReady,
          );
    final vp = _lastViewportSize;
    if (vp != null) {
      return SizedBox(width: vp.width, height: vp.height, child: child);
    }
    return child;
  }
}

class _IframeWithTimeout extends ConsumerStatefulWidget {
  const _IframeWithTimeout({required this.url});
  final String url;

  @override
  ConsumerState<_IframeWithTimeout> createState() => _IframeWithTimeoutState();
}

class _IframeWithTimeoutState extends ConsumerState<_IframeWithTimeout> {
  late IframeLoadObserver _observer;
  late String _viewType;
  IframeLoadState _state = const IframeLoading();
  StreamSubscription<IframeLoadState>? _sub;

  @override
  void initState() {
    super.initState();
    _observer = IframeLoadObserver(timeout: kIframeLoadTimeout);
    _sub = _observer.changes.listen((next) {
      if (!mounted) return;
      setState(() => _state = next);
    });
    _viewType = _registerIframe();
  }

  String _registerIframe() {
    return embedIframeFactory.register(
      widget.url,
      onLoad: _observer.notifyLoaded,
    );
  }

  @override
  void didUpdateWidget(_IframeWithTimeout oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.url != oldWidget.url) {
      _sub?.cancel();
      _observer.dispose();
      _state = const IframeLoading();
      _observer = IframeLoadObserver(timeout: kIframeLoadTimeout);
      _sub = _observer.changes.listen((next) {
        if (!mounted) return;
        setState(() => _state = next);
      });
      _viewType = _registerIframe();
    }
  }

  @override
  void dispose() {
    _sub?.cancel();
    _observer.dispose();
    super.dispose();
  }

  Future<void> _openExternally() async {
    final uri = Uri.tryParse(widget.url);
    if (uri == null) return;
    await webViewPanelLauncher(uri);
  }

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    return Stack(
      key: const Key('webview-panel-iframe'),
      children: [
        Positioned.fill(child: IframePlatformView.buildEmbedView(_viewType)),
        if (_state is IframeTimeout)
          Positioned(
            top: 0,
            left: 0,
            right: 0,
            child: Material(
              color: tokens.warn.withValues(alpha: 0.95),
              child: Padding(
                padding: const EdgeInsets.fromLTRB(16, 10, 16, 10),
                child: Row(
                  children: [
                    Expanded(
                      child: Text(
                        t(ref, 'embed.load_failed_hint'),
                        style: TextStyle(color: tokens.onWarn),
                      ),
                    ),
                    TextButton(
                      key: const Key('webview-panel-open-externally'),
                      onPressed: _openExternally,
                      child: Text(t(ref, 'embed.open_externally')),
                    ),
                  ],
                ),
              ),
            ),
          ),
      ],
    );
  }
}

/// Linux fallback — opens the resolved URL in the system browser.
///
/// `webview_all` has no Linux backend; rather than crashing, this widget
/// presents an "open externally" button so the user can still view the
/// embed content. SoC: this widget owns nothing but the launcher call.
class _LinuxFallback extends ConsumerWidget {
  const _LinuxFallback({
    required this.url,
    this.userAgent,
    required this.onControllerReady,
  });

  final String url;
  final String? userAgent;
  final void Function(WebViewController)? onControllerReady;

  /// No controller available on the Linux fallback path. We return the
  /// resolved URL via this stub; callers should check [onControllerReady]
  /// nullability upstream.
  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final uri = Uri.tryParse(url);
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          if (uri != null)
            TextButton(
              key: const Key('webview-panel-open-externally'),
              onPressed: () => webViewPanelLauncher(uri),
              child: Text(t(ref, 'embed.open_externally')),
            ),
        ],
      ),
    );
  }
}
