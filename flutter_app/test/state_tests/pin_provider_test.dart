/// Unit tests for the pin-state providers.
///
/// FRB calls are swapped via [isPinnedLoaderProvider] /
/// [pinToolMutatorProvider] / [unpinToolMutatorProvider] so the native dylib
/// never loads. Mutation invalidation is covered by
/// `pegboard_mutations_provider_test.dart`.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/state/pin_provider.dart';

void main() {
  group('pinnedProvider', () {
    test('pinnedProvider_returns_loader_result_verbatim', () {
      final container = ProviderContainer(
        overrides: [
          isPinnedLoaderProvider.overrideWith(
            (ref) =>
                (b, t) =>
                    b == BoardKey.parse('dev') &&
                    t == ToolId.parse('num.hex_to_decimal'),
          ),
        ],
      );
      addTearDown(container.dispose);

      expect(
        container.read(
          pinnedProvider((
            BoardKey.parse('dev'),
            ToolId.parse('num.hex_to_decimal'),
          )),
        ),
        isTrue,
      );
      expect(
        container.read(
          pinnedProvider((BoardKey.parse('dev'), ToolId.parse('id.uuid_v7'))),
        ),
        isFalse,
      );
    });
  });
}
