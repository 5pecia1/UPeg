/// Desktop stub for `dart:ui_web` + `dart:html` iframe wiring.
///
/// This file is the default import — Dart's conditional-import lookup
/// (`if (dart.library.html) '_iframe_platform_view_web.dart'`)
/// replaces it on the web target only. On desktop the production
/// embed branch never reaches this stub: `EmbedPanel.build`
/// short-circuits to the external launcher when `!kIsWeb`. Tests
/// inject a fake `IframeFactory` before any panel runs, so they
/// bypass the [registerIframeView] hook entirely; [buildEmbedView]
/// still falls back to a placeholder widget so widget tests can
/// locate the mount point by key without hitting Flutter's web-only
/// `HtmlElementView` constructor.
library;

import 'package:flutter/widgets.dart';

abstract class IframePlatformView {
  /// Registers a platform view factory under [viewType] that mounts an
  /// `<iframe src="$url">`. Desktop builds must never reach here.
  ///
  /// `onLoad` is the web-only `iframe.onLoad` hook used by the
  /// `_IframeWithTimeout` widget to settle its `IframeLoadObserver` —
  /// desktop tests pass through the optional callback unobserved.
  static void registerIframeView(
    String viewType,
    String url, {
    void Function()? onLoad,
  }) {
    throw UnsupportedError(
      'IframePlatformView.registerIframeView is web-only; '
      'desktop builds must route embeds to the in-app webview.',
    );
  }

  /// Build the platform-view widget that mounts [viewType]. Desktop
  /// builds receive an inert sized placeholder so tests can assert by
  /// key; the real `HtmlElementView` lives in the web variant.
  static Widget buildEmbedView(String viewType) {
    return SizedBox.expand(key: ValueKey('embed-view-$viewType'));
  }
}
