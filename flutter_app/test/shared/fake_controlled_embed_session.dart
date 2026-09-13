import 'dart:async';
import 'dart:collection';
import 'dart:convert';

import 'package:flutter/widgets.dart';
import 'package:upeg/src/features/controlled_embed/webview_session.dart';
import 'package:upeg/src/widgets/controlled_embed/settings.dart';

/// A browser factory that owns real asynchronous session boundaries, without
/// creating a platform controller or mounting a native view.
class FakeControlledEmbedSessionFactory {
  final sessions = <FakeControlledEmbedSession>[];
  Completer<void>? nextReadyGate;

  Future<ControlledEmbedWebViewSession> create({
    required String url,
    ResolvedBrowserSettings? settings,
  }) async {
    final session = FakeControlledEmbedSession(
      url: url,
      settings: settings,
      readyGate: nextReadyGate,
    );
    nextReadyGate = null;
    sessions.add(session);
    return session;
  }
}

class FakeControlledEmbedSession implements ControlledEmbedWebViewSession {
  FakeControlledEmbedSession({
    required this.url,
    this.settings,
    Completer<void>? readyGate,
  }) {
    _ready = readyGate == null
        ? Future<void>.value()
        : _untilClosed(readyGate.future);
    unawaited(_ready.then<void>((_) {}, onError: (Object _) {}));
  }

  static const waitPrefix = 'wait:';
  static const triggerScript = 'trigger';
  static const readScript = 'read';
  static const probePayload =
      '[{"role":"output","field":"result","selector":"#result",'
      '"matched":true,"valuePreview":"ready","textPreview":"ready"}]';

  final String url;
  final ResolvedBrowserSettings? settings;
  final operations = <String>[];
  final waitResponses = Queue<bool>();
  final commandStarted = Completer<void>();
  final _commandGates = Queue<Completer<void>>();
  final _closed = Completer<void>();
  late final Future<void> _ready;
  int closeCalls = 0;
  int triggerCount = 0;
  Object? readPayload;

  Completer<void> blockNextCommand() {
    final gate = Completer<void>();
    _commandGates.add(gate);
    return gate;
  }

  Future<void> _untilClosed(Future<void> future) => Future.any([
    future,
    _closed.future.then<void>((_) => throw StateError('Browser is closed.')),
  ]);

  void _checkOpen() {
    if (_closed.isCompleted) throw StateError('Browser is closed.');
  }

  @override
  Future<void> get ready => _ready;

  @override
  Size get viewportSize =>
      settings?.viewportSize ?? ViewPortPresetSizes.desktop;

  @override
  Widget buildView({Key? key, bool visible = true}) => SizedBox(key: key);

  @override
  Future<void> runCommandJs(String script) async {
    _checkOpen();
    operations.add(script);
    if (!commandStarted.isCompleted) commandStarted.complete();
    if (_commandGates.isNotEmpty) {
      await _untilClosed(_commandGates.removeFirst().future);
    }
    if (script == triggerScript) triggerCount++;
  }

  @override
  Future<Object> runResultJs(String script) async {
    _checkOpen();
    if (script.startsWith(waitPrefix)) {
      operations.add(script);
      return waitResponses.isEmpty ? true : waitResponses.removeFirst();
    }
    if (script == readScript) {
      operations.add(script);
      return readPayload ?? jsonEncode({'result': triggerCount.toString()});
    }
    return probePayload;
  }

  @override
  Future<void> close() async {
    closeCalls++;
    if (!_closed.isCompleted) _closed.complete();
  }
}
