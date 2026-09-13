/// Typed client for the paired `upeg` daemon's REST API (Task B3).
///
/// Every network outcome collapses into a closed set of sealed result
/// types so callers switch exhaustively instead of inspecting status
/// codes or catching raw exceptions:
///   * healthz  → [HealthzResult]        (ok / unreachable)
///   * list     → [AttachListResult]     (ok / unreachable / unauthorized /
///                                         unavailable)
///   * dispatch → [AttachDispatchResult] (ok / unreachable / unauthorized /
///                                         unavailable / toolError)
///
/// The concrete [HttpAttachClient] only ever runs on the wasm/PWA runtime;
/// widget/unit tests inject a fake [AttachClient] via `attachClientProvider`.
library;

import 'dart:async';
import 'dart:convert';

import 'package:http/http.dart' as http;

import 'package:upeg/src/features/host_attach/attach_canonical_result_decoder.dart';
import 'package:upeg/src/features/host_attach/attach_response_body_reader.dart';
import 'package:upeg/src/features/host_attach/host_attach_config.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

/// Wall-clock budget for a single attach request. Past this the request is
/// treated as `unreachable` (the daemon is not answering).
const Duration kAttachRequestTimeout = Duration(seconds: 8);

/// HTTP status meaning "the host answered but cannot serve this right
/// now". A running host always serves `/v1` — the desired-state gate is
/// gone — so a 503 is a generic upstream condition (a reverse proxy, a
/// shutting-down host), not a switch the user can flip.
const int kHostUnavailableStatus = 503;

/// Fallback hint shown when a 503 body carries no explicit remediation.
const String kHostUnavailableDefaultHint = '호스트가 일시적으로 응답할 수 없습니다';

const String kAttachResponseTooLargeErrorCode = 'response_too_large';
const String kAttachMalformedResponseErrorCode = 'malformed_response';

// ─── healthz ──────────────────────────────────────────────────────────

sealed class HealthzResult {
  const HealthzResult();
}

/// `GET /healthz` answered. A reachable host always serves the REST data
/// plane, so the probe carries identity only — there is no capability
/// flag left to report.
final class HealthzOk extends HealthzResult {
  const HealthzOk({required this.name, required this.version});

  final String name;
  final String version;
}

/// No answer (connection refused, DNS/CORS failure, timeout, malformed body).
final class HealthzUnreachable extends HealthzResult {
  const HealthzUnreachable();
}

// ─── list ─────────────────────────────────────────────────────────────

/// One row of `GET /v1/tools`.
final class AttachToolSummary {
  const AttachToolSummary({required this.id, required this.label});

  final String id;
  final String label;
}

sealed class AttachListResult {
  const AttachListResult();
}

final class AttachListOk extends AttachListResult {
  const AttachListOk(this.tools);
  final List<AttachToolSummary> tools;
}

final class AttachListUnreachable extends AttachListResult {
  const AttachListUnreachable();
}

final class AttachListUnauthorized extends AttachListResult {
  const AttachListUnauthorized();
}

final class AttachListUnavailable extends AttachListResult {
  const AttachListUnavailable(this.hint);
  final String hint;
}

// ─── dispatch ─────────────────────────────────────────────────────────

sealed class AttachDispatchResult {
  const AttachDispatchResult();
}

/// The tool ran remotely; [result] is the canonical outcome, ready to feed
/// `lastOutcomeProvider` for uniform inline rendering.
final class AttachDispatchOk extends AttachDispatchResult {
  const AttachDispatchOk(this.result);
  final CanonicalToolResult result;
}

final class AttachDispatchUnreachable extends AttachDispatchResult {
  const AttachDispatchUnreachable();
}

final class AttachDispatchUnauthorized extends AttachDispatchResult {
  const AttachDispatchUnauthorized();
}

final class AttachDispatchUnavailable extends AttachDispatchResult {
  const AttachDispatchUnavailable(this.hint);
  final String hint;
}

