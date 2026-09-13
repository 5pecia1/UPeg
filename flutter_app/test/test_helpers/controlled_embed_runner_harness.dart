/// Shared fixtures for the `controlled_embed_runner_*_test.dart` family:
/// the script-builder seam installer, the runJs recorders, and the
/// selector-binding fixtures the runner protocol is asserted against.
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/embed.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/widgets/controlled_embed/runner.dart';

/// Swaps the Rust-backed script builders for deterministic stubs,
/// restoring the originals in `tearDown`. Call once at the top of
/// `main()`.
void useStubbedExecutionScripts() {
  late ExecutionScriptsBuilder previous;
  late BindingWaitScriptBuilder previousWait;

  setUp(() {
    previous = executionScriptsBuilder;
    previousWait = bindingWaitScriptBuilder;
    executionScriptsBuilder = stubExecutionScripts;
    bindingWaitScriptBuilder = stubBindingWaitScript;
  });
  tearDown(() {
    executionScriptsBuilder = previous;
    bindingWaitScriptBuilder = previousWait;
  });
}

/// Wrapper to make the recorder throw any object (not just Exception).
final class ThrownObject {
  const ThrownObject(this.error);
  final Object error;
}

/// Split recorder that tracks command (write/trigger) and result (read) calls separately.
class RunJsRecorder {
  final List<String> commandCalls = [];
  final List<String> resultCalls = [];
  final List<Object?> commandResponses;
  final List<Object?> resultResponses;
  int _commandI = 0;
  int _resultI = 0;
  RunJsRecorder(this.commandResponses, this.resultResponses);

  Future<void> runCommandJs(String script) async {
    commandCalls.add(script);
    if (_commandI >= commandResponses.length) return;
    final r = commandResponses[_commandI];
    _commandI++;
    if (r is ThrownObject) throw r.error;
    if (r is Exception) throw r;
    // command responses are void, so we don't return anything
  }

  Future<Object?> runResultJs(String script) async {
    resultCalls.add(script);
    if (_resultI >= resultResponses.length) return null;
    final r = resultResponses[_resultI];
    _resultI++;
    if (r is ThrownObject) throw r.error;
    if (r is Exception) throw r;
    return r;
  }
}

/// Recorder that handles wait readiness checks separately.
/// Tracks wait calls and returns controlled readiness responses.
class WaitJsRecorder {
  final List<String> commandCalls = [];
  final List<String> waitCalls = [];
  final List<String> resultCalls = [];
  final List<Object?> commandResponses;
  final List<bool> waitResponses;
  final List<Object?> resultResponses;
  int _commandI = 0;
  int _waitI = 0;
  int _resultI = 0;
  WaitJsRecorder({
    this.commandResponses = const [],
    required this.waitResponses,
    this.resultResponses = const [],
  });

  Future<void> runCommandJs(String script) async {
    commandCalls.add(script);
    if (_commandI >= commandResponses.length) return;
    final r = commandResponses[_commandI];
    _commandI++;
    if (r is ThrownObject) throw r.error;
    if (r is Exception) throw r;
  }

  Future<bool> runWaitJs(String script) async {
    waitCalls.add(script);
    if (_waitI >= waitResponses.length) return false;
    final r = waitResponses[_waitI];
    _waitI++;
    return r;
  }

  Future<Object?> runResultJs(String script) async {
    resultCalls.add(script);
    if (_resultI >= resultResponses.length) return null;
    final r = resultResponses[_resultI];
    _resultI++;
    if (r is ThrownObject) throw r.error;
    if (r is Exception) throw r;
    return r;
  }
}

/// Unified recorder for per-binding wait tests.
///
/// Keeps wait/command/result calls in one timeline so tests can assert the
/// exact runner interleaving without reconstructing order from separate lists.
class EventJsRecorder {
  final List<String> events = <String>[];
  final List<Object?> commandResponses;
  final List<Object?> waitResponses;
  final List<Object?> resultResponses;
  int _commandI = 0;
  int _waitI = 0;
  int _resultI = 0;

  EventJsRecorder({
    List<Object?>? commandResponses, // ignore - reserved for future use
    this.waitResponses = const <Object?>[],
    this.resultResponses = const <Object?>[],
  }) : commandResponses = commandResponses ?? const <Object?>[];

