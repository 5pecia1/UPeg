/// Web-only impl for `IframePlatformView`. Loaded by Dart's
/// conditional-import lookup when `dart.library.html` is true (i.e.
/// web / wasm builds only). Desktop builds get
/// `_iframe_platform_view_stub.dart` instead.
///
/// `dart:ui_web` is the supported entry point for registering platform
/// view factories on the web target; the legacy `dart:ui` registry
/// was deprecated in Flutter 3.13.
library;

// This file is only loaded by the web conditional import; it never
// participates in desktop builds.

import 'dart:ui_web' as ui_web;

import 'package:flutter/widgets.dart';
import 'package:web/web.dart' as web;

abstract class IframePlatformView {
  /// `onLoad`, when supplied, is bound to the `<iframe>`'s `load`
  /// event so the caller's `IframeLoadObserver` can settle without
  /// needing to look up the element itself.
  static void registerIframeView(
    String viewType,
    String url, {
    void Function()? onLoad,
  }) {
    ui_web.platformViewRegistry.registerViewFactory(viewType, (int _) {
      final iframe = web.HTMLIFrameElement()
        ..src = url
        ..style.border = 'none'
        ..style.width = '100%'
        ..style.height = '100%';
      if (onLoad != null) {
        iframe.onLoad.listen((_) => onLoad());
      }
      return iframe;
    });
  }

  static Widget buildEmbedView(String viewType) {
    return HtmlElementView(viewType: viewType);
  }
}
