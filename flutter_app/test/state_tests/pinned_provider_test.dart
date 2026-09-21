/// Unit tests for [PinnedNotifier] + [pinnedProvider].
///
/// The pinned state is a binary flag (`bool`) that gates the popup
/// window's auto-hide behaviour: when true, `PopupAutoHideObserver`
/// MUST NOT call `windowManager.hide()` on focus loss. The provider is
/// the single source of truth — the tray TogglePin callback writes to
/// it and the focus-loss observer reads from it.
///
/// Type system note: `bool` is acceptable here because the state is
/// genuinely binary (pinned / unpinned). No need to wrap it in a
/// newtype — there are no other domain values to conflate it with.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/state/pinned_provider.dart';

void main() {
  group('PinnedNotifier', () {
    test('PinnedNotifier_initial_value_is_false', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);
      expect(container.read(pinnedProvider), isFalse);
    });

    test('PinnedNotifier_toggle_flips_state', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      container.read(pinnedProvider.notifier).toggle();
      expect(container.read(pinnedProvider), isTrue);

      container.read(pinnedProvider.notifier).toggle();
      expect(container.read(pinnedProvider), isFalse);
    });

    test('PinnedNotifier_set_applies_explicit_value', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      container.read(pinnedProvider.notifier).set(true);
      expect(container.read(pinnedProvider), isTrue);

      container.read(pinnedProvider.notifier).set(false);
      expect(container.read(pinnedProvider), isFalse);
    });
  });
}
