/// Riverpod state for the [HostAttachConfig] pairing.
///
/// `build()` hydrates from the [HostAttachStore] seam (localStorage on
/// web, no-op on desktop); [HostAttachConfigNotifier.save] persists back
/// through the same seam. Widget tests override [hostAttachStoreProvider]
/// with an in-memory fake so no browser storage is touched.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/features/host_attach/host_attach_config.dart';
import 'package:upeg/src/features/host_attach/_host_attach_storage_stub.dart'
    if (dart.library.html) 'package:upeg/src/features/host_attach/_host_attach_storage_web.dart';

/// Persistence seam for the host-attach pairing. Reads are synchronous so
/// the board can consult the pairing during `build` without an async gap.
abstract interface class HostAttachStore {
  HostAttachConfig read();
  void write(HostAttachConfig config);
}

/// Production store — reads/writes the conditional-import KV (localStorage
/// on web, no-op stub elsewhere).
final class KeyValueHostAttachStore implements HostAttachStore {
  const KeyValueHostAttachStore();

  @override
  HostAttachConfig read() {
    final baseUrl = readHostAttachValue(kHostAttachBaseUrlStorageKey);
    final token = readHostAttachValue(kHostAttachTokenStorageKey);
    if (baseUrl == null && token == null) return HostAttachConfig.empty;
    return HostAttachConfig(baseUrl: baseUrl ?? '', token: token ?? '');
  }

  @override
  void write(HostAttachConfig config) {
    writeHostAttachValue(kHostAttachBaseUrlStorageKey, config.baseUrl);
    writeHostAttachValue(kHostAttachTokenStorageKey, config.token);
  }
}

/// Storage seam. Tests override with an in-memory fake.
final hostAttachStoreProvider = Provider<HostAttachStore>(
  (ref) => const KeyValueHostAttachStore(),
);

/// The current pairing. Seeded from the store; [save] persists changes.
final hostAttachConfigProvider =
    NotifierProvider<HostAttachConfigNotifier, HostAttachConfig>(
      HostAttachConfigNotifier.new,
    );

class HostAttachConfigNotifier extends Notifier<HostAttachConfig> {
  @override
  HostAttachConfig build() => ref.read(hostAttachStoreProvider).read();

  /// Persist [next] through the store and update in-memory state.
  void save(HostAttachConfig next) {
    ref.read(hostAttachStoreProvider).write(next);
    state = next;
  }
}
