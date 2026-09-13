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
    test('hostStateProvider는_스트림_이벤트를_AsyncData로_노출한다', () async {
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

    test('hostStateProvider는_noHost_이벤트도_전달한다', () async {
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
