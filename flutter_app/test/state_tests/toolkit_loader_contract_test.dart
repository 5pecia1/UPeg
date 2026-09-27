import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/features/toolkits/toolkit_loader_contract.dart';

const _digest =
    'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';

void main() {
  test('only_web_toolkit_tools_are_routable_to_the_worker', () {
    final catalog = ToolkitCatalogDescriptor.fromJson(<String, Object?>{
      'catalog_digest': _digest,
      'toolkits': <Object?>[
        <String, Object?>{
          'id': 'text',
          'web': <String, Object?>{'wasm': '/toolkits/text.wasm'},
          'tools': <Object?>[
            <String, Object?>{'id': 'text.trim'},
          ],
        },
        <String, Object?>{
          'id': 'shell',
          'web': null,
          'tools': <Object?>[
            <String, Object?>{'id': 'shell.rg'},
          ],
        },
      ],
    });

    expect(catalog.toolkitByToolId, <String, String>{'text.trim': 'text'});
  });

  test('rejects_a_catalog_without_a_digest_or_toolkit_list', () {
    expect(
      () => ToolkitCatalogDescriptor.fromJson(const <String, Object?>{}),
      throwsFormatException,
    );
  });
}
