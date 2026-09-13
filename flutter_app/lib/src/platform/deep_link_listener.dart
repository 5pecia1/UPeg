/// `app_links` subscriber that resolves each incoming `upeg://open?...`
/// URL through the canonical Rust parser (`parseLaunchIntent`) and
/// pushes the parsed [LaunchIntent] into [launchIntentProvider].
///
/// One provider, three sources (Batch E):
///   1. Cold-boot argv (`AppInitReport.launch` → `app.dart` seeds the
///      provider on startup).
///   2. Second-instance `app_links` event (this listener).
///   3. In-process popup encode (popup writes directly via the
///      provider's `set()`).
///
/// All three converge on `LaunchIntentApplier` which drains the
/// provider and runs the matching activation. Keeping the routing
/// policy in one observer (and the URL parser in Rust) means a new
/// source only has to call `notifier.set(...)`.
library;

import 'package:app_links/app_links.dart';
import 'package:flutter/foundation.dart'
    show debugPrint, kIsWeb, visibleForTesting;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/rust/api/deep_link.dart';
import 'package:upeg/src/state/launch_intent_provider.dart';

/// Function shape for the URL → DTO parser. Swappable so widget tests
/// can stub the FRB call without loading the native dylib.
typedef LaunchIntentParser = LaunchIntentDto? Function(String url);

LaunchIntentDto? _defaultParser(String url) => parseLaunchIntent(uri: url);

class DeepLinkListener {
  DeepLinkListener._();

  static DeepLinkListener? _installed;

  /// Install the URL listener. Idempotent: subsequent calls are
  /// no-ops so hot-reload doesn't stack subscriptions.
  static Future<void> install(Ref ref) async {
    if (kIsWeb) {
      // app_links on web reads `window.location` which isn't relevant
      // for the FRB-backed desktop flow yet. Phase 9 wires the web
      // build separately.
      return;
    }
    if (_installed != null) return;
    final listener = DeepLinkListener._();
    final appLinks = AppLinks();
    appLinks.uriLinkStream.listen((uri) {
      _dispatch(ref.read(launchIntentProvider.notifier), uri, _defaultParser);
    });
    _installed = listener;
  }

  /// Drive the URL handler from a test. Accepts the notifier directly
  /// and an optional injected [parser] so tests don't have to load
  /// the FRB dylib — pass a closure that returns a synthetic
  /// `LaunchIntentDto`.
  @visibleForTesting
  static void handleForTest(
    ProviderContainer container,
    Uri uri, {
    LaunchIntentParser parser = _defaultParser,
  }) => _dispatch(container.read(launchIntentProvider.notifier), uri, parser);

  static void _dispatch(
    LaunchIntentNotifier notifier,
    Uri uri,
    LaunchIntentParser parser,
  ) {
    final url = uri.toString();
    final dto = parser(url);
    if (dto == null) {
      // Unsupported URIs are dropped here: non-upeg schemes, malformed
      // upeg links, and `upeg://open` lookalikes all resolve to null
      // in the Rust parser.
      debugPrint('upeg: ignoring unsupported deep link "$url"');
      return;
    }
    notifier.set(LaunchIntent.fromDto(dto));
  }
}
