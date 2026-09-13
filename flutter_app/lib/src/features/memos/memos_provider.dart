/// Memos feature store.
///
/// Wraps the Rust `loadMemos` / `saveMemos` FRB surface behind Riverpod
/// so the on-board notepad pin (`memo.scratch`) can display and edit
/// persisted memos, and the create action (`memo.create`, Cmd+Shift+N)
/// can append a fresh memo. Widget tests override the loader/saver seams
/// to stay off the Rust dylib.
///
/// SoC: this notifier owns the memo list + persistence. Which memo the
/// notepad currently shows is a separate concern
/// ([activeMemoKeyProvider]).
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/rust/api/memos.dart';

/// Stable key of the default on-board scratch memo. Editing the notepad
/// before any explicit `memo.create` lands on this entry, so a fresh
/// install still has one persistent scratch pad.
const String scratchMemoKey = 'scratch';

/// Prefix for keys minted by [MemosNotifier.create].
const String _createdMemoKeyPrefix = 'memo-';

typedef MemosLoader = List<MemoEntry> Function();
typedef MemosSaver = void Function(List<MemoEntry> entries);

final memosLoaderProvider = Provider<MemosLoader>((ref) => loadMemos);

final memosSaverProvider = Provider<MemosSaver>(
  (ref) =>
      (entries) => saveMemos(entries: entries),
);

/// The persisted memo list, newest-append order. Backed by the FRB
/// store via the loader/saver seams.
final memosProvider = NotifierProvider<MemosNotifier, List<MemoEntry>>(
  MemosNotifier.new,
);

/// Key of the memo the on-board notepad currently shows/edits. Defaults
/// to [scratchMemoKey]; `memo.create` points it at the freshly-created
/// memo so the new (empty) memo is what the user types into.
final activeMemoKeyProvider = NotifierProvider<ActiveMemoKeyNotifier, String>(
  ActiveMemoKeyNotifier.new,
);

class ActiveMemoKeyNotifier extends Notifier<String> {
  @override
  String build() => scratchMemoKey;

  void setActive(String key) => state = key;
}

class MemosNotifier extends Notifier<List<MemoEntry>> {
  @override
  List<MemoEntry> build() => ref.read(memosLoaderProvider)();

  /// Look up a memo by key, or `null` when absent.
  MemoEntry? byKey(String key) {
    for (final entry in state) {
      if (entry.key == key) return entry;
    }
    return null;
  }

  /// Create a new, empty memo and persist it. Returns the new memo's key
  /// so callers can focus/activate it.
  String create() {
    final key = _freshKey();
    state = <MemoEntry>[...state, MemoEntry(key: key, body: '')];
    _persist();
    return key;
  }

  /// Set the body of the memo [key], creating the entry if it does not
  /// exist yet (the scratch memo is lazily materialised on first edit).
  void updateBody(String key, String body) {
    if (byKey(key) == null) {
      state = <MemoEntry>[...state, MemoEntry(key: key, body: body)];
    } else {
      state = <MemoEntry>[
        for (final entry in state)
          if (entry.key == key) MemoEntry(key: key, body: body) else entry,
      ];
    }
    _persist();
  }

  void _persist() => ref.read(memosSaverProvider)(state);

  String _freshKey() {
    var maxIndex = 0;
    for (final entry in state) {
      if (!entry.key.startsWith(_createdMemoKeyPrefix)) continue;
      final suffix = entry.key.substring(_createdMemoKeyPrefix.length);
      final parsed = int.tryParse(suffix);
      if (parsed != null && parsed > maxIndex) maxIndex = parsed;
    }
    return '$_createdMemoKeyPrefix${maxIndex + 1}';
  }
}
