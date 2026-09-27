/// Connect the common Rust dispatcher to the application-owned WebView service.
library;

import 'dart:async';

import 'package:upeg/src/features/controlled_embed/session_service.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/embed.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/widgets/controlled_embed/runner.dart';
import 'package:upeg/src/rust/api/webview.dart' as frb;
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/widgets/controlled_embed/settings.dart';

abstract interface class WebViewExecutionApi {
  BigInt create();
  Stream<frb.WebViewExecutionEventDto> events(BigInt providerId);
  bool complete(
    BigInt providerId,
    BigInt requestId,
    frb.WebViewExecutionCompletionDto result,
  );
  bool unregister(BigInt providerId);
}

final class FrbWebViewExecutionApi implements WebViewExecutionApi {
  const FrbWebViewExecutionApi();

  @override
  BigInt create() => frb.createWebviewProvider();

  @override
  Stream<frb.WebViewExecutionEventDto> events(BigInt providerId) =>
      frb.webviewExecutionStream(providerId: providerId);

  @override
  bool complete(
    BigInt providerId,
    BigInt requestId,
    frb.WebViewExecutionCompletionDto result,
  ) => frb.completeWebviewExecution(
    providerId: providerId,
    requestId: requestId,
    completion: result,
  );

  @override
  bool unregister(BigInt providerId) =>
      frb.unregisterWebviewProvider(providerId: providerId);
}

final class ControlledEmbedExecutionBridge {
  ControlledEmbedExecutionBridge({
    required ControlledEmbedSessionService sessions,
    WebViewExecutionApi api = const FrbWebViewExecutionApi(),
  }) : this._(sessions, api);

  ControlledEmbedExecutionBridge._(this._sessions, this._api)
    : _providerId = _api.create() {
    // An app boot can dispose the bridge without anyone awaiting readiness.
    unawaited(_ready.future.then<void>((_) {}, onError: (Object _) {}));
    _subscription = _api
        .events(_providerId)
        .listen(
          _onEvent,
          onError: (Object error, StackTrace stack) =>
              _disconnect(error, stack),
          onDone: () => _disconnect(
            StateError('WebView execution stream closed.'),
            StackTrace.current,
          ),
        );
  }

  final ControlledEmbedSessionService _sessions;
  final WebViewExecutionApi _api;
  final BigInt _providerId;
  final Completer<void> _ready = Completer<void>();
  final Map<BigInt, ControlledEmbedCancellation> _pending = {};
  late final StreamSubscription<frb.WebViewExecutionEventDto> _subscription;
  bool _closed = false;

  Future<void> get ready => _ready.future;

  void _onEvent(frb.WebViewExecutionEventDto event) {
    if (_closed) return;
    switch (event) {
      case frb.WebViewExecutionEventDto_Ready():
        if (!_ready.isCompleted) _ready.complete();
      case frb.WebViewExecutionEventDto_Execute(:final request):
        final cancellation = ControlledEmbedCancellation();
        _pending[request.requestId] = cancellation;
        unawaited(_execute(request, cancellation));
      case frb.WebViewExecutionEventDto_Cancel(:final requestId):
        _pending[requestId]?.cancel();
    }
  }

  Future<void> _execute(
    frb.WebViewExecutionRequestDto request,
    ControlledEmbedCancellation cancellation,
  ) async {
    frb.WebViewExecutionCompletionDto completion;
    try {
      final pinKey = switch ((request.boardKey, request.pinId)) {
        (null, null) => null,
        (final String boardKey, final String pinId) => (
          BoardKey.parse(boardKey),
          PinId.parse(pinId),
        ),
        _ => throw StateError('WebView request has incomplete pin context.'),
      };
      final outcome = await _sessions.execute(
        spec: ControlledEmbedSessionSpec(
          pinKey: pinKey,
          toolId: ToolId.parse(request.toolId),
          url: request.url,
          settings: resolveBrowserSettings(request.settings),
        ),
        bindings: request.bindings,
        inputs: Map.fromEntries(
          request.inputs.map((pair) => MapEntry(pair.$1, pair.$2)),
        ),
        cancellation: cancellation,
      );
      completion = webViewCompletionFor(outcome.result, outcome.waitTimeout);
    } on ControlledEmbedCancelled {
      completion = const frb.WebViewExecutionCompletionDto.cancelled();
    } catch (error) {
      completion = frb.WebViewExecutionCompletionDto.failed(
        message: error.toString(),
      );
    }
    try {
      if (!_closed) _api.complete(_providerId, request.requestId, completion);
    } finally {
      _pending.remove(request.requestId);
    }
  }

  void _disconnect(Object error, StackTrace stack) {
    if (_closed) return;
    _closed = true;
    if (!_ready.isCompleted) _ready.completeError(error, stack);
    for (final cancellation in _pending.values) {
      cancellation.cancel();
    }
    _api.unregister(_providerId);
  }

  Future<void> close() async {
    _disconnect(
      StateError('The WebView provider was disposed.'),
      StackTrace.current,
    );
    await _subscription.cancel();
  }
}

frb.WebViewExecutionCompletionDto webViewCompletionFor(
  CanonicalToolResult result,
  ControlledEmbedWaitTimeoutError? timeout,
) {
  if (timeout != null) {
    return frb.WebViewExecutionCompletionDto.waitTimeout(
      role: BindingRoleDto.values.byName(timeout.roleLabel),
      selector: timeout.selector,
      forSelector: timeout.forSelector,
      condition: timeout.condition,
      timeoutMs: BigInt.from(timeout.timeoutMs),
    );
  }
  if (result.ok) {
    return frb.WebViewExecutionCompletionDto.success(
      outputs: result.outputs
          .map((entry) => (entry.id, entry.value.displayText))
          .toList(),
    );
  }
  return frb.WebViewExecutionCompletionDto.failed(
    message: result.error?.message ?? 'WebView execution failed.',
  );
}

/// Normalize on the same Rust output path before exposing a session's result.
/// The original DOM values still return to the waiting Rust invocation.
CanonicalToolResult normalizeControlledEmbedResult(
  ToolId toolId,
  CanonicalToolResult result,
  ControlledEmbedWaitTimeoutError? timeout,
) => frb.normalizeWebviewResult(
  toolId: toolId.value,
  completion: webViewCompletionFor(result, timeout),
);