/// The daemon ran the tool but it failed (`ok:false`, or a tool-scoped
/// HTTP error). Carries the structured error for display.
final class AttachDispatchToolError extends AttachDispatchResult {
  const AttachDispatchToolError(this.error);
  final CanonicalToolError error;
}

// ─── client ───────────────────────────────────────────────────────────

/// Client seam. Production is [HttpAttachClient]; tests inject a fake.
abstract interface class AttachClient {
  Future<HealthzResult> checkHealth();
  Future<AttachListResult> listTools();

  /// Dispatch [toolId] with [args]. When [boardKey] is set the call
  /// routes through `POST /v1/boards/{board}/tools/{id}` so the host
  /// applies its board gate (user pegboard state) and pin-preset merge;
  /// otherwise the global `/v1/tools/{id}` route is used.
  Future<AttachDispatchResult> dispatch({
    required ToolId toolId,
    required ToolArgs args,
    String? boardKey,
  });
}

/// REST client against a configured [HostAttachConfig]. kIsWeb-safe:
/// `package:http` routes through `fetch` on the web target.
final class HttpAttachClient implements AttachClient {
  HttpAttachClient({required this.config, http.Client? httpClient})
    : _http = httpClient ?? http.Client();

  final HostAttachConfig config;
  final http.Client _http;

  Uri? _uri(String path) {
    final base = _trimmedBase();
    if (base.isEmpty) return null;
    return Uri.parse('$base$path');
  }

  String _trimmedBase() {
    final base = config.baseUrl.trim();
    return base.endsWith('/') ? base.substring(0, base.length - 1) : base;
  }

  Map<String, String> _authHeaders({bool json = false}) {
    return <String, String>{
      if (config.token.trim().isNotEmpty)
        'Authorization': 'Bearer ${config.token.trim()}',
      if (json) 'Content-Type': 'application/json',
    };
  }

  @override
  Future<HealthzResult> checkHealth() async {
    try {
      final uri = _uri('/healthz');
      if (uri == null) return const HealthzUnreachable();
      final resp = await _http.get(uri).timeout(kAttachRequestTimeout);
      if (resp.statusCode != 200) return const HealthzUnreachable();
      final decoded = jsonDecode(resp.body);
      if (decoded is! Map) return const HealthzUnreachable();
      final name = decoded['name'];
      final version = decoded['version'];
      if (name is! String) return const HealthzUnreachable();
      return HealthzOk(name: name, version: version is String ? version : '');
    } on Object {
      return const HealthzUnreachable();
    }
  }

  @override
  Future<AttachListResult> listTools() async {
    try {
      final uri = _uri('/v1/tools');
      if (uri == null) return const AttachListUnreachable();
      final resp = await _http
          .get(uri, headers: _authHeaders())
          .timeout(kAttachRequestTimeout);
      switch (resp.statusCode) {
        case 401 || 403:
          return const AttachListUnauthorized();
        case kHostUnavailableStatus:
          return AttachListUnavailable(_unavailableHint(resp.body));
        case 200:
          final decoded = jsonDecode(resp.body);
          final rows = decoded is List ? decoded : const <Object?>[];
          return AttachListOk(<AttachToolSummary>[
            for (final row in rows)
              if (row is Map)
                AttachToolSummary(
                  id: '${row['id'] ?? ''}',
                  label: '${row['label'] ?? row['id'] ?? ''}',
                ),
          ]);
        default:
          return const AttachListUnreachable();
      }
    } on Object {
      return const AttachListUnreachable();
    }
  }

