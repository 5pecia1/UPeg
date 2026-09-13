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
    test('build는_저장소에서_기존_메모를_적재한다', () {
      final container = _container(
        seed: const [MemoEntry(key: 'a', body: 'one')],
      );
      expect(container.read(memosProvider), hasLength(1));
      expect(container.read(memosProvider).single.body, 'one');
    });

    test('create는_새_빈_메모를_추가하고_저장한다', () {
      final saved = <List<MemoEntry>>[];
      final container = _container(onSave: saved.add);

      final key = container.read(memosProvider.notifier).create();

      expect(container.read(memosProvider), hasLength(1));
      expect(container.read(memosProvider.notifier).byKey(key)?.body, '');
      expect(saved.last, hasLength(1));
    });

    test('create를_두번_하면_서로_다른_key를_만든다', () {
      final container = _container();
      final notifier = container.read(memosProvider.notifier);
      final first = notifier.create();
      final second = notifier.create();
      expect(first, isNot(second));
      expect(container.read(memosProvider), hasLength(2));
    });

    test('updateBody는_없는_key를_지연_생성한다', () {
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

    test('updateBody는_기존_메모_본문을_교체한다', () {
      final container = _container(
        seed: const [MemoEntry(key: scratchMemoKey, body: 'old')],
      );
      container.read(memosProvider.notifier).updateBody(scratchMemoKey, 'new');

      expect(container.read(memosProvider), hasLength(1));
      expect(container.read(memosProvider).single.body, 'new');
    });
  });
}
