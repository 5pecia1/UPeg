/// Controlled Embed form and canonical outputs. Browser sessions belong to
/// the application service. Run shares the Rust dispatcher with attached CLI
/// calls; Debug displays that session without reloading it.
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/features/controlled_embed/providers.dart';
import 'package:upeg/src/features/controlled_embed/session_service.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/rust/api/embed.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/state/controlled_embed_settings_provider.dart';
import 'package:upeg/src/state/selector_bindings_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/controlled_embed/debug_modal.dart';
import 'package:upeg/src/widgets/controlled_embed/runner.dart';
import 'package:upeg/src/widgets/controlled_embed/settings.dart'
    show ResolvedBrowserSettings, resolveBrowserSettings;
import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';
import 'package:upeg/src/widgets/expanded_modal/generic_form.dart';
import 'package:url_launcher/url_launcher.dart';

const double _kTileBorderRadius = 6;
const double _kTilePadding = 6;
const double _kRunButtonHorizontalPadding = 10;
const double _kDebugButtonHorizontalPadding = 8;
const double _kButtonVerticalPadding = 0;
const double _kButtonMinWidth = 0;
const double _kButtonHeight = 28;
const double _kCompactTextFontSize = 11;
const double _kButtonGap = 4;
const double _kOutputGap = 4;
const double _kOutputRowGap = 4;
const double _kLabelValueGap = 2;

/// Shown when the platform target cannot supply a `WebViewController`
/// (web iframe / Linux external-launcher). Inline Run drives the DOM
/// through a controller, so on these platforms it is disabled and this
/// notice offers the external-browser escape hatch instead. Catalog key
/// — the En/Ko copy lives in upeg-pegboard-ui/src/i18n.rs.
const String kControlledEmbedInlineRunUnsupportedNoticeKey =
    'controlled_embed.inline_run_unsupported';

/// Key for the inline-unsupported notice block (Run disabled state).
const Key kControlledEmbedInlineUnsupportedKey = Key(
  'controlled-embed-inline-unsupported',
);

/// Key for the "open externally" button shown in the unsupported notice.
const Key kControlledEmbedOpenExternallyKey = Key(
  'controlled-embed-open-externally',
);

class ControlledEmbedTile extends ConsumerStatefulWidget {
  const ControlledEmbedTile({
    this.pinKey,
    required this.tool,
    required this.resolution,
    this.outputFields,
    super.key,
  });

  final PinKey? pinKey;
  final ToolDto tool;
  final EmbedResolutionDto resolution;
  final List<OutputFieldDto>? outputFields;

  @override
  ConsumerState<ControlledEmbedTile> createState() =>
      _ControlledEmbedTileState();
}

class _ControlledEmbedTileState extends ConsumerState<ControlledEmbedTile> {
  final GenericFormController _formController = GenericFormController();
  late final ControlledEmbedSessionService _sessions;
  CanonicalToolResult? _observedSessionResult;
  CanonicalToolResult? _lastResult;
  bool _formAllOk = false;
  bool _running = false;
  List<SelectorBindingDto> _bindings = const [];
  ResolvedBrowserSettings? _resolvedSettings;

  PinKey? get _pinKey => widget.pinKey;

  ControlledEmbedSessionEntry? get _sessionEntry {
    final pinKey = _pinKey;
    return pinKey == null
        ? _sessions.entryForTool(ToolId.parse(widget.tool.id))
        : _sessions.entryForPin(pinKey);
  }

  @override
  void initState() {
    super.initState();
    _sessions = ref.read(controlledEmbedSessionsProvider);
    _sessions.addListener(_onSessionChanged);
    _lastResult = _sessionEntry?.lastResult;
    _observedSessionResult = _lastResult;
    // Selector bindings are manifest-time data — load once when the
    // tile mounts. Re-loading the manifest will rebuild the pin
    // (different ToolDto), which mounts a fresh tile.
    final loader = ref.read(selectorBindingsLoaderProvider);
    _bindings = loader(ToolId.parse(widget.tool.id));

    // Browser settings (UA + viewport) are also manifest-level data.
    // Load once alongside bindings.
    final settings = ref.read(controlledEmbedSettingsLoaderProvider)(
      ToolId.parse(widget.tool.id),
    );
    _resolvedSettings = resolveBrowserSettings(settings);
  }

  void _onSessionChanged() {
    final result = _sessionEntry?.lastResult;
    if (!mounted ||
        result == null ||
        identical(result, _observedSessionResult)) {
      return;
    }
    setState(() {
      _observedSessionResult = result;
      _lastResult = result;
    });
  }