  Future<void> runCommandJs(String script) async {
    events.add('command:${_eventLabel(script)}');
    if (_commandI >= commandResponses.length) return;
    final response = commandResponses[_commandI];
    _commandI++;
    _throwIfNeeded(response);
  }

  Future<bool> runWaitJs(String script) async {
    events.add('wait:${_eventLabel(script)}');
    if (_waitI >= waitResponses.length) return false;
    final response = waitResponses[_waitI];
    _waitI++;
    _throwIfNeeded(response);
    return response == true;
  }

  Future<Object?> runResultJs(String script) async {
    events.add('result:${_eventLabel(script)}');
    if (_resultI >= resultResponses.length) return null;
    final response = resultResponses[_resultI];
    _resultI++;
    _throwIfNeeded(response);
    return response;
  }

  static String _eventLabel(String script) {
    if (script.startsWith('wait:')) {
      return script.substring('wait:'.length);
    }
    return script;
  }

  static void _throwIfNeeded(Object? response) {
    if (response is ThrownObject) throw response.error;
    if (response is Exception) throw response;
  }
}

/// Test-local constants for error and payload scenarios.
const kOpaqueObservedError = 'javascript -> ERROR';
const kReadFailure = 'read failed after trigger';
const kExpectedOutput = 'ok';

/// Stub `buildExecutionScripts` so the runner tests don't pull in
/// the FRB bridge. Production calls Rust; tests assemble the
/// `ExecutionScriptsDto` directly.
ExecutionScriptsDto stubExecutionScripts({
  required List<SelectorBindingDto> bindings,
  required List<(String, String)> inputs,
}) {
  final hasInput = bindings.any((b) => b.role == BindingRoleDto.input);
  final hasTrigger = bindings.any((b) => b.role == BindingRoleDto.trigger);
  final hasOutput = bindings.any((b) => b.role == BindingRoleDto.output);
  return ExecutionScriptsDto(
    write: hasInput ? '/*WRITE*/' : '',
    trigger: hasTrigger ? '/*TRIGGER*/' : '',
    read: hasOutput ? '/*READ*/' : '"{}"',
    isActionable: hasInput || hasTrigger || hasOutput,
  );
}

ExecutionScriptsDto eventExecutionScripts({
  required List<SelectorBindingDto> bindings,
  required List<(String, String)> inputs,
}) {
  final hasInput = bindings.any((b) => b.role == BindingRoleDto.input);
  final hasTrigger = bindings.any((b) => b.role == BindingRoleDto.trigger);
  final hasOutput = bindings.any((b) => b.role == BindingRoleDto.output);
  return ExecutionScriptsDto(
    write: hasInput ? 'write' : '',
    trigger: hasTrigger ? 'trigger' : '',
    read: hasOutput ? 'read' : '"{}"',
    isActionable: hasInput || hasTrigger || hasOutput,
  );
}

String stubBindingWaitScript({required SelectorBindingDto binding}) {
  return binding.wait == null ? '' : '/*WAIT*/';
}

String eventBindingWaitScript({required SelectorBindingDto binding}) {
  final wait = binding.wait;
  if (wait == null) return '';
  final selector = wait.forSelector != null && wait.forSelector!.isNotEmpty
      ? wait.forSelector!
      : binding.selector;
  return 'wait:$selector';
}

const emptySettle = Duration.zero;
final big0 = BigInt.zero;
final big1 = BigInt.one;
final big50 = BigInt.from(50);
final big100 = BigInt.from(100);
final big5000 = BigInt.from(5000);

/// Helper: binding with wait config, for reuse across test groups.
SelectorBindingDto inputBindingWithWait({
  String? forSelector,
  BindingWaitConditionDto condition = BindingWaitConditionDto.exists,
  required BigInt timeoutMs,
  BigInt? settleMs,
  BindingWaitOnTimeoutDto onTimeout = BindingWaitOnTimeoutDto.fail,
}) {
  return SelectorBindingDto(
    role: BindingRoleDto.input,
    field: 'q',
    selector: '#q',
    triggerAction: ControlledEmbedTriggerActionDto.click,
    wait: BindingWaitDto(
      forSelector: forSelector,
      condition: condition,
      timeoutMs: timeoutMs,
      settleMs: settleMs ?? BigInt.zero,
      onTimeout: onTimeout,
    ),
  );
}

Map<String, String> outputTextById(CanonicalToolResult result) => {
  for (final output in result.outputs) output.id: output.value.displayText,
};
