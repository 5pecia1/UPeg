/// Debug event model and selector probe helpers for Controlled Embed.
///
/// This file is **backend-agnostic**: no Flutter widget, WebViewController,
/// modal, or CDP implementation imports. It provides the data models and
/// helper functions needed by both the debug modal (Task 3) and the debug
/// executor (Task 2).
library;

import 'dart:convert';

import 'package:upeg/src/rust/api/embed.dart';

/// Debug event severity level. Ordered from least to most severe.
enum ControlledEmbedDebugLevel {
  /// Informational message (neutral).
  info,

  /// Successful operation.
  success,

  /// Warning (non-fatal issue).
  warning,

  /// Error (operation failed).
  error,
}

/// Pipeline phase during which the debug event occurred.
enum ControlledEmbedDebugPhase {
  /// Initial setup before any WebView interaction.
  setup,

  /// WebView controller creation/initialization.
  webview,

  /// Selector probe execution (DOM query).
  probe,

  /// Input field writing phase.
  write,

  /// Trigger click execution phase.
  trigger,

  /// Output reading phase.
  read,

  /// Console log capture.
  console,
}

/// Target backend kind for debug session.
enum ControlledEmbedDebugTargetKind {
  /// Flutter's in-app WebView (default).
  flutterWebView,

  /// Local browser with CDP attach (planned, not yet implemented).
  localBrowserCdp,
}

/// Debug event representing a single console row or pipeline step.
class ControlledEmbedDebugEvent {
  /// Event kind identifier (e.g., 'probe', 'write', 'read').
  final String kind;

  /// Pipeline phase when this event occurred.
  final ControlledEmbedDebugPhase phase;

  /// Severity level.
  final ControlledEmbedDebugLevel level;

  /// Human-readable message.
  final String message;

  /// Optional structured details (may be null).
  final Map<String, dynamic>? details;

  /// Milliseconds elapsed since debug session start.
  final int elapsedMs;

  /// Monotonically increasing sequence number.
  final int sequence;

  const ControlledEmbedDebugEvent({
    required this.kind,
    required this.phase,
    required this.level,
    required this.message,
    this.details,
    required this.elapsedMs,
    required this.sequence,
  });
}

/// Debug backend capabilities. Defines which features are available.
class ControlledEmbedDebugCapabilities {
  /// Whether the target shows a visible WebView window.
  final bool visibleWebView;

  /// Whether console logs can be captured.
  final bool consoleCapture;

  /// Whether selector probes (DOM query) is supported.
  final bool selectorProbe;

  /// Whether screenshots can be taken.
  final bool screenshot;

  /// Whether CDP attach is available (for localBrowserCdp only).
  final bool cdpAttach;

  /// Whether network traffic can be captured.
  final bool networkCapture;

  /// Default constructor with all optional parameters.
  const ControlledEmbedDebugCapabilities({
    this.visibleWebView = false,
    this.consoleCapture = false,
    this.selectorProbe = false,
    this.screenshot = false,
    this.cdpAttach = false,
    this.networkCapture = false,
  });

  /// Flutter WebView default capabilities.
  factory ControlledEmbedDebugCapabilities.flutterWebView() {
    return const ControlledEmbedDebugCapabilities(
      visibleWebView: true,
      consoleCapture: false, // App-level JS hooks not yet implemented
      selectorProbe: true,
      screenshot: false, // Not implemented in this plan
      cdpAttach: false,
      networkCapture: false,
    );
  }

  /// Local browser with CDP capabilities.
  ///
  /// This backend is planned but **not yet implemented** — there is no
  /// CDP attach, screenshot, network-capture, console-capture, or
  /// selector-probe code path behind it. The capability report is
  /// therefore all-`false` (honest "false until implemented") so the
  /// debug UI never advertises a feature it cannot deliver. Each flag
  /// flips to `true` only when its implementation lands.
  factory ControlledEmbedDebugCapabilities.localBrowserCdp() {
    return const ControlledEmbedDebugCapabilities(
      visibleWebView: false,
      consoleCapture: false,
      selectorProbe: false,
      screenshot: false,
      cdpAttach: false,
      networkCapture: false,
    );
  }
}

/// Selector probe result for a single binding.
class ControlledEmbedSelectorProbe {
  /// Role of this binding (input/trigger/output).
  final BindingRoleDto role;

  /// Field name from the binding.
  final String field;

  /// CSS selector used for matching.
  final String selector;

  /// Whether the selector matched an element.
  final bool matched;

  /// Preview of the element's value attribute (capped at 160 chars).
  final String valuePreview;

  /// Preview of the element's textContent (capped at 160 chars).
  final String textPreview;

  const ControlledEmbedSelectorProbe({
    required this.role,
    required this.field,
    required this.selector,
    required this.matched,
    required this.valuePreview,
    required this.textPreview,
  });
}

/// In-memory debug session containing events, probes, and outputs.
///
/// No persistence - all state is kept in memory for the duration
/// of a debug session.
class ControlledEmbedDebugSession {
  /// Target backend kind.
  final ControlledEmbedDebugTargetKind targetKind;

