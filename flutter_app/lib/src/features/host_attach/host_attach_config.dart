/// PWA host-attach configuration (Task B3).
///
/// On the wasm/PWA runtime a tool whose invoker cannot dispatch in-process
/// (external / http / chain / llm / wasm) can still run if the user has
/// paired the app with a local `upeg` daemon exposing the REST API. This
/// value object holds that pairing: the daemon base URL and the bearer
/// token pasted from the daemon.
///
/// Native surfaces never consult this — they link the full runtime and
/// support every declared tool in-process (see `capability_provider.dart`).
library;

import 'package:flutter/foundation.dart';

/// localStorage keys under which the pairing persists on the web target.
/// Namespaced so they never collide with the memo store's keys.
const String kHostAttachBaseUrlStorageKey = 'upeg.host_attach.base_url';
const String kHostAttachTokenStorageKey = 'upeg.host_attach.token';

/// Immutable host-attach pairing. `baseUrl` empty means "not paired".
@immutable
final class HostAttachConfig {
  const HostAttachConfig({required this.baseUrl, required this.token});

  /// The empty (unpaired) config. `isConfigured` is `false`.
  static const HostAttachConfig empty = HostAttachConfig(
    baseUrl: '',
    token: '',
  );

  final String baseUrl;
  final String token;

  /// True once a non-blank base URL is present. The token may be blank
  /// for a tokenless daemon; auth failures surface as `unauthorized` at
  /// dispatch time rather than blocking the attempt here.
  bool get isConfigured => baseUrl.trim().isNotEmpty;

  HostAttachConfig copyWith({String? baseUrl, String? token}) {
    return HostAttachConfig(
      baseUrl: baseUrl ?? this.baseUrl,
      token: token ?? this.token,
    );
  }

  @override
  int get hashCode => baseUrl.hashCode ^ token.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is HostAttachConfig &&
          runtimeType == other.runtimeType &&
          baseUrl == other.baseUrl &&
          token == other.token;
}
