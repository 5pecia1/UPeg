import 'dart:collection';

/// Separates command responses from result responses for debug event tests.
class DebugJavaScriptRecorder {
  DebugJavaScriptRecorder(
    List<Object?> commandResponses,
    List<Object?> resultResponses,
  ) : _commandResponses = Queue.of(commandResponses),
      _resultResponses = Queue.of(resultResponses);

  final commandCalls = <String>[];
  final resultCalls = <String>[];
  final Queue<Object?> _commandResponses;
  final Queue<Object?> _resultResponses;

  Future<void> runCommandJs(String script) async {
    commandCalls.add(script);
    if (_commandResponses.isEmpty) return;
    final response = _commandResponses.removeFirst();
    if (response is Exception) throw response;
  }

  Future<Object?> runResultJs(String script) async {
    resultCalls.add(script);
    if (_resultResponses.isEmpty) return null;
    final response = _resultResponses.removeFirst();
    if (response is Exception) throw response;
    return response;
  }
}

/// Records page operations while answering observational probes separately.
class FakeControlledEmbedBrowser {
  FakeControlledEmbedBrowser({
    List<bool> waitResponses = const [],
    this.readPayload = '{"result":"ready"}',
    this.probeFailure,
  }) : _waitResponses = Queue.of(waitResponses);

  static const waitPrefix = 'wait:';
  static const readScript = 'read';
  static const probePayload =
      '[{"role":"output","field":"result","selector":"#result",'
      '"matched":true,"valuePreview":"ready","textPreview":"ready"}]';

  final Queue<bool> _waitResponses;
  final Object? readPayload;
  final Exception? probeFailure;
  final operations = <String>[];

  Future<void> runCommandJs(String script) async {
    operations.add(script);
  }

  Future<Object?> runResultJs(String script) async {
    if (script.startsWith(waitPrefix)) {
      operations.add(script);
      return _waitResponses.isNotEmpty ? _waitResponses.removeFirst() : false;
    }
    if (script == readScript) {
      operations.add(script);
      return readPayload;
    }
    if (probeFailure case final error?) throw error;
    return probePayload;
  }
}
