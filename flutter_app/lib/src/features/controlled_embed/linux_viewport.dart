/// Linux WebKitGTK sizing requires a native allocation before it can be hidden.
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:webview_all/webview_all.dart';
import 'package:webview_all_linux/webview_all_linux.dart';

const _hiddenViewportGap = 16.0;
const _layoutTimeout = Duration(seconds: 5);
const _layoutPollInterval = Duration(milliseconds: 20);

bool isLinuxWebView(WebViewController controller) =>
    controller.platform is LinuxWebViewController;

Future<void> prepareLinuxHiddenViewport(
  WebViewController webView,
  Size size, {
  required bool Function() isMounted,
}) async {
  final controller = webView.platform as LinuxWebViewController;
  final frame = Rect.fromLTWH(
    -size.width - _hiddenViewportGap,
    -size.height - _hiddenViewportGap,
    size.width,
    size.height,
  );
  // GTK only allocates mapped widgets. Allocate outside the app window, then
  // hide the native container with its size intact so it cannot take focus.
  await webView.setJavaScriptMode(JavaScriptMode.unrestricted);
  await controller.setFrame(frame, visible: true);
  final deadline = DateTime.now().add(_layoutTimeout);
  while (isMounted()) {
    final matches = await webView.runJavaScriptReturningResult(
      'window.innerWidth === ${size.width} && '
      'window.innerHeight === ${size.height}',
    );
    if (matches == true) {
      await controller.setFrame(frame, visible: false);
      return;
    }
    if (DateTime.now().isAfter(deadline)) {
      throw TimeoutException('The native WebView viewport was not allocated.');
    }
    await Future<void>.delayed(_layoutPollInterval);
  }
}
