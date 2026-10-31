import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/platform/motion_preferences.dart';

void main() {
  const channel = MethodChannel('upeg/accessibility');

  testWidgets('should honor framework reduced motion before native queries', (
    tester,
  ) async {
    final messenger = tester.binding.defaultBinaryMessenger;
    var nativeCalls = 0;
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    tester.platformDispatcher.accessibilityFeaturesTestValue =
        const FakeAccessibilityFeatures(disableAnimations: true);
    messenger.setMockMethodCallHandler(channel, (_) async {
      nativeCalls++;
      return false;
    });

    try {
      expect(await motionDisabledBySystem(), isTrue);
      expect(nativeCalls, 0);
    } finally {
      debugDefaultTargetPlatformOverride = null;
      tester.platformDispatcher.clearAccessibilityFeaturesTestValue();
      messenger.setMockMethodCallHandler(channel, null);
    }
  });

  testWidgets('should query native preferences only in desktop runtimes', (
    tester,
  ) async {
    final messenger = tester.binding.defaultBinaryMessenger;
    var nativeCalls = 0;
    tester.platformDispatcher.accessibilityFeaturesTestValue =
        const FakeAccessibilityFeatures();
    messenger.setMockMethodCallHandler(channel, (call) async {
      expect(call.method, 'reduceMotion');
      nativeCalls++;
      return true;
    });

    try {
      for (final platform in [TargetPlatform.macOS, TargetPlatform.windows]) {
        debugDefaultTargetPlatformOverride = platform;
        expect(await motionDisabledBySystem(), !kIsWeb);
      }
      expect(nativeCalls, kIsWeb ? 0 : 2);
    } finally {
      debugDefaultTargetPlatformOverride = null;
      tester.platformDispatcher.clearAccessibilityFeaturesTestValue();
      messenger.setMockMethodCallHandler(channel, null);
    }
  });
}
