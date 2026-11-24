import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:http/http.dart' as http;
import 'package:url_launcher/url_launcher.dart';

/// The native installer may terminate the process before returning.
enum DesktopUpdateAction { installed, openedDownloads }

class DesktopUpdateException implements Exception {
  const DesktopUpdateException(this.message);
  final String message;
  @override
  String toString() => message;
}

/// Checks for releases without downloading anything. Call [perform] only
/// after the user has chosen to update.
class DesktopUpdateService {
  DesktopUpdateService({
    Future<Uri?> Function()? resolveLatest,
    Future<bool> Function(Uri)? openUrl,
    Future<String?> Function(String)? nativeUpdate,
    TargetPlatform? platform,
  }) : _resolveLatest = resolveLatest ?? _resolvePublicLatest,
       _openUrl = openUrl ?? _openExternal,
       _nativeUpdate = nativeUpdate ?? _performNativeUpdate,
       _platform = platform ?? defaultTargetPlatform;

  final Future<Uri?> Function() _resolveLatest;
  final Future<bool> Function(Uri) _openUrl;
  final Future<String?> Function(String) _nativeUpdate;
  final TargetPlatform _platform;

  static final Uri _latest = Uri.parse(
    'https://github.com/5pecia1/UPeg/releases/latest',
  );
  static final RegExp _version = RegExp(
    r'^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$',
  );

  /// Returns the newer stable version, or null if no update can be established.
  Future<String?> check(String currentVersion) async {
    final current = _parts(currentVersion);
    if (current == null) return null;
    try {
      final uri = await _resolveLatest();
      if (uri == null ||
          uri.scheme != 'https' ||
          uri.host != 'github.com' ||
          uri.hasPort ||
          uri.userInfo.isNotEmpty ||
          uri.hasQuery ||
          uri.hasFragment) {
        return null;
      }
      final match = RegExp(
        r'^/5pecia1/UPeg/releases/tag/v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$',
      ).firstMatch(uri.path);
      if (match == null) return null;
      final version = '${match[1]}.${match[2]}.${match[3]}';
      final latest = _parts(version)!;
      for (var i = 0; i < 3; i++) {
        if (latest[i] > current[i]) return version;
        if (latest[i] < current[i]) return null;
      }
      return null;
    } on Exception {
      return null;
    }
  }

  Future<DesktopUpdateAction> perform(String version) async {
    if (_parts(version) == null) {
      throw ArgumentError.value(version, 'version', 'Invalid stable version');
    }
    if (!kIsWeb &&
        (_platform == TargetPlatform.macOS ||
            _platform == TargetPlatform.linux)) {
      try {
        final result = await _nativeUpdate(version);
        if (result == 'installed') return DesktopUpdateAction.installed;
        if (result == 'openedDownloads') {
          return DesktopUpdateAction.openedDownloads;
        }
        throw const DesktopUpdateException('Unexpected native update result');
      } on MissingPluginException {
        // Older or locally built runners can still open the release page.
      }
    }
    final uri = Uri.https('github.com', '/5pecia1/UPeg/releases/tag/v$version');
    if (!await _openUrl(uri)) {
      throw const DesktopUpdateException('Could not open release page');
    }
    return DesktopUpdateAction.openedDownloads;
  }

  static List<BigInt>? _parts(String value) {
    final match = _version.firstMatch(value);
    if (match == null) return null;
    return [for (var i = 1; i <= 3; i++) BigInt.parse(match[i]!)];
  }

  static Future<Uri?> _resolvePublicLatest() async {
    final client = http.Client();
    try {
      final request = http.Request('HEAD', _latest)..followRedirects = false;
      final response = await client
          .send(request)
          .timeout(const Duration(seconds: 10));
      if (response.statusCode < 300 || response.statusCode >= 400) return null;
      final location = response.headers['location'];
      return location == null ? null : _latest.resolve(location);
    } finally {
      client.close();
    }
  }

  static Future<bool> _openExternal(Uri uri) =>
      launchUrl(uri, mode: LaunchMode.externalApplication);

  static Future<String?> _performNativeUpdate(String version) =>
      const MethodChannel(
        'upeg/desktop_update',
      ).invokeMethod<String>('performUpdate', {'version': version});
}
