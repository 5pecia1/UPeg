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
    test('초기값은_index_0이다', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      expect(
        container.read(popupSelectionProvider),
        const PopupSelection(index: 0),
      );
    });

    test('next는_상한에서_랩한다', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      final notifier = container.read(popupSelectionProvider.notifier);
      notifier.resetTo(5);
      notifier.next(hitCount: 6);

      expect(container.read(popupSelectionProvider).index, 0);
    });

    test('next는_중간에서_index를_증가시킨다', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      final notifier = container.read(popupSelectionProvider.notifier);
      notifier.next(hitCount: 4);

      expect(container.read(popupSelectionProvider).index, 1);
    });

    test('prev는_하한에서_상한으로_랩한다', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      final notifier = container.read(popupSelectionProvider.notifier);
      notifier.prev(hitCount: 4);

      expect(container.read(popupSelectionProvider).index, 3);
    });

    test('hitCount_0에서는_index가_바뀌지_않는다', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      final notifier = container.read(popupSelectionProvider.notifier);
      notifier.next(hitCount: 0);
      notifier.prev(hitCount: 0);

      expect(container.read(popupSelectionProvider).index, 0);
    });
  });
}
