/// Application-owned WebView sessions and the shared normal/debug execution.
library;

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:upeg/src/features/controlled_embed/webview_session.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/embed.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/widgets/controlled_embed/debug.dart';
import 'package:upeg/src/widgets/controlled_embed/runner.dart';
import 'package:upeg/src/widgets/controlled_embed/settings.dart';

const kControlledEmbedTraceLimit = 200;

typedef ControlledEmbedSessionFactory =
    Future<ControlledEmbedWebViewSession> Function({
      required String url,
      ResolvedBrowserSettings? settings,
    });

typedef ControlledEmbedResultNormalizer =
    CanonicalToolResult Function(
      ToolId toolId,
      CanonicalToolResult rawResult,
      ControlledEmbedWaitTimeoutError? waitTimeout,
    );

/// A board pin or a tool-level caller keeps its own page until settings change.
typedef _SessionKey = (PinKey?, ToolId?);

@immutable
final class ControlledEmbedSessionSpec {
  const ControlledEmbedSessionSpec({
    this.pinKey,
    required this.toolId,
    required this.url,
    this.settings,
  });

  /// Present for a board placement; null identifies a tool-level call.
  final PinKey? pinKey;
  final ToolId toolId;
  final String url;
  final ResolvedBrowserSettings? settings;

  @override
  bool operator ==(Object other) =>
      other is ControlledEmbedSessionSpec &&
      other._key == _key &&
      other.toolId == toolId &&
      other.url == url &&
      (other.settings?.hasUserAgentOverride ?? false) ==
          (settings?.hasUserAgentOverride ?? false) &&
      other.settings?.userAgent == settings?.userAgent &&
      other.settings?.viewportSize == settings?.viewportSize;

  @override
  int get hashCode => Object.hash(
    toolId,
    _key,
    url,
    settings?.hasUserAgentOverride ?? false,
    settings?.userAgent,
    settings?.viewportSize,
  );

  _SessionKey get _key => pinKey == null ? (null, toolId) : (pinKey, null);
}

enum ControlledEmbedSessionPhase { loading, ready, running, failed }

final class ControlledEmbedSessionEntry {
  ControlledEmbedSessionEntry(this.spec, this.browser);

  final ControlledEmbedSessionSpec spec;
  final ControlledEmbedWebViewSession browser;
  ControlledEmbedSessionPhase phase = ControlledEmbedSessionPhase.loading;
  final List<ControlledEmbedDebugEvent> _events = [];
  List<ControlledEmbedDebugEvent> get events => List.unmodifiable(_events);
  List<ControlledEmbedSelectorProbe> probes = const [];
  CanonicalToolResult? lastResult;
  bool _displayedInDebugger = false;
  bool get displayedInDebugger => _displayedInDebugger;
}

final class ControlledEmbedCancellation {
  final Completer<void> _cancelled = Completer<void>();
  bool get isCancelled => _cancelled.isCompleted;

  void cancel() {
    if (!isCancelled) _cancelled.complete();
  }

  void check() {
    if (isCancelled) throw const ControlledEmbedCancelled();
  }

  Future<T> untilCancelled<T>(Future<T> operation) => Future.any([
    operation,
    _cancelled.future.then<T>((_) => throw const ControlledEmbedCancelled()),
  ]);
}

final class ControlledEmbedCancelled implements Exception {
  const ControlledEmbedCancelled();

  @override
  String toString() => 'Controlled Embed execution cancelled';
}

/// Raw DOM output and typed wait metadata go back through the Rust dispatcher.
final class ControlledEmbedSessionOutcome {
  const ControlledEmbedSessionOutcome(
    this.result,
    this.waitTimeout,
    this.canonicalResult,
  );

  final CanonicalToolResult result;
  final ControlledEmbedWaitTimeoutError? waitTimeout;
  final CanonicalToolResult canonicalResult;
}

final class ControlledEmbedSessionService extends ChangeNotifier {
  ControlledEmbedSessionService({
    required ControlledEmbedSessionFactory createSession,
    required ControlledEmbedResultNormalizer normalizeResult,
    ControlledEmbedRunner runner = const ControlledEmbedRunner(),
  }) : this._(createSession, normalizeResult, runner);

  ControlledEmbedSessionService._(
    this._createSession,
    this._normalizeResult,
    this._runner,
  );

  final ControlledEmbedSessionFactory _createSession;
  final ControlledEmbedResultNormalizer _normalizeResult;
  final ControlledEmbedRunner _runner;
  final Map<_SessionKey, ControlledEmbedSessionEntry> _entries = {};
  final Map<_SessionKey, Future<ControlledEmbedSessionEntry>> _opening = {};
  final Map<_SessionKey, Future<void>> _queues = {};
  final Set<ControlledEmbedCancellation> _executions = {};
  bool _closed = false;

  List<ControlledEmbedSessionEntry> get entries =>
      List.unmodifiable(_entries.values);

  ControlledEmbedSessionEntry? entryForPin(PinKey key) => _entries[(key, null)];

  ControlledEmbedSessionEntry? entryForTool(ToolId toolId) =>
      _entries[(null, toolId)];

  void _notify() {
    if (!_closed) notifyListeners();
  }

