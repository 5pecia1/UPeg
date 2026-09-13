/// Embed iframe factory — registers a platform view per URL.
///
/// The web target's `<iframe>` rendering needs a platform-view factory
/// registered with `dart:ui_web`. Wrapping that registration in an
/// abstract [IframeFactory] gives tests a clean seam (override the
/// global, assert the URL was registered) without threading a `kIsWeb`
/// boolean through every widget. The desktop build never invokes
/// [DartUiWebIframeFactory]; `EmbedPanel`'s `kIsWeb` guard skips the
/// iframe branch entirely. On web, `dart:ui_web` + `dart:html` are
/// loaded through the conditional-import wrapper in
/// `_iframe_platform_view_stub.dart` / `_iframe_platform_view_web.dart`.
library;

import 'package:flutter/foundation.dart' show protected;

import 'package:upeg/src/widgets/_iframe_platform_view_stub.dart'
    if (dart.library.html) 'package:upeg/src/widgets/_iframe_platform_view_web.dart';

/// Stable view-type prefix. Each distinct URL gets `prefix-<counter>`
/// where the counter is allocated monotonically — keyed on the full
/// URL string, never `url.hashCode`, so two pages with the same hash
/// can't share one iframe factory.
const String _kEmbedIframeViewTypePrefix = 'embed-iframe';

/// Strategy for registering an iframe platform view. Tests inject a
/// recording fake; production wires the real `dart:ui_web` registry.
abstract class IframeFactory {
  const IframeFactory();

  /// Register a platform view backed by an `<iframe src="$url">` and
  /// return the view-type identifier the caller can mount via
  /// `HtmlElementView(viewType: ...)`.
  ///
  /// Implementations MUST be idempotent: calling `register(url)` twice
  /// with the same URL must produce the same view-type string and
  /// must not register a second factory under `dart:ui_web`.
  ///
  /// `onLoad`, when supplied, is bound to the iframe's `onLoad` event so
  /// the caller can settle a [`IframeLoadObserver`] without inspecting
  /// the DOM directly. Re-registering with a fresh `onLoad` callback
  /// (e.g. after a reload-nonce bump) replaces the previous binding.
  String register(String url, {void Function()? onLoad});
}

/// Production factory — registers a real `<iframe>` element via
/// `dart:ui_web`'s platform-view registry. Only safe to invoke on web;
/// desktop callers must gate on `kIsWeb` upstream (see `EmbedPanel`).
class DartUiWebIframeFactory extends IframeFactory {
  const DartUiWebIframeFactory();

  /// Monotonic counter for minting collision-free view types.
  /// Each mount gets a fresh type — `prefix-<counter>` — so the same
  /// URL rendered twice produces different view types and avoids the
  /// `registerViewFactory` "already registered" edge case.
  static int _nextId = 0;

  @override
  String register(String url, {void Function()? onLoad}) {
    final viewType = '$_kEmbedIframeViewTypePrefix-${_nextId++}';
    registerView(viewType, url, onLoad: onLoad);
    return viewType;
  }

  /// Platform-registration seam. Production wires the real
  /// `dart:ui_web` registry; tests override this to record without
  /// hitting the desktop stub (which throws "web-only").
  ///
  /// The conditional-import wrapper picks the web impl when
  /// `dart.library.html` is true. On desktop the stub throws — but
  /// production never gets here (WebViewPanel routes desktop to
  /// `InAppWebView` instead) and tests override either
  /// `embedIframeFactory` or this method.
  @protected
  void registerView(String viewType, String url, {void Function()? onLoad}) {
    IframePlatformView.registerIframeView(viewType, url, onLoad: onLoad);
  }
}

/// Process-wide iframe factory. Production code reads this global and
/// gets the default [DartUiWebIframeFactory]; tests overwrite it with
/// a recording fake (see `_RecordingIframeFactory` in the widget
/// tests) and restore the default in `tearDown`. Not annotated
/// `@visibleForTesting` because the production `EmbedPanel` build also
/// reads it — the seam is the variable itself, not the assignment.
IframeFactory embedIframeFactory = const DartUiWebIframeFactory();
