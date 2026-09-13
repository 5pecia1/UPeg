/// The native WebView lifetime shared by execution and its optional debugger.
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:upeg/src/widgets/controlled_embed/settings.dart';
import 'package:webview_all/webview_all.dart';

import 'package:upeg/src/features/controlled_embed/linux_viewport_stub.dart'
    if (dart.library.io) 'package:upeg/src/features/controlled_embed/linux_viewport.dart';

const kControlledEmbedSessionLoadTimeout = Duration(seconds: 30);

/// Browser operations needed by the executor, independent of a platform view.
///
/// The application must mount exactly one [buildView] per live session. It can
/// move that view between its hidden browser host and the visible debugger.
/// Removing the view does not close the browser or reload its page.
abstract interface class ControlledEmbedWebViewSession {
  /// Completes after the initial page has loaded in its configured viewport.
  Future<void> get ready;

  Size get viewportSize;

  Widget buildView({Key? key, bool visible = true});

  Future<void> runCommandJs(String script);

  Future<Object> runResultJs(String script);

  Future<void> close();
}

/// Owns native resources while the application owns the view's placement.
///
/// `OffscreenWebViewSession` supplies deterministic lifetime management, but
/// does not have a portable viewport sizing API. A mounted [WebViewWidget] is
/// therefore retained by the application even while its browser is hidden.
final class NativeControlledEmbedWebViewSession
    implements ControlledEmbedWebViewSession {
  NativeControlledEmbedWebViewSession._(this._session, this.viewportSize);

  /// Creates the controller without waiting for its view to be mounted.
  ///
  /// Publish the returned session to the application's browser host before
  /// awaiting [ready], so it can lay out the viewport and start navigation.
  static Future<NativeControlledEmbedWebViewSession> create({
    required String url,
    ResolvedBrowserSettings? settings,
    Duration loadTimeout = kControlledEmbedSessionLoadTimeout,
  }) async {
    final uri = Uri.parse(url);
    if (!uri.hasScheme) {
      throw ArgumentError.value(url, 'url', 'A browser URL needs a scheme.');
    }
    final ownedSession = await OffscreenWebViewSession.create();
    final session = NativeControlledEmbedWebViewSession._(
      ownedSession,
      settings?.viewportSize ?? ViewPortPresetSizes.desktop,
    );
    session._ready = session._initialize(uri, settings, loadTimeout);
    // The host may take a frame to mount the view before a caller awaits ready.
    // Preserve the original error for that caller while observing it eagerly.
    unawaited(session._ready.then<void>((_) {}, onError: (Object _) {}));
    return session;
  }

  final OffscreenWebViewSession _session;
  final Completer<void> _viewMounted = Completer<void>();
  final Completer<void> _pageLoaded = Completer<void>();
  Completer<void> _viewportReady = _observedCompleter();
  Object? _viewOwner;
  late final Future<void> _ready;

  @override
  final Size viewportSize;

  @override
  Future<void> get ready => _ready;

  Future<void> _initialize(
    Uri uri,
    ResolvedBrowserSettings? settings,
    Duration loadTimeout,
  ) async {
    // Navigation errors can arrive before loadRequest itself completes.
    unawaited(_pageLoaded.future.then<void>((_) {}, onError: (Object _) {}));
    try {
      await _configureAndLoad(uri, settings).timeout(loadTimeout);
    } catch (error, stackTrace) {
      try {
        await close();
      } catch (_) {
        // Report the original initialization failure if cleanup also fails.
      }
      Error.throwWithStackTrace(error, stackTrace);
    }
  }

  Future<void> _configureAndLoad(
    Uri uri,
    ResolvedBrowserSettings? settings,
  ) async {
    await _viewMounted.future;
    final controller = _session.controller;
    await controller.setJavaScriptMode(JavaScriptMode.unrestricted);
    if (settings?.hasUserAgentOverride ?? false) {
      await _session.controller.setUserAgent(settings!.userAgent);
    }
    await _session.controller.setNavigationDelegate(
      NavigationDelegate(
        onPageFinished: (_) {
          if (!_pageLoaded.isCompleted) _pageLoaded.complete();
        },
        onWebResourceError: (error) {
          if (error.isForMainFrame != false && !_pageLoaded.isCompleted) {
            _pageLoaded.completeError(
              StateError(
                'WebView page failed to load (${error.errorCode}): '
                '${error.description}',
              ),
            );
          }
        },
      ),
    );
    await _session.controller.loadRequest(uri);
    await _pageLoaded.future;
  }

  @override
  Widget buildView({Key? key, bool visible = true}) {
    final controller = _session.controller;
    return _SessionView(
      key: key,
      controller: controller,
      viewportSize: viewportSize,
      visible: visible,
      onAttached: (owner) {
        _viewOwner = owner;
        _resetViewportWait();
      },
      onDetached: (owner) {
        if (!identical(_viewOwner, owner)) return;
        _viewOwner = null;
        _resetViewportWait();
      },
      onMounted: (owner) {
        if (!identical(_viewOwner, owner) || _session.isClosed) return;
        if (!_viewportReady.isCompleted) _viewportReady.complete();
        if (!_viewMounted.isCompleted) _viewMounted.complete();
      },
      onMountError: (owner, error, trace) {
        if (!identical(_viewOwner, owner)) return;
        if (!_viewportReady.isCompleted) {
          _viewportReady.completeError(error, trace);
        }
        if (!_viewMounted.isCompleted) {
          _viewMounted.completeError(error, trace);
        }
      },
    );
  }

  @override
  Future<void> runCommandJs(String script) async {
    await ready;
    await _waitForViewport();
    await _session.controller.runJavaScript(script);
  }

  @override
  Future<Object> runResultJs(String script) async {
    await ready;
    await _waitForViewport();
    return _session.controller.runJavaScriptReturningResult(script);
  }

  void _resetViewportWait() {
    if (!_session.isClosed && _viewportReady.isCompleted) {
      _viewportReady = _observedCompleter();
    }
  }

  Future<void> _waitForViewport() async {
    while (true) {
      final viewport = _viewportReady;
      await viewport.future;
      if (identical(viewport, _viewportReady)) return;
    }
  }

  @override
  Future<void> close() {
    final error = StateError('The Controlled Embed browser session is closed.');
    if (!_viewMounted.isCompleted) _viewMounted.completeError(error);
    if (!_viewportReady.isCompleted) _viewportReady.completeError(error);
    if (!_pageLoaded.isCompleted) _pageLoaded.completeError(error);
    return _session.close();
  }
}