  @override
  void dispose() {
    _sessions.removeListener(_onSessionChanged);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    // Native runs use the app-owned provider. Web does not create a native
    // WebView; its host-attach path is handled by the browser surface.
    final supportsInlineRun = ref.watch(controlledEmbedNativeSupportedProvider);
    final canRun =
        supportsInlineRun &&
        !_running &&
        (widget.tool.inputFields.isEmpty || _formAllOk);

    return Shortcuts(
      shortcuts: const <ShortcutActivator, Intent>{
        SingleActivator(LogicalKeyboardKey.f4): _RunIntent(),
      },
      child: Actions(
        actions: <Type, Action<Intent>>{
          _RunIntent: CallbackAction<_RunIntent>(
            onInvoke: (_) {
              if (canRun) {
                // ignore: discarded_futures — by design.
                unawaited(_handleRun());
              }
              return null;
            },
          ),
        },
        child: Focus(
          child: Container(
            decoration: BoxDecoration(
              color: tokens.surface,
              border: Border.all(color: tokens.lineSoft),
              borderRadius: BorderRadius.circular(_kTileBorderRadius),
            ),
            padding: const EdgeInsets.all(_kTilePadding),
            child: Stack(
              key: const Key('controlled-embed-tile-stack'),
              clipBehavior: Clip.hardEdge,
              children: [
                // COCKPIT — form + Run + outputs. Sized to the pin
                // body; compact layout fits in a U2 (118 px) or
                // U2T (244 px) cell.
                Positioned.fill(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      if (widget.tool.inputFields.isNotEmpty)
                        Flexible(
                          child: GenericFormWidget(
                            tool: widget.tool,
                            controller: _formController,
                            onValidationChanged: (ok) =>
                                setState(() => _formAllOk = ok),
                          ),
                        ),
                      // Wrap (not Row) so a narrow pin cell (e.g. a U2
                      // 168px column) can never RenderFlex-overflow: the
                      // Debug button drops to the next line instead of
                      // painting a yellow-black overflow strip.
                      Wrap(
                        alignment: WrapAlignment.end,
                        spacing: _kButtonGap,
                        runSpacing: _kButtonGap,
                        children: [
                          FilledButton(
                            key: const Key('controlled-embed-run-btn'),
                            style: FilledButton.styleFrom(
                              padding: const EdgeInsets.symmetric(
                                horizontal: _kRunButtonHorizontalPadding,
                                vertical: _kButtonVerticalPadding,
                              ),
                              minimumSize: const Size(
                                _kButtonMinWidth,
                                _kButtonHeight,
                              ),
                              tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                              textStyle: const TextStyle(
                                fontSize: _kCompactTextFontSize,
                              ),
                            ),
                            onPressed: canRun ? _handleRun : null,
                            child: Text(_running ? '…' : 'Run · F4'),
                          ),
                          OutlinedButton(
                            key: const Key('controlled-embed-debug-open'),
                            style: OutlinedButton.styleFrom(
                              padding: const EdgeInsets.symmetric(
                                horizontal: _kDebugButtonHorizontalPadding,
                                vertical: _kButtonVerticalPadding,
                              ),
                              minimumSize: const Size(
                                _kButtonMinWidth,
                                _kButtonHeight,
                              ),
                              tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                              textStyle: const TextStyle(
                                fontSize: _kCompactTextFontSize,
                              ),
                            ),
                            onPressed: canRun ? _handleDebug : null,
                            child: Text(
                              t(ref, 'controlled_embed.debug_button'),
                            ),
                          ),
                        ],
                      ),
                      const SizedBox(height: _kOutputGap),
                      Flexible(
                        child: supportsInlineRun
                            ? _CanonicalOutputStrip(
                                result: _lastResult,
                                outputFields:
                                    widget.outputFields ??
                                    widget.tool.outputFields,
                                tokens: tokens,
                              )
                            : _InlineUnsupportedNotice(
                                url: widget.resolution.url,
                                tokens: tokens,
                              ),
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  Future<void> _handleRun() async {
    setState(() => _running = true);
    CanonicalToolResult? nextResult;
    try {
      nextResult = await ref.read(controlledEmbedToolExecutorProvider)(
        toolId: ToolId.parse(widget.tool.id),
        args: _formController.snapshot(),
        boardKey: _pinKey?.$1.value,
        pinId: _pinKey?.$2.value,
      );
    } catch (error) {
      nextResult = _executionErrorResult(error);
    } finally {
      if (mounted) {
        setState(() {
          if (nextResult != null) _lastResult = nextResult;
          _running = false;
        });
      }
    }
  }

  CanonicalToolResult _executionErrorResult(Object error) {
    return CanonicalToolResult(
      ok: false,
      outputs: const <CanonicalOutputEntry>[],
      error: CanonicalToolError(
        code: kControlledEmbedExecutionErrorCode,
        message: error.toString(),
      ),
    );
  }

  /// Opens the debug modal without executing or altering _lastResult.
  Future<void> _handleDebug() async {
    final inputs = _formController.snapshot().toJsonObject();
    if (!mounted) return;
    await showDialog<void>(
      context: context,
      builder: (context) => ControlledEmbedDebugModal(
        tool: widget.tool,
        resolution: widget.resolution,
        bindings: _bindings,
        initialInputs: inputs,
        pinKey: _pinKey,
        resolvedSettings: _resolvedSettings,
      ),
    );
  }
}

class _RunIntent extends Intent {
  const _RunIntent();
}

/// Shown in place of the output strip when the platform cannot supply a
/// `WebViewController` for inline execution. Explains why Run is disabled
/// and offers an external-browser launch of the embed URL.
class _InlineUnsupportedNotice extends ConsumerWidget {
  const _InlineUnsupportedNotice({required this.url, required this.tokens});

  final String url;
  final UpegTokens tokens;

  Future<void> _openExternally() async {
    final uri = Uri.tryParse(url);
    if (uri == null) return;
    await launchUrl(uri, mode: LaunchMode.externalApplication);
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Column(
      key: kControlledEmbedInlineUnsupportedKey,
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(
          t(ref, kControlledEmbedInlineRunUnsupportedNoticeKey),
          style: TextStyle(color: tokens.fg4, fontSize: _kCompactTextFontSize),
        ),
        const SizedBox(height: _kOutputGap),
        OutlinedButton(
          key: kControlledEmbedOpenExternallyKey,
          style: OutlinedButton.styleFrom(
            padding: const EdgeInsets.symmetric(
              horizontal: _kDebugButtonHorizontalPadding,
              vertical: _kButtonVerticalPadding,
            ),
            minimumSize: const Size(_kButtonMinWidth, _kButtonHeight),
            tapTargetSize: MaterialTapTargetSize.shrinkWrap,
            textStyle: const TextStyle(fontSize: _kCompactTextFontSize),
          ),
          onPressed: _openExternally,
          child: Text(t(ref, 'embed.open_externally')),
        ),
      ],
    );
  }
}

class _CanonicalOutputStrip extends ConsumerWidget {
  const _CanonicalOutputStrip({
    required this.result,
    required this.outputFields,
    required this.tokens,
  });

  final CanonicalToolResult? result;
  final List<OutputFieldDto> outputFields;
  final UpegTokens tokens;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final result = this.result;
    final writer = ref.watch(clipboardWriterProvider);
    if (result == null) {
      return Text(
        t(ref, 'controlled_embed.press_run'),
        style: TextStyle(color: tokens.fg4, fontSize: _kCompactTextFontSize),
        overflow: TextOverflow.ellipsis,
      );
    }
    final errorMessage = result.errorMessage;
    if (!result.ok && errorMessage != null && errorMessage.isNotEmpty) {
      return SingleChildScrollView(
        child: SelectableText(
          errorMessage,
          key: const Key('controlled-embed-error'),
          style: TextStyle(color: tokens.warn, fontSize: _kCompactTextFontSize),
          maxLines: null,
          textAlign: TextAlign.left,
        ),
      );
    }

    final rows = result.displayRows(outputFields);
    return SingleChildScrollView(
      child: Column(
        key: const Key('controlled-embed-outputs'),
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: rows.isEmpty
            ? [
                Text(
                  t(ref, 'controlled_embed.no_outputs'),
                  style: TextStyle(
                    color: tokens.fg4,
                    fontSize: _kCompactTextFontSize,
                  ),
                ),
              ]
            : [
                for (final row in rows)
                  _CanonicalOutputRow(
                    key: ValueKey('controlled-embed-output-${row.id}'),
                    row: row,
                    tokens: tokens,
                    writer: writer,
                  ),
              ],
      ),
    );
  }
}

class _CanonicalOutputRow extends StatelessWidget {
  const _CanonicalOutputRow({
    required this.row,
    required this.tokens,
    required this.writer,
    super.key,
  });

  final CanonicalOutputDisplayRow row;
  final UpegTokens tokens;
  final ClipboardWriter writer;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: _kOutputRowGap),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(
                row.label,
                style: TextStyle(
                  color: tokens.fg4,
                  fontSize: _kCompactTextFontSize,
                ),
              ),
              const Spacer(),
              CopyToClipboardButton(
                key: ValueKey('controlled-embed-output-copy-${row.id}'),
                textToCopy: row.value,
                writer: writer,
              ),
            ],
          ),
          const SizedBox(height: _kLabelValueGap),
          SelectableText(
            row.value,
            key: ValueKey('controlled-embed-output-value-${row.id}'),
            maxLines: null,
            textAlign: TextAlign.left,
            style: TextStyle(color: tokens.fg, fontSize: _kCompactTextFontSize),
          ),
        ],
      ),
    );
  }
}
