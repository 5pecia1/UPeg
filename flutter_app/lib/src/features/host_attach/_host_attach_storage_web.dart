/// Web impl for the host-attach key/value backing store.
///
/// Loaded by Dart's conditional-import lookup when `dart.library.html`
/// is true (web / wasm builds). Persists the pairing in the browser's
/// `localStorage` — the same synchronous substrate the memo store uses
/// on the web target — so the config is available synchronously during
/// the first `build` of the board.
library;

import 'package:web/web.dart' as web;

String? readHostAttachValue(String key) => web.window.localStorage.getItem(key);

void writeHostAttachValue(String key, String value) {
  web.window.localStorage.setItem(key, value);
}
