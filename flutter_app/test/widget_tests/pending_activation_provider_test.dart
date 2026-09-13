/// Unit tests for [pendingActivationProvider].
///
/// The popup writes the pending tool id on cell tap; the board page
/// reads + clears it once it mounts. The provider is a tiny
/// `NotifierProvider<ToolId?>` — pure state container, no FRB calls,
/// no async work. Tests live in `widget_tests/` to share the Riverpod
/// test container fixture pattern with adjacent files.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/state/pending_activation_provider.dart';

void main() {
  group('PendingActivationNotifier', () {
    test('초기값은_null이다', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      expect(container.read(pendingActivationProvider), isNull);
    });

    test('set는_toolId를_저장한다', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      final toolId = ToolId.parse('num.hex_to_decimal');
      container.read(pendingActivationProvider.notifier).set(toolId);

      expect(container.read(pendingActivationProvider), toolId);
    });

    test('clear는_상태를_null로_되돌린다', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      final notifier = container.read(pendingActivationProvider.notifier);
      final toolId = ToolId.parse('id.uuid_v7');
      notifier.set(toolId);
      expect(container.read(pendingActivationProvider), toolId);

      notifier.clear();
      expect(container.read(pendingActivationProvider), isNull);
    });
  });
}