Completer<void> _observedCompleter() {
  final completer = Completer<void>();
  unawaited(completer.future.then<void>((_) {}, onError: (Object _) {}));
  return completer;
}

class _SessionView extends StatefulWidget {
  const _SessionView({
    required this.controller,
    required this.viewportSize,
    required this.visible,
    required this.onAttached,
    required this.onDetached,
    required this.onMounted,
    required this.onMountError,
    super.key,
  });

  final WebViewController controller;
  final Size viewportSize;
  final bool visible;
  final void Function(Object) onAttached;
  final void Function(Object) onDetached;
  final void Function(Object) onMounted;
  final void Function(Object, Object, StackTrace) onMountError;

  @override
  State<_SessionView> createState() => _SessionViewState();
}

class _SessionViewState extends State<_SessionView> {
  Object _owner = Object();
  bool get _hiddenLinux => !widget.visible && isLinuxWebView(widget.controller);

  @override
  void initState() {
    super.initState();
    widget.onAttached(_owner);
    _scheduleViewportMount();
  }

  @override
  void didUpdateWidget(_SessionView oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!identical(oldWidget.controller, widget.controller) ||
        oldWidget.visible != widget.visible) {
      oldWidget.onDetached(_owner);
      _owner = Object();
      widget.onAttached(_owner);
      _scheduleViewportMount();
    }
  }

  void _scheduleViewportMount() {
    final owner = _owner;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && identical(_owner, owner)) {
        unawaited(_mountViewport(owner));
      }
    });
  }

  Future<void> _mountViewport(Object owner) async {
    final view = widget;
    bool current() => mounted && identical(_owner, owner);
    try {
      if (_hiddenLinux) {
        await prepareLinuxHiddenViewport(
          view.controller,
          view.viewportSize,
          isMounted: current,
        );
      }
      if (current()) view.onMounted(owner);
    } catch (error, trace) {
      if (current()) view.onMountError(owner, error, trace);
    }
  }

  @override
  void dispose() {
    widget.onDetached(_owner);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => SizedBox.fromSize(
    size: widget.viewportSize,
    child: _hiddenLinux ? null : WebViewWidget(controller: widget.controller),
  );
}
