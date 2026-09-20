/// Unit tests for [`toolsProvider`] and the [`toolByIdProvider`] family.
///
/// The FRB call is swapped through [toolsLoaderProvider] so the native
/// dylib is never touched. The family lookup is the load-bearing
/// behavior: callers expect `null` for unknown ids so they can render
/// a placeholder without throwing.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/tools_provider.dart';

import '../test_helpers/tool_fixture.dart';

final ToolDto _hex = fixtureToolDto(
  id: 'num.hex_to_decimal',
  toolkit: 'convert',
  label: 'hex → dec',
  tags: const <String>['convert'],
);

final ToolDto _uuid = fixtureToolDto(
  id: 'id.uuid_v7',
  toolkit: 'id',
  label: 'uuid v7',
  tags: const <String>['id'],
);

void main() {
  group('toolsProvider', () {
    test('toolsProvider_returns_loader_result_verbatim', () async {
      final container = ProviderContainer(
        overrides: [
          toolsLoaderProvider.overrideWith(
            (ref) =>
                () => <ToolDto>[_hex, _uuid],
          ),
        ],
      );
      addTearDown(container.dispose);

      final tools = await container.read(toolsProvider.future);
      expect(tools.map((t) => t.id), ['num.hex_to_decimal', 'id.uuid_v7']);
    });
  });

  group('toolByIdProvider', () {
    test('toolByIdProvider_returns_matching_id', () async {
      final container = ProviderContainer(
        overrides: [
          toolsLoaderProvider.overrideWith(
            (ref) =>
                () => <ToolDto>[_hex, _uuid],
          ),
        ],
      );
      addTearDown(container.dispose);

      // Wait for toolsProvider to resolve so the family lookup sees a
      // populated `AsyncData`.
      await container.read(toolsProvider.future);

      expect(
        container.read(toolByIdProvider(ToolId.parse('id.uuid_v7')))?.id,
        'id.uuid_v7',
      );
    });

    test('toolByIdProvider_returns_null_for_missing_id', () async {
      final container = ProviderContainer(
        overrides: [
          toolsLoaderProvider.overrideWith(
            (ref) =>
                () => <ToolDto>[_hex],
          ),
        ],
      );
      addTearDown(container.dispose);

      await container.read(toolsProvider.future);

      expect(
        container.read(toolByIdProvider(ToolId.parse('nonexistent.tool'))),
        isNull,
      );
    });
  });
}