  @override
  Future<AttachDispatchResult> dispatch({
    required ToolId toolId,
    required ToolArgs args,
    String? boardKey,
  }) async {
    try {
      final path = boardKey == null
          ? '/v1/tools/${toolId.value}'
          : '/v1/boards/$boardKey/tools/${toolId.value}';
      final uri = _uri(path);
      if (uri == null) return const AttachDispatchUnreachable();
      final abortTrigger = Completer<void>();
      final request =
          http.AbortableRequest('POST', uri, abortTrigger: abortTrigger.future)
            ..headers.addAll(_authHeaders(json: true))
            ..body = args.encodeJson();
      final stopwatch = Stopwatch()..start();
      final deadlineTimer = Timer(
        kAttachRequestTimeout,
        () => _completeAbort(abortTrigger),
      );
      try {
        final response = await _http
            .send(request)
            .timeout(
              kAttachRequestTimeout,
              onTimeout: () {
                _completeAbort(abortTrigger);
                throw TimeoutException('host attach request timed out');
              },
            );
        final remainingMicros =
            kAttachRequestTimeout.inMicroseconds -
            stopwatch.elapsedMicroseconds;
        final bodyResult = await readAttachResponseBody(
          response,
          timeout: Duration(microseconds: remainingMicros),
        );
        final String body;
        switch (bodyResult) {
          case AttachResponseBodyOk(body: final value):
            body = value;
          case AttachResponseBodyTooLarge():
            _completeAbort(abortTrigger);
            return _dispatchToolError(
              code: kAttachResponseTooLargeErrorCode,
              message:
                  '호스트 응답이 최대 '
                  '$kAttachDispatchResponseMaxBytes bytes를 초과했습니다',
            );
          case AttachResponseBodyMalformed():
            return _dispatchToolError(
              code: kAttachMalformedResponseErrorCode,
              message: '호스트 응답이 유효한 UTF-8이 아닙니다',
            );
          case AttachResponseBodyUnreachable():
            _completeAbort(abortTrigger);
            return const AttachDispatchUnreachable();
        }
        switch (response.statusCode) {
          case 401 || 403:
            return const AttachDispatchUnauthorized();
          case kHostUnavailableStatus:
            return AttachDispatchUnavailable(_unavailableHint(body));
          case 200:
            return decodeDispatchBody(body);
          default:
            // A tool-scoped failure the daemon still answered structurally.
            return decodeDispatchBody(
              body,
              statusFallback: response.statusCode,
            );
        }
      } finally {
        deadlineTimer.cancel();
      }
    } on Object {
      return const AttachDispatchUnreachable();
    }
  }

  String _unavailableHint(String body) {
    try {
      final decoded = jsonDecode(body);
      if (decoded is Map) {
        final err = decoded['error'];
        if (err is Map && err['message'] is String) {
          return err['message'] as String;
        }
        if (decoded['message'] is String) return decoded['message'] as String;
      }
    } on FormatException {
      // fall through to default
    }
    return kHostUnavailableDefaultHint;
  }
}

void _completeAbort(Completer<void> trigger) {
  if (!trigger.isCompleted) trigger.complete();
}

/// Parse a canonical `POST /v1/tools/{id}` response body into an
/// [AttachDispatchResult]. `ok:true` → [AttachDispatchOk]; otherwise
/// [AttachDispatchToolError]. Unparseable bodies degrade to a tool error.
AttachDispatchResult decodeDispatchBody(String body, {int? statusFallback}) {
  final Object? decoded;
  try {
    decoded = jsonDecode(body);
  } on FormatException {
    return AttachDispatchToolError(
      CanonicalToolError(
        code: kAttachMalformedResponseErrorCode,
        message: statusFallback == null
            ? '호스트 응답을 해석할 수 없습니다'
            : 'host returned status $statusFallback',
      ),
    );
  }
  if (decoded is! Map) {
    return const AttachDispatchToolError(
      CanonicalToolError(
        code: kAttachMalformedResponseErrorCode,
        message: '호스트 응답 형식이 올바르지 않습니다',
      ),
    );
  }
  final result = decodeAttachCanonicalResult(decoded);
  if (result.ok) return AttachDispatchOk(result);
  return AttachDispatchToolError(
    result.error ??
        const CanonicalToolError(code: 'tool_error', message: '도구 실행 실패'),
  );
}

AttachDispatchToolError _dispatchToolError({
  required String code,
  required String message,
}) {
  return AttachDispatchToolError(
    CanonicalToolError(code: code, message: message),
  );
}
