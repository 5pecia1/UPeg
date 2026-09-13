/// Non-web stub for the host-attach key/value backing store.
///
/// Selected by Dart's conditional-import lookup on every target where
/// `dart.library.html` is false (desktop / test VM). Host-attach is a
/// wasm/PWA-only concern, so the desktop build has nothing to persist —
/// reads report "absent" and writes are no-ops.
library;

String? readHostAttachValue(String key) => null;

void writeHostAttachValue(String key, String value) {
  // No persistent web storage off the web target; intentionally empty.
}
