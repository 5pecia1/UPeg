/// Unit tests for [`hostStateProvider`].
///
/// The provider is a thin Riverpod bridge over [hostStateStream]; the
/// load-bearing behavior is that emitted events surface as
/// `AsyncValue.data`. We swap [hostStateStreamProvider] to a controlled
/// stream so the FRB call never fires.
library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/rust/api/events.dart';
import 'package:upeg/src/state/host_state_provider.dart';

void main() {
  group('hostStateProvider', () {
    test('hostStateProvider_exposes_stream_events_as_AsyncData', () async {
      final controller = StreamController<HostStateEvent>();
      addTearDown(controller.close);

      final container = ProviderContainer(
        overrides: [
          hostStateStreamProvider.overrideWith((ref) => controller.stream),
        ],
      );
      addTearDown(container.dispose);

      // Listen before emitting so the StreamProvider subscribes.
      final sub = container.listen<AsyncValue<HostStateEvent>>(
        hostStateProvider,
        (_, _) {},
      );
      addTearDown(sub.close);

      controller.add(const HostStateEvent.attached(endpoint: 'unix:/tmp/sock'));
      await Future<void>.delayed(Duration.zero);

      final value = container.read(hostStateProvider);
      expect(value.value, isA<HostStateEvent_Attached>());
      expect(
        (value.value! as HostStateEvent_Attached).endpoint,
        'unix:/tmp/sock',
      );
    });

    test('hostStateProvider_forwards_noHost_events_too', () async {
      final controller = StreamController<HostStateEvent>();
      addTearDown(controller.close);

      final container = ProviderContainer(
        overrides: [
          hostStateStreamProvider.overrideWith((ref) => controller.stream),
        ],
      );
      addTearDown(container.dispose);

      final sub = container.listen<AsyncValue<HostStateEvent>>(
        hostStateProvider,
        (_, _) {},
      );
      addTearDown(sub.close);

      controller.add(const HostStateEvent.noHost());
      await Future<void>.delayed(Duration.zero);

      expect(
        container.read(hostStateProvider).value,
        isA<HostStateEvent_NoHost>(),
      );
    });
  });
}
