/// Debug view of the application-owned browser session. Opening the debugger
/// never creates another page or executes the tool. Run uses the same Rust
/// dispatcher as the normal tile and attached CLI requests.
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/rust/api/embed.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/controlled_embed/debug.dart';
import 'package:upeg/src/features/controlled_embed/providers.dart';
import 'package:upeg/src/features/controlled_embed/session_service.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';

import 'package:upeg/src/widgets/controlled_embed/settings.dart'
    show ResolvedBrowserSettings;
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

// ---------- Keys ----------
const kDebugModalKey = Key('controlled-embed-debug-modal');

// ---------- Widget ----------

/// Split-pane debug workbench for Controlled Embed tools.
///
/// Renders inside a [showDialog] context. Displays the service-owned browser and its execution events.
class ControlledEmbedDebugModal extends ConsumerStatefulWidget {
  const ControlledEmbedDebugModal({
    required this.tool,
    required this.resolution,
    required this.bindings,
    required this.initialInputs,
    this.resolvedSettings,
    this.boardKey,
    super.key,
  });

  /// The tool definition (used for input field metadata).
  final ToolDto tool;

  /// Resolved embed URL for the WebView.
  final EmbedResolutionDto resolution;

  /// Selector bindings for the controlled embed pipeline.
  final List<SelectorBindingDto> bindings;

  /// Current form snapshot (field key → value string).
  final Map<String, Object?> initialInputs;
  final String? boardKey;

  /// Resolved browser settings from the manifest (UA + viewport).
  /// `null` means settings were omitted — preserve default behavior.
  final ResolvedBrowserSettings? resolvedSettings;

  @override
  ConsumerState<ControlledEmbedDebugModal> createState() =>
      _ControlledEmbedDebugModalState();
}

