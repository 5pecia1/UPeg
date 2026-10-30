/// OS motion preferences shared by startup and tray cues.
library;

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

const _motionChannel = MethodChannel('upeg/accessibility');

/// macOS/Windows embedders do not expose the OS animation preference through
/// Flutter's accessibility flags, so the runners supply a small native query.
/// An unavailable query keeps the static image. Callers start from a static
/// image and query only while their finite cue is active.
Future<bool> motionDisabledBySystem() async {
  if (WidgetsBinding
      .instance
      .platformDispatcher
      .accessibilityFeatures
      .disableAnimations) {
    return true;
  }
  if (kIsWeb) return false;
  if (defaultTargetPlatform != TargetPlatform.macOS &&
      defaultTargetPlatform != TargetPlatform.windows) {
    return false;
  }
  try {
    return await _motionChannel
            .invokeMethod<bool>('reduceMotion')
            .timeout(
              const Duration(milliseconds: 200),
              onTimeout: () => true,
            ) ??
        true;
  } on Exception {
    return true;
  }
}
