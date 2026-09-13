/// Empty-board tool affinity suggestions.
///
/// Wraps `pegboard.suggestionsForEmptyBoard(boardKey)` in a
/// `Provider.family` so the empty card can render the same three
/// ghost tiles every time without re-crossing FFI on a passive
/// rebuild. Pinning a suggestion invalidates the matching
/// `layoutProvider` entry (handled by `pin_provider.dart`).
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart' as frb;
import 'package:upeg/src/rust/api/tools.dart';

typedef SuggestionsLoader = List<ToolDto> Function(BoardKey boardKey);

final suggestionsLoaderProvider = Provider<SuggestionsLoader>(
  (ref) =>
      (boardKey) => frb.suggestionsForEmptyBoard(boardKey: boardKey.value),
);

final emptyBoardSuggestionsProvider = Provider.family<List<ToolDto>, BoardKey>((
  ref,
  boardKey,
) {
  final load = ref.watch(suggestionsLoaderProvider);
  return load(boardKey);
});