class _ControlledEmbedDebugModalState
    extends ConsumerState<ControlledEmbedDebugModal> {
  late final ControlledEmbedSessionService _sessions;
  ControlledEmbedSessionEntry? _session;
  VoidCallback? _releaseDebugger;
  Object? _openError;
  bool _viewReady = false;
  bool _isRunning = false;
  final List<ControlledEmbedDebugEvent> _events = [];
  Map<String, String> _latestOutputs = {};

  ToolId get _toolId => ToolId.parse(widget.tool.id);

  @override
  void initState() {
    super.initState();
    _sessions = ref.read(controlledEmbedSessionsProvider);
    _sessions.addListener(_onSessionChanged);
    unawaited(_prepareSession());
  }

  Future<void> _prepareSession() async {
    try {
      final session = await _sessions.ensure(
        ControlledEmbedSessionSpec(
          toolId: _toolId,
          url: widget.resolution.url,
          settings: widget.resolvedSettings,
        ),
      );
      if (!mounted) return;
      _session = session;
      _releaseDebugger = _sessions.reserveDebugger(session);
      // Unmount the hidden view before displaying the same controller here.
      await WidgetsBinding.instance.endOfFrame;
      if (!mounted) return;
      setState(() => _viewReady = true);
      _onSessionChanged();
    } catch (error) {
      if (mounted) setState(() => _openError = error);
    }
  }

  void _onSessionChanged() {
    if (!mounted) return;
    final session = _session;
    if (session == null) return;
    setState(() {
      _events
        ..clear()
        ..addAll(session.events);
      final result = session.lastResult;
      if (result != null) {
        _latestOutputs = {
          for (final output in result.outputs)
            output.id: output.value.displayText,
        };
      }
    });
  }

  Future<void> _execute() async {
    if (!_viewReady ||
        _isRunning ||
        _session?.phase == ControlledEmbedSessionPhase.running) {
      return;
    }
    setState(() => _isRunning = true);
    try {
      final result = await ref.read(controlledEmbedToolExecutorProvider)(
        toolId: _toolId,
        args: ToolArgs.fromJsonObject(widget.initialInputs),
        boardKey: widget.boardKey,
      );
      if (!mounted) return;
      setState(() {
        _latestOutputs = {
          for (final output in result.outputs)
            output.id: output.value.displayText,
        };
        if (!result.ok) {
          _events.add(
            ControlledEmbedDebugEvent(
              kind: 'error',
              phase: ControlledEmbedDebugPhase.read,
              level: ControlledEmbedDebugLevel.error,
              message: result.error?.message ?? 'Tool execution failed.',
              elapsedMs: 0,
              sequence: _events.length,
            ),
          );
        }
      });
    } catch (error) {
      if (mounted) {
        setState(
          () => _events.add(
            ControlledEmbedDebugEvent(
              kind: 'error',
              phase: ControlledEmbedDebugPhase.read,
              level: ControlledEmbedDebugLevel.error,
              message: error.toString(),
              elapsedMs: 0,
              sequence: _events.length,
            ),
          ),
        );
      }
    } finally {
      if (mounted) setState(() => _isRunning = false);
    }
  }

  void _clearConsole() => _sessions.clearEvents(_toolId);

  void _close() => Navigator.of(context).pop();

  @override
  void dispose() {
    _sessions.removeListener(_onSessionChanged);
    final release = _releaseDebugger;
    if (release != null) scheduleMicrotask(release);
    super.dispose();
  }

  Widget _buildBrowser() {
    if (_openError != null) {
      return Center(
        child: Text(
          t(ref, 'controlled_embed.session_unavailable', {
            'error': _openError.toString(),
          }),
        ),
      );
    }
    if (!_viewReady) return const Center(child: CircularProgressIndicator());
    return ClipRect(
      child: SingleChildScrollView(
        scrollDirection: Axis.horizontal,
        child: SingleChildScrollView(child: _session!.browser.buildView()),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;

    return Dialog(
      key: kDebugModalKey,
      insetPadding: const EdgeInsets.all(16),
      child: ConstrainedBox(
        constraints: const BoxConstraints(
          minWidth: 700,
          minHeight: 500,
          maxWidth: 1200,
          maxHeight: 800,
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            // Header
            _buildHeader(tokens),
            const Divider(height: 1),
            // Split pane: WebView + Console
            Expanded(
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  // Left: WebView pane
                  Expanded(
                    flex: 3,
                    child: Column(
                      children: [
                        Expanded(
                          child: Container(
                            color: tokens.bg2,
                            key: kDebugWebViewPaneKey,
                            child: _buildBrowser(),
                          ),
                        ),
                        // Input snapshot section
                        if (widget.initialInputs.isNotEmpty)
                          _buildInputSnapshot(tokens),
                        // Output preview section
                        if (_latestOutputs.isNotEmpty) _buildOutputs(tokens),
                      ],
                    ),
                  ),
                  const VerticalDivider(width: 1),
                  // Right: Console pane
                  Expanded(flex: 2, child: _buildConsole(tokens)),
                ],
              ),
            ),
            const Divider(height: 1),
            // Footer with action buttons
            _buildFooter(tokens),
          ],
        ),
      ),
    );
  }

  Widget _buildHeader(UpegTokens tokens) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 12, 16, 12),
      child: Row(
        children: [
          Text(
            'Debug: ${widget.tool.label}',
            style: TextStyle(
              color: tokens.fg,
              fontSize: 14,
              fontWeight: FontWeight.w600,
            ),
          ),
          const Spacer(),
          Text(
            '${widget.bindings.length} bindings',
            style: TextStyle(color: tokens.fg3, fontSize: 11),
          ),
        ],
      ),
    );
  }

  Widget _buildInputSnapshot(UpegTokens tokens) {
    return Container(
      key: kDebugInputsKey,
      padding: const EdgeInsets.all(8),
      decoration: BoxDecoration(
        border: Border(top: BorderSide(color: tokens.line)),
        color: tokens.surface,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            'Inputs',
            style: TextStyle(
              color: tokens.fg3,
              fontSize: 10,
              fontWeight: FontWeight.w600,
              letterSpacing: 0.5,
            ),
          ),
          const SizedBox(height: 4),
          ...widget.initialInputs.entries.map(
            (e) => Padding(
              padding: const EdgeInsets.only(bottom: 2),
              child: Text.rich(
                TextSpan(
                  children: [
                    TextSpan(
                      text: '${e.key} · ',
                      style: TextStyle(color: tokens.fg4, fontSize: 11),
                    ),
                    TextSpan(
                      text: previewDebugValue(e.value.toString()),
                      style: TextStyle(color: tokens.fg, fontSize: 11),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildOutputs(UpegTokens tokens) {
    return Container(
      key: kDebugOutputsKey,
      padding: const EdgeInsets.all(8),
      decoration: BoxDecoration(
        border: Border(top: BorderSide(color: tokens.line)),
        color: tokens.surface,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: _latestOutputs.isEmpty
            ? [
                Text(
                  t(ref, 'controlled_embed.no_outputs'),
                  style: TextStyle(color: tokens.fg4, fontSize: 11),
                ),
              ]
            : _latestOutputs.entries
                  .map(
                    (e) => Padding(
                      padding: const EdgeInsets.only(bottom: 2),
                      child: Text.rich(
                        TextSpan(
                          children: [
                            TextSpan(
                              text: '${e.key} · ',
                              style: TextStyle(color: tokens.fg4, fontSize: 11),
                            ),
                            TextSpan(
                              text: previewDebugValue(e.value),
                              style: TextStyle(color: tokens.fg, fontSize: 11),
                            ),
                          ],
                        ),
                      ),
                    ),
                  )
                  .toList(),
      ),
    );
  }

  Widget _buildConsole(UpegTokens tokens) {
    return Container(
      key: kDebugConsolePaneKey,
      color: tokens.bg,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(8, 8, 8, 4),
            child: Text(
              'Console',
              style: TextStyle(
                color: tokens.fg3,
                fontSize: 10,
                fontWeight: FontWeight.w600,
                letterSpacing: 0.5,
              ),
            ),
          ),
          Expanded(
            child: ListView.builder(
              padding: const EdgeInsets.symmetric(horizontal: 8),
              itemCount: _events.length,
              itemBuilder: (_, index) =>
                  _ConsoleRow(event: _events[index], tokens: tokens),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildFooter(UpegTokens tokens) {
    return Padding(
      padding: const EdgeInsets.all(8),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.end,
        children: [
          TextButton(
            key: kDebugClearKey,
            onPressed: _clearConsole,
            child: const Text('Clear'),
          ),
          const SizedBox(width: 8),
          ElevatedButton.icon(
            key: kDebugRunKey,
            onPressed:
                !_viewReady ||
                    _isRunning ||
                    _session?.phase == ControlledEmbedSessionPhase.running
                ? null
                : _execute,
            icon: const Icon(Icons.play_arrow, size: 16),
            label: const Text('Run'),
          ),
          const SizedBox(width: 8),
          ElevatedButton.icon(
            key: kDebugRerunKey,
            onPressed:
                !_viewReady ||
                    _isRunning ||
                    _session?.phase == ControlledEmbedSessionPhase.running
                ? null
                : _execute,
            icon: const Icon(Icons.replay, size: 16),
            label: const Text('Re-run'),
          ),
          const SizedBox(width: 8),
          OutlinedButton(
            key: kDebugCloseKey,
            onPressed: _close,
            child: const Text('Close'),
          ),
        ],
      ),
    );
  }
}

/// Single console row rendering a debug event.
class _ConsoleRow extends StatelessWidget {
  const _ConsoleRow({required this.event, required this.tokens});

  final ControlledEmbedDebugEvent event;
  final UpegTokens tokens;

  Color get _levelColor {
    return switch (event.level) {
      ControlledEmbedDebugLevel.info => tokens.fg4,
      ControlledEmbedDebugLevel.success => tokens.accent,
      ControlledEmbedDebugLevel.warning => tokens.accent2,
      ControlledEmbedDebugLevel.error => tokens.warn,
    };
  }

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 2),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Level indicator
          Container(
            width: 4,
            height: 12,
            margin: const EdgeInsets.only(top: 3, right: 6),
            decoration: BoxDecoration(
              color: _levelColor,
              borderRadius: BorderRadius.circular(2),
            ),
          ),
          // Phase badge
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 1),
            decoration: BoxDecoration(
              color: tokens.surface2,
              borderRadius: BorderRadius.circular(2),
            ),
            child: Text(
              event.phase.name,
              style: TextStyle(
                color: tokens.fg3,
                fontSize: 9,
                fontFamily: upegMonoFontFamily,
                fontWeight: FontWeight.w600,
              ),
            ),
          ),
          const SizedBox(width: 6),
          // Message
          Expanded(
            child: Text(
              event.message,
              style: TextStyle(
                color: event.level == ControlledEmbedDebugLevel.error
                    ? tokens.warn
                    : tokens.fg2,
                fontSize: 11,
                fontFamily: upegMonoFontFamily,
              ),
              overflow: TextOverflow.ellipsis,
            ),
          ),
          // Elapsed time
          if (event.elapsedMs > 0)
            Text(
              '${event.elapsedMs}ms',
              style: TextStyle(
                color: tokens.fg4,
                fontSize: 9,
                fontFamily: upegMonoFontFamily,
              ),
            ),
        ],
      ),
    );
  }
}

// ---------- Pane Keys ----------
const kDebugWebViewPaneKey = Key('controlled-embed-debug-webview-pane');
const kDebugConsolePaneKey = Key('controlled-embed-debug-console-pane');
const kDebugInputsKey = Key('controlled-embed-debug-inputs');
const kDebugOutputsKey = Key('controlled-embed-debug-outputs');
const kDebugRunKey = Key('controlled-embed-debug-run');
const kDebugRerunKey = Key('controlled-embed-debug-rerun');
const kDebugClearKey = Key('controlled-embed-debug-clear');
const kDebugCloseKey = Key('controlled-embed-debug-close');
