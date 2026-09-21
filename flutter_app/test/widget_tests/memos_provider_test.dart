import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/features/memos/memos_provider.dart';
import 'package:upeg/src/rust/api/memos.dart' show MemoEntry;

ProviderContainer _container({
  List<MemoEntry> seed = const <MemoEntry>[],
  void Function(List<MemoEntry> entries)? onSave,
}) {
  final container = ProviderContainer(
    overrides: [
      memosLoaderProvider.overrideWithValue(() => seed),
      memosSaverProvider.overrideWithValue(onSave ?? (_) {}),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

void main() {
  group('MemosNotifier', () {
    test('build_loads_existing_memos_from_the_store', () {
      final container = _container(
        seed: const [MemoEntry(key: 'a', body: 'one')],
      );
      expect(container.read(memosProvider), hasLength(1));
      expect(container.read(memosProvider).single.body, 'one');
    });

    test('create_appends_a_new_empty_memo_and_saves', () {
      final saved = <List<MemoEntry>>[];
      final container = _container(onSave: saved.add);

      final key = container.read(memosProvider.notifier).create();

      expect(container.read(memosProvider), hasLength(1));
      expect(container.read(memosProvider.notifier).byKey(key)?.body, '');
      expect(saved.last, hasLength(1));
    });

    test('calling_create_twice_produces_distinct_keys', () {
      final container = _container();
      final notifier = container.read(memosProvider.notifier);
      final first = notifier.create();
      final second = notifier.create();
      expect(first, isNot(second));
      expect(container.read(memosProvider), hasLength(2));
    });

    test('updatebody_lazily_creates_a_missing_key', () {
      final saved = <List<MemoEntry>>[];
      final container = _container(onSave: saved.add);

      container
          .read(memosProvider.notifier)
          .updateBody(scratchMemoKey, 'scratch text');

      expect(
        container.read(memosProvider.notifier).byKey(scratchMemoKey)?.body,
        'scratch text',
      );
      expect(saved.last.single.key, scratchMemoKey);
    });

    test('updatebody_replaces_an_existing_memo_body', () {
      final container = _container(
        seed: const [MemoEntry(key: scratchMemoKey, body: 'old')],
      );
      container.read(memosProvider.notifier).updateBody(scratchMemoKey, 'new');

      expect(container.read(memosProvider), hasLength(1));
      expect(container.read(memosProvider).single.body, 'new');
    });
  });
}
