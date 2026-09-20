/// Unit tests for [`currentBoardKeyProvider`].
///
/// The provider was a plain `StateProvider<String?>` before Batch J;
/// it is now a typed `NotifierProvider<CurrentBoardNotifier, BoardKey?>`
/// whose load-bearing behavior is the `null` default and the
/// `select` / `clear` mutation path. Persistence-specific
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/state/current_board_provider.dart';

import '../test_helpers/pegboard_selection_overrides.dart';

ProviderContainer _makeContainer() {
  final container = ProviderContainer(overrides: pegboardSelectionOverrides());
  addTearDown(container.dispose);
  return container;
}

void main() {
  group('currentBoardKeyProvider', () {
    test('currentBoardKeyProvider_initial_value_is_null', () {
      final container = _makeContainer();

      expect(container.read(currentBoardKeyProvider), isNull);
    });

    test('currentBoardKeyProvider_updates_value_via_select', () {
      final container = _makeContainer();

      container
          .read(currentBoardKeyProvider.notifier)
          .select(BoardKey.parse('dev'));
      expect(container.read(currentBoardKeyProvider), BoardKey.parse('dev'));

      container.read(currentBoardKeyProvider.notifier).clear();
      expect(container.read(currentBoardKeyProvider), isNull);
    });
  });
}
