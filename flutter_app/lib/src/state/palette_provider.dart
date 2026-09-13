/// Palette query state + FRB-backed search results.
///
/// `paletteQueryProvider` holds the live text input; `paletteResultsProvider`
/// reacts to it and returns matches from `palette.searchTools`. Empty query is
/// intentionally forwarded to Rust so the palette can browse and pin tools
/// before the user types.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/legacy.dart';
import 'package:upeg/src/rust/api/palette.dart';

typedef PaletteSearcher = List<PaletteHit> Function(String query);

final paletteSearcherProvider = Provider<PaletteSearcher>(
  (ref) =>
      (query) => searchTools(query: query),
);

final paletteQueryProvider = StateProvider<String>((ref) => '');

final paletteResultsProvider = FutureProvider<List<PaletteHit>>((ref) async {
  final String query = ref.watch(paletteQueryProvider);
  final search = ref.watch(paletteSearcherProvider);
  return search(query);
});
