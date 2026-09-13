/// Unit tests for [isPauseControllable] + [pauseControllableProvider].
///
/// Pause is a process-local `AtomicBool` (WS4 PRD §5.9) — it cannot
/// reach a foreign daemon. The desktop control must be interactive
/// ONLY when THIS process hosts in-process (`HostStateEvent.embedded`).
/// Every other [HostStateEvent] variant — including the ones that mean
/// "some host exists" (`attached`) — must read as not-controllable, so
/// a stale toggle can never appear to work against a process it cannot
/// reach.
library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/rust/api/events.dart';
import 'package:upeg/src/state/host_state_provider.dart';
import 'package:upeg/src/state/pause_control_provider.dart';

void main() {
  group('isPauseControllable', () {
    test('isPauseControllable은_embedded에서만_참이다', () {
      const cases = <(HostStateEvent, bool)>[
        (HostStateEvent.embedded(endpoint: 'unix:/tmp/embed'), true),
        (HostStateEvent.attached(endpoint: 'unix:/tmp/daemon'), false),
        (HostStateEvent.noHost(), false),
        (HostStateEvent.failed(reason: 'boom'), false),
      ];

      for (final (state, expected) in cases) {
        expect(
          isPauseControllable(state),
          expected,
          reason: '$state must map to $expected',
        );
      }
    });
  });

  group('pauseControllableProvider', () {
    test('pauseControllableProvider는_embedded_상태에서_참이다', () async {
      final controller = StreamController<HostStateEvent>();
      addTearDown(controller.close);

      final container = ProviderContainer(
        overrides: [
          hostStateStreamProvider.overrideWith((ref) => controller.stream),
        ],
      );
      addTearDown(container.dispose);

      final sub = container.listen<bool>(pauseControllableProvider, (_, _) {});
      addTearDown(sub.close);

      controller.add(const HostStateEvent.embedded(endpoint: 'unix:/tmp/e'));
      await Future<void>.delayed(Duration.zero);

      expect(container.read(pauseControllableProvider), isTrue);
    });

    test('pauseControllableProvider는_로딩_중이거나_attached면_거짓이다', () async {
      final controller = StreamController<HostStateEvent>();
      addTearDown(controller.close);

      final container = ProviderContainer(
        overrides: [
          hostStateStreamProvider.overrideWith((ref) => controller.stream),
        ],
      );
      addTearDown(container.dispose);

      final sub = container.listen<bool>(pauseControllableProvider, (_, _) {});
      addTearDown(sub.close);

      // No event has arrived yet — the provider must default to false
      // rather than assume controllability.
      expect(container.read(pauseControllableProvider), isFalse);

      controller.add(const HostStateEvent.attached(endpoint: 'unix:/tmp/d'));
      await Future<void>.delayed(Duration.zero);

      expect(container.read(pauseControllableProvider), isFalse);
    });
  });
}
