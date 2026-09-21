/// Unit tests for [DeepLinkListener] handler wiring.
///
/// The full [DeepLinkListener.install] subscribes to
/// `AppLinks().uriLinkStream`, which can't be driven from a unit test
/// without platform channels. The test surface is the
/// `handleForTest(container, uri, parser: ...)` seam that mirrors what
/// the stream callback does on a real second-instance event. After D03
/// the handler writes the parsed `LaunchIntent` into
/// [launchIntentProvider] — the same provider the cold-boot path and
/// the [`LaunchIntentApplier`] observer share.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter/foundation.dart' show DebugPrintCallback, debugPrint;
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/platform/deep_link_listener.dart';
import 'package:upeg/src/rust/api/deep_link.dart' show LaunchIntentDto;
import 'package:upeg/src/state/launch_intent_provider.dart';

void main() {
  group('DeepLinkListener.handleForTest', () {
    test('deep_link_listener_forwards_a_upeg_uri_to_launchIntentProvider', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      // Drive a second-instance event manually with an injected
      // parser so the test stays dylib-free. The handler must write
      // the parsed intent into the typed shared provider.
      DeepLinkListener.handleForTest(
        container,
        Uri.parse('upeg://open?surface=ext&board=dev&tool=id.uuid_v7'),
        parser: (_) => const LaunchIntentDto(board: 'dev', tool: 'id.uuid_v7'),
      );

      final LaunchIntent? intent = container.read(launchIntentProvider);
      expect(intent, isNotNull);
      expect(intent!.tool?.value, 'id.uuid_v7');
      expect(intent.board, 'dev');
    });

    test('deep_link_listener_ignores_a_non_upeg_uri', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      // Parser returns null for non-upeg input; the listener must not
      // touch the provider.
      DeepLinkListener.handleForTest(
        container,
        Uri.parse('https://example.test'),
        parser: (_) => null,
      );

      expect(container.read(launchIntentProvider), isNull);
    });

    test('deep_link_listener_logs_and_ignores_a_malformed_upeg_uri', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);
      final logs = <String>[];
      final DebugPrintCallback previousDebugPrint = debugPrint;
      debugPrint = (String? message, {int? wrapWidth}) {
        if (message != null) {
          logs.add(message);
        }
      };
      addTearDown(() {
        debugPrint = previousDebugPrint;
      });

      DeepLinkListener.handleForTest(
        container,
        Uri.parse('upeg://nonsense'),
        parser: (_) => null,
      );

      expect(container.read(launchIntentProvider), isNull);
      expect(logs, hasLength(1));
      expect(logs.single, contains('unsupported deep link'));
      expect(logs.single, contains('upeg://nonsense'));
    });
  });
}
