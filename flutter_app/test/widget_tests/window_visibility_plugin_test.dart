/// The production visibility seam against the real `window_manager` plugin
/// dispatcher: events sent over the plugin's method channel must reach
/// [windowShownProvider], and a disposed container must stop receiving them.
///
/// Lives apart from `state_tests/window_visibility_test.dart` because this
/// file needs a Flutter binding (the first `windowManager` access registers a
/// method-call handler), which would hide an accidental `windowManager` touch
/// in that file's binding-free tests.
library;

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/platform/window.dart' show isWindowManagerSupported;
import 'package:upeg/src/platform/window_visibility.dart';

/// Channel name the `window_manager` plugin uses for native -> Dart events.
const String _pluginChannel = 'window_manager';

/// Send one native event the way the platform side does, and fail the test if
/// the Dart-side handler answered with an error envelope (any exception thrown
/// by a listener is turned into one by the method channel).
Future<void> _deliverNativeEvent(WidgetTester tester, String eventName) async {
  const codec = StandardMethodCodec();
  final answered = Completer<ByteData?>();
  await tester.binding.defaultBinaryMessenger.handlePlatformMessage(
    _pluginChannel,
    codec.encodeMethodCall(
      MethodCall('onEvent', <String, Object?>{'eventName': eventName}),
    ),
    answered.complete,
  );
  final envelope = await answered.future;
  if (envelope != null) codec.decodeEnvelope(envelope);
}

void main() {
  group('WindowManagerVisibilitySource', () {
    testWidgets(
      'plugin_events_drive_the_window_shown_state',
      (tester) async {
        final container = ProviderContainer();
        addTearDown(container.dispose);
        expect(container.read(windowShownProvider), isTrue);

        await _deliverNativeEvent(tester, 'hide');
        expect(container.read(windowShownProvider), isFalse);

        await _deliverNativeEvent(tester, 'show');
        expect(container.read(windowShownProvider), isTrue);

        await _deliverNativeEvent(tester, 'minimize');
        expect(container.read(windowShownProvider), isFalse);

        await _deliverNativeEvent(tester, 'restore');
        expect(container.read(windowShownProvider), isTrue);

        // Windows brings a minimized, maximized window back as `maximize`.
        await _deliverNativeEvent(tester, 'minimize');
        expect(container.read(windowShownProvider), isFalse);

        await _deliverNativeEvent(tester, 'maximize');
        expect(container.read(windowShownProvider), isTrue);

        await _deliverNativeEvent(tester, 'hide');
        await _deliverNativeEvent(tester, 'blur');
        await _deliverNativeEvent(tester, 'resize');
        expect(
          container.read(windowShownProvider),
          isFalse,
          reason: 'unrelated events must not change the state',
        );
      },
      skip: !isWindowManagerSupported,
    );

    testWidgets(
      'a_disposed_container_is_unregistered_from_the_plugin',
      (tester) async {
        final container = ProviderContainer();
        container.read(windowShownProvider);
        container.dispose();

        // A listener left registered would throw on its disposed notifier,
        // which `_deliverNativeEvent` reports as an error envelope.
        await _deliverNativeEvent(tester, 'hide');
        await _deliverNativeEvent(tester, 'show');
      },
      skip: !isWindowManagerSupported,
    );
  });
}
