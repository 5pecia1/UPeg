/// Unit tests for the palette query + results providers.
///
/// `paletteSearcherProvider` is overridden so the FRB call is not
/// invoked. We drive `paletteQueryProvider` and observe what
/// `paletteResultsProvider` resolves to.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/rust/api/palette.dart';
import 'package:upeg/src/rust/api/tools.dart' show PinKindDto;
import 'package:upeg/src/state/app_state.dart';

const _hit = PaletteHit(
  id: 'num.hex_to_decimal',
  label: 'Hex → Dec',
  description: '',
  score: 1.0,
  pinKind: PinKindDto.inline,
);

ProviderContainer _container({
  List<PaletteHit> hits = const <PaletteHit>[_hit],
  void Function(String query)? onSearch,
}) {
  return ProviderContainer(
    overrides: [
      paletteSearcherProvider.overrideWith(
        (ref) => (q) {
          onSearch?.call(q);
          return hits;
        },
      ),
    ],
  );
}

void main() {
  group('paletteProvider', () {
    test('paletteProvider_passes_even_empty_query_to_search_loader', () async {
      String? observed;
      final container = _container(onSearch: (query) => observed = query);
      addTearDown(container.dispose);

      // Default query is the empty string.
      final result = await container.read(paletteResultsProvider.future);
      expect(observed, '');
      expect(result, hasLength(1));
    });

    test(
      'paletteProvider_passes_whitespace_only_query_to_search_loader',
      () async {
        String? observed;
        final container = _container(onSearch: (query) => observed = query);
        addTearDown(container.dispose);

        container.read(paletteQueryProvider.notifier).state = '   ';
        final result = await container.read(paletteResultsProvider.future);
        expect(observed, '   ');
        expect(result, hasLength(1));
      },
    );

    test('paletteProvider_forwards_query_to_FRB', () async {
      String? observed;
      final container = _container(onSearch: (q) => observed = q);
      addTearDown(container.dispose);

      container.read(paletteQueryProvider.notifier).state = 'hex';
      final result = await container.read(paletteResultsProvider.future);

      expect(observed, 'hex');
      expect(result, hasLength(1));
      expect(result.first.id, 'num.hex_to_decimal');
    });
  });
}
