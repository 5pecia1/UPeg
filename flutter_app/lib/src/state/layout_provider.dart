/// Per-board layout snapshot.
///
/// `FutureProvider.family` keyed by [LayoutQuery]. Board rendering includes
/// the active tag filter, while movement/reordering paths can request
/// [LayoutQuery.all] when they need the full board layout.
library;

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/legacy.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/state/tag_selection.dart';

@immutable
class LayoutQuery {
  const LayoutQuery({required this.boardKey, required this.tag});

  const LayoutQuery.all(this.boardKey) : tag = const TagAll();

  final BoardKey boardKey;
  final TagSelection tag;

  String? get tagForFrb => switch (tag) {
    TagAll() => null,
    TagSpecific(:final tag) => tag,
  };

  @override
  bool operator ==(Object other) =>
      other is LayoutQuery && other.boardKey == boardKey && other.tag == tag;

  @override
  int get hashCode => Object.hash(boardKey, tag);
}

typedef LayoutLoader = LayoutSnapshotDto Function(LayoutQuery query);

final layoutRevisionProvider = StateProvider.family<int, BoardKey>(
  (ref, boardKey) => 0,
);

final layoutLoaderProvider = Provider<LayoutLoader>(
  (ref) =>
      (query) => loadLayoutSnapshotForFilter(
        boardKey: query.boardKey.value,
        tag: query.tagForFrb,
      ),
);

final layoutProvider = FutureProvider.family<LayoutSnapshotDto, LayoutQuery>((
  ref,
  query,
) async {
  ref.watch(layoutRevisionProvider(query.boardKey));
  final load = ref.watch(layoutLoaderProvider);
  return load(query);
});

void bumpLayoutRevision(WidgetRef ref, BoardKey boardKey) {
  ref.read(layoutRevisionProvider(boardKey).notifier).state++;
}

void bumpLayoutRevisionRef(Ref ref, BoardKey boardKey) {
  ref.read(layoutRevisionProvider(boardKey).notifier).state++;
}
