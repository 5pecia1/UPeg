/// Tag filter providers.
///
/// `tagOptionsProvider` exposes the global Rust-side tag set wrapped in the
/// [`TagSelection`] sealed class. Board surfaces should use
/// [`tagOptionsForBoardProvider`] so the chip row reflects tags from pins on
/// the current board, matching the TUI pegboard contract.
///
/// Durable board/tag selection is shared through
/// `pegboardSelectionProvider`; this file keeps the tag-focused public
/// provider surface and owns tag option/count adapters.
///
/// Type system (Batch W): `kAllTag` magic-string sentinel is gone.
/// The sealed [`TagSelection`] makes the type system distinguish
/// "no filter" (TagAll) from "filter by a tag literally named 'all'"
/// (TagSpecific('all')). Only the FRB-boundary wrappers below ever
/// touch the underlying `'all'` string.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/state/boards_provider.dart';
import 'package:upeg/src/state/pegboard_selection_provider.dart';
import 'package:upeg/src/state/tag_selection.dart';

export 'package:upeg/src/state/tag_selection.dart'
    show TagSelection, TagAll, TagSpecific;

typedef TagOptionsLoader = List<String> Function();

final tagOptionsLoaderProvider = Provider<TagOptionsLoader>(
  (ref) => tagOptions,
);

/// Wraps Rust `tag_options()` so the rest of the codebase consumes
/// a typed list. The first element of the Rust response is the
/// `"all"` sentinel — we map it to [`TagAll`]; everything else
/// becomes [`TagSpecific`].
final tagOptionsProvider = Provider<List<TagSelection>>((ref) {
  final load = ref.watch(tagOptionsLoaderProvider);
  final raw = load();
  return [
    for (final tag in raw) TagSelection.fromPersisted(_unwrapSentinel(tag)),
  ];
});

/// Hides the FRB sentinel inside this file. The Rust API uses
/// `"all"` to mean "everything" — Dart code switches on the typed
/// variant instead, so this is the only translation point.
String? _unwrapSentinel(String raw) =>
    raw == const TagAll().frbValue ? null : raw;

/// Synchronous loader for `count_for_tag` — wraps the FRB
/// `countForTag` call so widget tests can `overrideWith` an
/// in-memory map without loading the Rust dylib.
typedef TagCountLoader = int Function(TagSelection sel);

final tagCountLoaderProvider = Provider<TagCountLoader>(
  (ref) =>
      (sel) => countForTag(tag: sel.frbValue),
);

/// Family-of-int: tool count for the requested selection. The widget
/// tree reads this through `ref.watch(countForTagProvider(sel))`.
/// Backed by [`tagCountLoaderProvider`] so tests can pin deterministic
/// counts.
final countForTagProvider = Provider.family<int, TagSelection>((ref, sel) {
  final load = ref.watch(tagCountLoaderProvider);
  return load(sel);
});

typedef BoardTagOptionsLoader = List<String> Function(BoardKey? boardKey);

final boardTagOptionsLoaderProvider = Provider<BoardTagOptionsLoader>(
  (ref) =>
      (boardKey) => tagOptionsForBoard(boardKey: boardKey?.value),
);

/// Board-scoped tag options based on pinned placements, not every tool in the
/// global toolbox. `null` board means a union across all boards.
final tagOptionsForBoardProvider =
    Provider.family<List<TagSelection>, BoardKey?>((ref, boardKey) {
      final load = ref.watch(boardTagOptionsLoaderProvider);
      final raw = load(boardKey);
      return [
        for (final tag in raw) TagSelection.fromPersisted(_unwrapSentinel(tag)),
      ];
    });

typedef BoardTagCountLoader =
    int Function(BoardKey? boardKey, TagSelection selection);

final boardTagCountLoaderProvider = Provider<BoardTagCountLoader>(
  (ref) =>
      (boardKey, selection) =>
          countPinnedForTag(boardKey: boardKey?.value, tag: selection.frbValue),
);

final countForBoardTagProvider =
    Provider.family<int, (BoardKey?, TagSelection)>((ref, key) {
      final load = ref.watch(boardTagCountLoaderProvider);
      return load(key.$1, key.$2);
    });

class SelectedTagNotifier extends Notifier<TagSelection> {
  @override
  TagSelection build() {
    return ref.watch(
      pegboardSelectionProvider.select((selection) => selection.tag),
    );
  }

  /// Select [`tag`] (a specific tag string) and persist it. The
  /// store write is fire-and-forget so the UI thread is never
  /// blocked. Pass a [`TagSelection`] to [`setSelection`] for the
  /// generic case.
  void select(String tag) {
    setSelection(TagSpecific(tag));
  }

  /// Return to [`TagAll`] and persist `null` (distinct from "no
  /// preference recorded yet"). Used by widgets that expose a
  /// dedicated "show all" affordance.
  void clear() {
    setSelection(const TagAll());
  }

  /// Generic setter — switches the state to [`selection`] and
  /// persists `selection.persistValue`. The typed surface that
  /// `select` / `clear` thin-wrap.
  void setSelection(TagSelection selection) {
    ref.read(pegboardSelectionProvider.notifier).setTag(selection);
  }

  Future<void> restore() async {
    final boards = await ref.read(boardsProvider.future);
    await ref.read(pegboardSelectionProvider.notifier).restore(boards);
  }
}

final selectedTagProvider = NotifierProvider<SelectedTagNotifier, TagSelection>(
  SelectedTagNotifier.new,
);
