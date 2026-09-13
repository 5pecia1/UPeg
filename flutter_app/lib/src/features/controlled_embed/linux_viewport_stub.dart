/// Keeps Linux's dart:io-backed implementation outside PWA builds.
library;

import 'package:flutter/widgets.dart';
import 'package:webview_all/webview_all.dart';

bool isLinuxWebView(WebViewController controller) => false;

Future<void> prepareLinuxHiddenViewport(
  WebViewController controller,
  Size size, {
  required bool Function() isMounted,
}) =>
    throw UnsupportedError('Native Linux WebViews are unavailable on the web.');
