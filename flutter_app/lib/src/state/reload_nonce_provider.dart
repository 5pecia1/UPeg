/// EmbedPanel reload nonce (J08).
///
/// Monotonically-increasing counter; bumping it forces the iframe
/// branch in [`EmbedPanel`] to re-call `IframeFactory.register` with
/// the same URL, which on the web target wipes the cached
/// `dart:ui_web` platform-view registration and re-mounts the
/// `<iframe>` from scratch.
///
/// `Notifier<int>` rather than `StateProvider<int>` because the
/// `bump()` method is the canonical mutation — callers should not
/// `read.notifier.state++` from random places. A typed method keeps
/// the seam clean.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

class ReloadNonceNotifier extends Notifier<int> {
  @override
  int build() => 0;

  /// Increment the nonce by one. Wraps via `int + 1`; Dart's `int` is
  /// arbitrary-precision so no `wrapping_add` dance is needed.
  void bump() => state = state + 1;
}

final reloadNonceProvider = NotifierProvider<ReloadNonceNotifier, int>(
  ReloadNonceNotifier.new,
);
