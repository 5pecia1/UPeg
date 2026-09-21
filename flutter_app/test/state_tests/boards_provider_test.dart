/// Unit tests for [`boardsProvider`].
///
/// Overrides [boardsLoaderProvider] so the FRB call never loads the
/// native dylib. Two scenarios mirror the only meaningful contract:
///   1) the FutureProvider awaits the loader output verbatim,
///   2) loader exceptions surface through `AsyncValue.error` instead of
///      crashing the provider scope.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/state/boards_provider.dart';

const BoardDto _board = BoardDto(key: 'dev', title: 'Dev');

void main() {
  group('boardsProvider', () {
    test('boardsProvider_returns_loader_result_verbatim', () async {
      final container = ProviderContainer(
        overrides: [
          boardsLoaderProvider.overrideWith(
            (ref) =>
                () => const <BoardDto>[_board],
          ),
        ],
      );
      addTearDown(container.dispose);

      final result = await container.read(boardsProvider.future);
      expect(result, hasLength(1));
      expect(result.single.key, 'dev');
    });

    test('boardsProvider_propagates_loader_exception_as_AsyncError', () async {
      final container = ProviderContainer(
        overrides: [
          boardsLoaderProvider.overrideWith(
            (ref) =>
                () => throw const FormatException('boom'),
          ),
        ],
      );
      addTearDown(container.dispose);

      // Subscribe so the provider mounts; an unconsumed FutureProvider
      // that throws synchronously throws on dispose during loading.
      final sub = container.listen<AsyncValue<List<BoardDto>>>(
        boardsProvider,
        (_, _) {},
      );
      addTearDown(sub.close);

      // Settle a microtask so the provider records the sync error.
      await Future<void>.delayed(Duration.zero);

      final value = container.read(boardsProvider);
      expect(value.hasError, isTrue);
      expect(value.error, isA<FormatException>());
    });
  });
}
