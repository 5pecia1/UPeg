import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/features/memos/memos_provider.dart';
import 'package:upeg/src/identity.dart';
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

  test('active_memo_selection_is_independent_for_each_pin', () {
    final container = _container();
    final first = (BoardKey.parse('dev'), PinId.parse('memo-first'));
    final second = (BoardKey.parse('dev'), PinId.parse('memo-second'));

    container.read(activeMemoKeyProvider(first).notifier).setActive('memo-1');
    container.read(activeMemoKeyProvider(second).notifier).setActive('memo-2');

    expect(container.read(activeMemoKeyProvider(first)), 'memo-1');
    expect(container.read(activeMemoKeyProvider(second)), 'memo-2');
  });

  test('new_pin_defaults_are_distinct_and_restore_their_saved_bodies', () {
    final first = (BoardKey.parse('dev'), PinId.parse('memo-first'));
    final second = (BoardKey.parse('dev'), PinId.parse('memo-second'));
    final firstKey = defaultMemoKeyForPin(first);
    final secondKey = defaultMemoKeyForPin(second);
    final saved = <List<MemoEntry>>[];
    final writer = _container(onSave: saved.add);

    expect(writer.read(activeMemoKeyProvider(first)), firstKey);
    expect(writer.read(activeMemoKeyProvider(second)), secondKey);
    writer.read(memosProvider.notifier).updateBody(firstKey, 'first body');
    writer.read(memosProvider.notifier).updateBody(secondKey, 'second body');

    final restored = _container(seed: saved.last);
    expect(restored.read(activeMemoKeyProvider(first)), firstKey);
    expect(restored.read(activeMemoKeyProvider(second)), secondKey);
    expect(
      restored.read(memosProvider.notifier).byKey(firstKey)?.body,
      'first body',
    );
    expect(
      restored.read(memosProvider.notifier).byKey(secondKey)?.body,
      'second body',
    );
  });

  test('legacy_memo_scratch_pin_keeps_the_existing_scratch_key', () {
    final legacy = (BoardKey.parse('dev'), PinId.parse('memo.scratch'));
    final container = _container();

    expect(container.read(activeMemoKeyProvider(legacy)), scratchMemoKey);
  });
}