  /// Backend capabilities.
  final ControlledEmbedDebugCapabilities capabilities;

  /// All debug events in chronological order.
  final List<ControlledEmbedDebugEvent> events = [];

  /// Latest selector probe results (from last probe execution).
  List<ControlledEmbedSelectorProbe> latestProbes = [];

  /// Latest output values (from last read execution).
  Map<String, String> latestOutputs = {};

  /// Sequence counter for events.
  int _sequence = 0;

  ControlledEmbedDebugSession({
    required this.targetKind,
    required this.capabilities,
  });

  /// Add a new event to the session.
  void addEvent(ControlledEmbedDebugEvent event) {
    events.add(event);
    _sequence = event.sequence + 1;
  }

  /// Current sequence number for next event.
  int get nextSequence => _sequence;
}

/// Build a JavaScript probe script that queries DOM elements.
///
/// Returns a JS string that produces a JSON array of probe results:
/// `[{"role":..., "field":..., "selector":..., "matched":bool, "valuePreview":..., "textPreview":...}, ...]`
///
/// The script safely escapes selectors using `jsonEncode`.
String buildControlledEmbedSelectorProbeScript(
  List<SelectorBindingDto> bindings,
) {
  if (bindings.isEmpty) {
    return 'JSON.stringify([])';
  }

  final buffer = StringBuffer();
  buffer.writeln('(function(){');
  buffer.writeln('var results=[];');

  for (final binding in bindings) {
    final escapedSelector = jsonEncode(binding.selector);
    final escapedField = jsonEncode(binding.field);
    final roleStr = switch (binding.role) {
      BindingRoleDto.input => 'input',
      BindingRoleDto.trigger => 'trigger',
      BindingRoleDto.output => 'output',
    };

    buffer.writeln('(function(){');
    buffer.writeln('var el=document.querySelector($escapedSelector);');
    buffer.writeln('if(el){');
    buffer.writeln('var vp=el.value?el.value.toString().substring(0,160):"";');
    buffer.writeln(
      'var tp=el.textContent?el.textContent.toString().substring(0,160):"";',
    );
    buffer.writeln(
      'results.push({role:"$roleStr",field:$escapedField,selector:$escapedSelector,matched:true,valuePreview:vp,textPreview:tp});',
    );
    buffer.writeln('}else{');
    buffer.writeln(
      'results.push({role:"$roleStr",field:$escapedField,selector:$escapedSelector,matched:false,valuePreview:"",textPreview:""});',
    );
    buffer.writeln('}');
    buffer.writeln('})();');
  }

  buffer.writeln('return JSON.stringify(results);');
  buffer.write('})()');

  return buffer.toString();
}

/// Parse WebView probe result into probe DTOs.
///
/// `raw` should be the parsed JSON result from the probe script
/// (either a List or a JSON string). Returns empty list on any error.
List<ControlledEmbedSelectorProbe> parseControlledEmbedSelectorProbes(
  Object? raw,
) {
  if (raw == null) return [];

  Object? cursor = raw;

  // Handle string input (JSON-encoded)
  if (cursor is String) {
    try {
      cursor = jsonDecode(cursor);
    } catch (_) {
      return [];
    }
  }

  // Must be a list
  if (cursor is! List) return [];

  final probes = <ControlledEmbedSelectorProbe>[];

  for (final item in cursor) {
    if (item is! Map) continue;

    final roleStr = item['role'] as String?;
    final field = item['field'] as String?;
    final selector = item['selector'] as String?;
    final matched = item['matched'] as bool?;
    final valuePreview = item['valuePreview'] as String?;
    final textPreview = item['textPreview'] as String?;

    // Skip malformed entries
    if (roleStr == null ||
        field == null ||
        selector == null ||
        matched == null) {
      continue;
    }

    BindingRoleDto? role;
    if (roleStr == 'input') {
      role = BindingRoleDto.input;
    } else if (roleStr == 'trigger') {
      role = BindingRoleDto.trigger;
    } else if (roleStr == 'output') {
      role = BindingRoleDto.output;
    } else {
      // Unknown role, skip
      continue;
    }

    probes.add(
      ControlledEmbedSelectorProbe(
        role: role,
        field: field,
        selector: selector,
        matched: matched,
        valuePreview: valuePreview ?? '',
        textPreview: textPreview ?? '',
      ),
    );
  }

  return probes;
}

/// Preview a debug value with length truncation and newline normalization.
///
/// - Replaces all newline variants (`\n`, `\r\n`, `\r`) with single space.
/// - Truncates to `max` characters (default 160), adding ellipsis if truncated.
String previewDebugValue(String value, {int max = 160}) {
  if (value.isEmpty) return '';

  // Normalize newlines: replace all newline variants with space
  // Then compress multiple spaces into one
  final normalized = value
      .replaceAll(RegExp(r'[\n\r]+'), ' ')
      .replaceAll(RegExp(r' +'), ' ')
      .trim();

  if (normalized.length <= max) {
    return normalized;
  }

  // Truncate to max-1 to account for the ellipsis character
  return '${normalized.substring(0, max - 1)}…';
}
