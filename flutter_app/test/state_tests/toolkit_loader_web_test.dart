@TestOn('browser')
library;

import 'dart:js_interop';
import 'dart:js_interop_unsafe';

import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/features/toolkits/toolkit_loader_web.dart';

void main() {
  test(
    'should read a nested JS catalog when syncing the browser client',
    () async {
      final originalLoader = globalContext['UpegToolkitLoader'];
      addTearDown(() => globalContext['UpegToolkitLoader'] = originalLoader);

      final catalog = <String, Object?>{
        'catalog_digest': 'a' * 64,
        'toolkits': <Object?>[
          <String, Object?>{
            'id': 'num',
            'web': <String, Object?>{'wasm': '/toolkits/num.wasm'},
            'tools': <Object?>[
              <String, Object?>{'id': 'num.hex_to_decimal'},
            ],
          },
        ],
      };
      final response = <String, Object?>{'catalog': catalog}.jsify();
      final loader = <String, Object?>{}.jsify()! as JSObject;
      loader['sync'] = ((JSAny? _) => Future<JSAny?>.value(response).toJS).toJS;
      globalContext['UpegToolkitLoader'] = loader;

      final synced = await const BrowserToolkitWorkerClient().sync(
        catalogUrl: '/toolkits/catalog.json',
        abiDigest: 'b' * 64,
      );

      expect(synced.digest, 'a' * 64);
      expect(synced.toolkitByToolId, {'num.hex_to_decimal': 'num'});
    },
  );
}