  Future<ControlledEmbedSessionEntry> ensure(
    ControlledEmbedSessionSpec spec,
  ) async {
    if (_closed) throw StateError('The WebView service is closed.');
    final opening = _opening[spec._key];
    if (opening != null) {
      await opening;
      return ensure(spec);
    }
    final existing = _entries[spec._key];
    if (existing != null && existing.spec == spec) {
      await existing.browser.ready;
      return existing;
    }
    if (existing != null &&
        (existing.phase == ControlledEmbedSessionPhase.running ||
            existing.displayedInDebugger)) {
      throw StateError(
        'Close the debugger and finish the run before changing browser settings.',
      );
    }
    final pending = _open(spec, existing);
    _opening[spec._key] = pending;
    try {
      return await pending;
    } finally {
      _opening.remove(spec._key);
    }
  }

  Future<ControlledEmbedSessionEntry> _open(
    ControlledEmbedSessionSpec spec,
    ControlledEmbedSessionEntry? previous,
  ) async {
    if (previous != null) {
      _entries.remove(spec._key);
      _notify();
      await previous.browser.close();
    }
    final browser = await _createSession(
      url: spec.url,
      settings: spec.settings,
    );
    if (_closed) {
      await browser.close();
      throw StateError('The WebView service is closed.');
    }
    final entry = ControlledEmbedSessionEntry(spec, browser);
    _entries[spec._key] = entry;
    // Publish before waiting: the hidden host mounts the native viewport.
    _notify();
    try {
      await browser.ready;
      if (_closed) throw StateError('The WebView service is closed.');
      entry.phase = ControlledEmbedSessionPhase.ready;
      _notify();
      return entry;
    } catch (_) {
      if (identical(_entries[spec._key], entry)) {
        _entries.remove(spec._key);
        _notify();
        await browser.close();
      }
      rethrow;
    }
  }

  /// Only one view may display a controller. The caller waits a Flutter frame
  /// after reserving it so the hidden host removes its view first.
  VoidCallback reserveDebugger(ControlledEmbedSessionEntry entry) {
    if (entry.displayedInDebugger) {
      throw StateError('This browser session is already open in a debugger.');
    }
    entry._displayedInDebugger = true;
    _notify();
    var released = false;
    return () {
      if (released) return;
      released = true;
      entry._displayedInDebugger = false;
      _notify();
    };
  }

  void clearEventsForPin(PinKey key) {
    entryForPin(key)?._events.clear();
    _notify();
  }

  void clearEventsForTool(ToolId toolId) {
    entryForTool(toolId)?._events.clear();
    _notify();
  }

  Future<ControlledEmbedSessionOutcome> execute({
    required ControlledEmbedSessionSpec spec,
    required List<SelectorBindingDto> bindings,
    required Map<String, String> inputs,
    required ControlledEmbedCancellation cancellation,
  }) {
    if (_closed) {
      return Future.error(StateError('The WebView service is closed.'));
    }
    _executions.add(cancellation);
    final previous = _queues[spec._key] ?? Future<void>.value();
    final operation = previous.then((_) async {
      cancellation.check();
      final entry = await cancellation.untilCancelled(ensure(spec));
      cancellation.check();
      entry.phase = ControlledEmbedSessionPhase.running;
      entry._events.clear();
      entry.probes = const [];
      _notify();
      ControlledEmbedWaitTimeoutError? waitTimeout;
      try {
        final result = await _runner.execute(
          runCommandJs: (script) async {
            cancellation.check();
            await entry.browser.runCommandJs(script);
          },
          runResultJs: (script) async {
            cancellation.check();
            return entry.browser.runResultJs(script);
          },
          bindings: bindings,
          inputs: inputs,
          onEvent: (event) {
            entry._events.add(event);
            if (entry._events.length > kControlledEmbedTraceLimit) {
              entry._events.removeAt(0);
            }
            _notify();
          },
          onProbes: (probes) {
            entry.probes = List.unmodifiable(probes);
            _notify();
          },
          onWaitTimeout: (error) => waitTimeout = error,
        );
        cancellation.check();
        final canonical = _normalizeResult(spec.toolId, result, waitTimeout);
        entry.lastResult = canonical;
        entry.phase = canonical.ok
            ? ControlledEmbedSessionPhase.ready
            : ControlledEmbedSessionPhase.failed;
        if (result.ok && !canonical.ok) {
          entry._events.add(
            ControlledEmbedDebugEvent(
              kind: 'output',
              phase: ControlledEmbedDebugPhase.read,
              level: ControlledEmbedDebugLevel.error,
              message: canonical.error?.message ?? 'Output conversion failed.',
              elapsedMs: entry._events.lastOrNull?.elapsedMs ?? 0,
              sequence: (entry._events.lastOrNull?.sequence ?? 0) + 1,
            ),
          );
        }
        return ControlledEmbedSessionOutcome(result, waitTimeout, canonical);
      } finally {
        if (entry.phase == ControlledEmbedSessionPhase.running) {
          entry.phase = ControlledEmbedSessionPhase.ready;
        }
        _notify();
      }
    });
    final settled = operation.then<void>((_) {}, onError: (Object _) {});
    _queues[spec._key] = settled;
    unawaited(
      settled.then((_) {
        _executions.remove(cancellation);
        if (identical(_queues[spec._key], settled)) {
          _queues.remove(spec._key);
        }
      }),
    );
    // Release the caller promptly while retaining the queue until an already
    // started JavaScript operation has settled. Later calls cannot overtake it.
    return cancellation.untilCancelled(operation);
  }

  Future<void> close() async {
    if (_closed) return;
    _closed = true;
    for (final cancellation in _executions) {
      cancellation.cancel();
    }
    final sessions = _entries.values.toList();
    _entries.clear();
    await Future.wait(sessions.map((entry) => entry.browser.close()));
    super.dispose();
  }
}
