/// Unit tests for [PopupSelectionNotifier].
///
/// The popup's selection cursor is a single typed index into the
/// effective hit list. Tests live in `widget_tests/` so the Riverpod
/// container fixture pattern stays adjacent to the popup-key-event
/// unit tests; the provider itself doesn't depend on any widget.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/popup/popup_selection.dart';
import 'package:upeg/src/popup/popup_selection_provider.dart';

void main() {
  group('PopupSelectionNotifier', () {
    test('the_initial_value_is_index_0', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      expect(
        container.read(popupSelectionProvider),
        const PopupSelection(index: 0),
      );
    });

    test('next_wraps_at_the_upper_bound', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      final notifier = container.read(popupSelectionProvider.notifier);
      notifier.resetTo(5);
      notifier.next(hitCount: 6);

      expect(container.read(popupSelectionProvider).index, 0);
    });

    test('next_increments_the_index_in_the_middle', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      final notifier = container.read(popupSelectionProvider.notifier);
      notifier.next(hitCount: 4);

      expect(container.read(popupSelectionProvider).index, 1);
    });

    test('prev_wraps_from_the_lower_to_the_upper_bound', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      final notifier = container.read(popupSelectionProvider.notifier);
      notifier.prev(hitCount: 4);

      expect(container.read(popupSelectionProvider).index, 3);
    });

    test('the_index_stays_put_when_hitcount_is_0', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      final notifier = container.read(popupSelectionProvider.notifier);
      notifier.next(hitCount: 0);
      notifier.prev(hitCount: 0);

      expect(container.read(popupSelectionProvider).index, 0);
    });
  });
}
