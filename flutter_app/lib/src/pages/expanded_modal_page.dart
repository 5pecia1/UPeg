/// Modal that opens when a pin (or palette hit) is activated.
///
/// A centered ~560px card with a kind-badged header, a tool-specific
/// body (bespoke first, generic second), an output region, and a
/// `source / surfaces` footer strip.
///
/// `ExpandedModalPage.open` is preserved as the navigator-push helper
/// so existing call sites (Pin taps, palette hits) keep their
/// shape — the page is now rendered inside a transparent Material
/// scaffold so the dialog-shaped card centers on the viewport with a
/// dimmed backdrop.
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/keyboard/keyboard_command_resolver.dart';
import 'package:upeg/src/pages/embed_page.dart';
import 'package:upeg/src/rust/api/keyboard.dart';
import 'package:upeg/src/rust/api/palette.dart';
import 'package:upeg/src/rust/api/pin_activation.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/rust/api/dispatch_stream.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/dispatch_stream_provider.dart';
import 'package:upeg/src/state/pegboard_mutations_provider.dart';
import 'package:upeg/src/state/pin_activation_provider.dart';
import 'package:upeg/src/state/running_tools_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/approval_confirm_dialog.dart';
import 'package:upeg/src/widgets/controlled_embed/surface.dart';
import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';
import 'package:upeg/src/widgets/expanded_modal/bespoke_forms/registry.dart';
import 'package:upeg/src/widgets/expanded_modal/generic_form.dart';
import 'package:upeg/src/widgets/expanded_modal/kind_badge.dart';
import 'package:upeg/src/widgets/expanded_modal/live_output_tail.dart';
import 'package:upeg/src/widgets/expanded_modal/outcome_block.dart';
import 'package:upeg/src/widgets/expanded_modal/primary_button.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';
import 'package:upeg/src/widgets/palette_overlay.dart';
import 'package:upeg/src/widgets/settings_overlay.dart';

/// Test seam for opening the Settings overlay from modal-level
/// shortcuts. Production delegates to [showSettingsOverlay].
typedef SettingsOverlayLauncher = Future<void> Function(BuildContext context);

final settingsOverlayLauncherProvider = Provider<SettingsOverlayLauncher>(
  (ref) => showSettingsOverlay,
);

/// Footer metadata label prefix catalog keys. Kept as constants so the
/// diagnostic strip has no inline magic strings; the En/Ko copy lives
/// in the Rust catalog (upeg-pegboard-ui/src/i18n.rs).
/// Machine code for a dispatch whose *stream* failed, as opposed to a
/// tool that ran and returned a failure. Distinct so the two are
/// distinguishable in a copied result.
const String _dispatchStreamFailedCode = 'dispatch_stream_failed';

const String _footerSourcePrefixKey = 'modal.footer.source_prefix';
const String _footerToolkitPrefixKey = 'modal.footer.toolkit_prefix';
const String _footerInvokerPrefixKey = 'modal.footer.invoker_prefix';

/// Batch O2 (I13 F3): modal-wide shortcut intent for "+ pin". Routed
/// through `Shortcuts`/`Actions` so bespoke forms (which bind F1+F2
/// only) let F3 bubble up to the modal shell.
class _PinIntent extends Intent {
  const _PinIntent();
}

class _CopyIntent extends Intent {
  const _CopyIntent();
}

class _RunIntent extends Intent {
  const _RunIntent();
}

class _CloseIntent extends Intent {
  const _CloseIntent();
}

/// Where one Run request currently sits.
///
/// Three states rather than a bool because the middle one is real: while
/// the approval dialog is open nothing is dispatching yet, but a second
/// Run must not open a second dialog.
enum _RunPhase {
  idle,
  awaitingApproval,
  dispatching;

  /// Run is refused for anything but [idle] — one request at a time.
  bool get blocksRun => this != _RunPhase.idle;

  /// The spinner belongs to the dispatch itself, not to the question in
  /// front of it.
  bool get showsSpinner => this == _RunPhase.dispatching;
}

class ExpandedModalPage extends ConsumerStatefulWidget {
  const ExpandedModalPage({required this.tool, this.initialInput, super.key});

  final ToolDto tool;

  /// Optional pre-fill consumed once at mount, then forgotten. The
  /// deep-link router (`LaunchIntentApplier`) parses the
  /// `upeg://open?input=…` JSON object and threads it here so the
  /// user opens the modal with their fields already populated.
  final ToolArgs? initialInput;

  /// Helper that pushes this page onto the navigator, used by
  /// Pin taps and palette hits. The `initialInput` flag is reserved
  /// for the deep-link path which builds the page directly.
  static Future<void> open(
    BuildContext context,
    ToolDto tool, {
    ToolArgs? initialInput,
  }) {
    return Navigator.of(context).push<void>(
      PageRouteBuilder<void>(
        opaque: false,
        barrierColor: Colors.black54,
        barrierDismissible: true,
        barrierLabel: MaterialLocalizations.of(
          context,
        ).modalBarrierDismissLabel,
        transitionDuration: Duration.zero,
        reverseTransitionDuration: Duration.zero,
        pageBuilder: (_, _, _) =>
            ExpandedModalPage(tool: tool, initialInput: initialInput),
      ),
    );
  }

  @override
  ConsumerState<ExpandedModalPage> createState() => _ExpandedModalPageState();
}

class _ExpandedModalPageState extends ConsumerState<ExpandedModalPage> {
  final GenericFormController _controller = GenericFormController();
  CanonicalToolResult? _outcome;

  /// Aggregate validity from the [GenericFormWidget]. Run is disabled
  /// (greyed-out, ignores taps) until every required field passes its
  /// validator — toolless tools (`inputFields.isEmpty`) skip the gate.
  bool _formAllOk = false;

  /// Where the current Run request sits. Drives the Run button's
  /// loading state and blocks re-dispatch (F1 / Enter / tap) until the
  /// current run settles.
  _RunPhase _phase = _RunPhase.idle;

  /// Last few lines the running tool has printed, capped. Reset at the
  /// start of every run so the previous run's tail never reads as this
  /// one's progress.
  LiveTail _tail = const LiveTail.empty(maxLines: modalLiveTailMaxLines);

  /// Identity of the in-flight run, so Cancel can name it. Null whenever
  /// nothing is dispatching.
  DispatchRunId? _runId;

  @override
  void initState() {
    super.initState();
    // Apply the deep-link pre-fill before the form first renders — the
    // GenericFormWidget reads through `controller.value(key)` for its
    // initialValue so the user sees seeded text in the very first frame.
    final initial = widget.initialInput;
    if (initial != null && initial.isNotEmpty) {
      _controller.seed(initial, fields: widget.tool.inputFields);
    }
  }

  /// The one funnel every Run in this modal passes through: approval
  /// gate, then one streamed dispatch.
  Future<void> _dispatchToolArgs(ToolArgs args) async {
    // Single-flight: a second Run tap / F1 while the first request is
    // still resolving is ignored so the user can't double-fire a slow
    // tool — nor stack two approval dialogs on one barrier.
    if (_phase.blocksRun) return;
    setState(() => _phase = _RunPhase.awaitingApproval);
    final verdict = await _askApproval();
    if (!mounted) return;
    if (verdict is! ApprovalGranted) {
      setState(() => _phase = _RunPhase.idle);
      return;
    }
    await _streamDispatch(args, approve: verdict.approve);
  }

  Future<ApprovalVerdict> _askApproval() => resolveApprovalBeforeDispatch(
    context: context,
    ref: ref,
    tool: widget.tool,
  );

  Future<void> _streamDispatch(ToolArgs args, {required bool approve}) async {
    final toolId = ToolId.parse(widget.tool.id);
    final dispatch = ref.read(dispatchStreamFnProvider);
    final running = ref.read(runningToolsProvider.notifier);
    final runId = nextDispatchRunId();
    setState(() {
      _phase = _RunPhase.dispatching;
      _runId = runId;
      _tail = const LiveTail.empty(maxLines: modalLiveTailMaxLines);
    });
    final runningLease = running.begin(toolId);
    try {
      final events = dispatch(
        toolId: toolId,
        args: args,
        approve: approve,
        runId: runId,
      );
      await for (final event in events) {
        if (!mounted) continue;
        switch (event) {
          case DispatchStreamEventDto_Chunk(:final chunk):
            setState(() => _tail = _tail.append(chunk));
          case DispatchStreamEventDto_Done(:final result):
            setState(() => _outcome = result);
        }
      }
    } on Object catch (err) {
      // The stream itself failed (the bridge refused the run, the
      // channel broke). Nothing settled it, so the modal has to: an
      // unhandled async error would leave the person looking at a
      // spinner that never resolves.
      if (mounted) {
        setState(
          () => _outcome = CanonicalToolResult(
            ok: false,
            outputs: const <CanonicalOutputEntry>[],
            error: CanonicalToolError(
              code: _dispatchStreamFailedCode,
              message: '$err',
            ),
          ),
        );
      }
    } finally {
      running.end(runningLease);
      if (mounted) {
        setState(() {
          _phase = _RunPhase.idle;
          _runId = null;
        });
      }
    }
  }

  /// Ask the in-flight run to stop. Cancellation is cooperative: the run
  /// still ends with its own `Done` event, so the modal keeps exactly
  /// one completion path.
  void _handleCancel() {
    final runId = _runId;
    if (runId == null) return;
    ref.read(cancelDispatchFnProvider)(runId: runId);
  }

  void _handleRun() => unawaited(_dispatchToolArgs(_controller.snapshot()));

  void _handleBespokeSubmit(ToolArgs args) =>
      unawaited(_dispatchToolArgs(args));

  void _handleCopy() {
    final text = _copyTextForOutcome(_outcome);
    if (text == null) return;
    final writer = ref.read(clipboardWriterProvider);
    unawaited(writer.write(text));
  }

  Future<void> _openPalette() {
    return showPaletteOverlay(context, onPick: _handlePaletteHit);
  }

  void _handlePaletteHit(PaletteHit hit) {
    final toolId = ToolId.parse(hit.id);
    final tool = ref.read(toolByIdProvider(toolId));
    if (tool == null) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            tRead(ref, 'common.unknown_tool', {'tool_id': '$toolId'}),
          ),
        ),
      );
      return;
    }
    // Embed-kind palette hits go to EmbedPage even when the palette
    // was opened from inside a modal — picking transform.tools should
    // never land in another empty form. ControlledEmbed pins go to
    // their full-screen surface (same tile widget as the pin body).
    // Non-embed tools open as a nested ExpandedModalPage like before.
    if (tool.pinKind == PinKindDto.embed) {
      // Use the shared activation policy to distinguish URL-backed
      // embeds from URL-less embed tools.
      final activationFn = ref.read(pinActivationProvider);
      final activation = activationFn(
        toolId: ToolId.parse(tool.id),
        argsJson: ToolArgs.emptyJson,
      );
      if (activation is PinActivationDto_OpenEmbed) {
        unawaited(EmbedPage.open(context, tool));
        return;
      }
      // URL-less embed: fall through to modal.
    }
    if (tool.pinKind == PinKindDto.controlledEmbed) {
      unawaited(ControlledEmbedSurface.open(context, tool));
      return;
    }
    unawaited(ExpandedModalPage.open(context, tool));
  }

  bool _handleAliasKey(KeyEvent event) {
    if (_isPrimarySearchShortcut(event)) {
      return _openPaletteCommand();
    }
    if (_isModalShortcutActionKey(event)) {
      return false;
    }
    final cmd = resolveKeyboardCommand(
      ref,
      event,
      scope: KeyboardScopeDto.detail,
    );
    if (cmd == null) return false;
    if (primaryFocusIsEditableText()) return false;
    return switch (cmd) {
      KeyboardCommandDto_Search() => _openPaletteCommand(),
      KeyboardCommandDto_Run() => _runCommand(),
      KeyboardCommandDto_Close() => _closeCommand(),
      KeyboardCommandDto_OpenSettings() => _openSettingsCommand(),
      _ => false,
    };
  }

  bool _isPrimarySearchShortcut(KeyEvent event) {
    return isPrimaryKeyboardShortcut(event, 'k') ||
        isPrimaryKeyboardShortcut(event, 'K');
  }

  bool _isModalShortcutActionKey(KeyEvent event) {
    if (!isKeyboardPress(event)) return false;
    final key = event.logicalKey;
    return key == LogicalKeyboardKey.f1 ||
        key == LogicalKeyboardKey.enter ||
        key == LogicalKeyboardKey.f2 ||
        key == LogicalKeyboardKey.escape ||
        key == LogicalKeyboardKey.f3;
  }

  bool _openPaletteCommand() {
    unawaited(_openPalette());
    return true;
  }

  bool _runCommand() {
    if (widget.tool.inputFields.isEmpty || _formAllOk) {
      _handleRun();
    }
    return true;
  }

  bool _closeCommand() {
    Navigator.of(context).pop();
    return true;
  }

  bool _openSettingsCommand() {
    final openSettings = ref.read(settingsOverlayLauncherProvider);
    unawaited(openSettings(context));
    return true;
  }

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    final tool = widget.tool;
    final toolId = ToolId.parse(tool.id);
    final bespoke = bespokeFor(toolId);
    return Scaffold(
      backgroundColor: Colors.transparent,
      body: Shortcuts(
        shortcuts: const <ShortcutActivator, Intent>{
          SingleActivator(LogicalKeyboardKey.f1): _RunIntent(),
          SingleActivator(LogicalKeyboardKey.enter): _RunIntent(),
          SingleActivator(LogicalKeyboardKey.f2): _CopyIntent(),
          SingleActivator(LogicalKeyboardKey.escape): _CloseIntent(),
          SingleActivator(LogicalKeyboardKey.f3): _PinIntent(),
        },
        child: Actions(
          actions: <Type, Action<Intent>>{
            _CopyIntent: CallbackAction<_CopyIntent>(
              onInvoke: (_) {
                _handleCopy();
                return null;
              },
            ),
            _RunIntent: CallbackAction<_RunIntent>(
              onInvoke: (_) {
                if (tool.inputFields.isEmpty || _formAllOk) {
                  _handleRun();
                }
                return null;
              },
            ),
            _CloseIntent: CallbackAction<_CloseIntent>(
              onInvoke: (_) {
                Navigator.of(context).pop();
                return null;
              },
            ),
            _PinIntent: CallbackAction<_PinIntent>(
              onInvoke: (_) {
                // Fire-and-forget: `_handlePin` awaits internally for
                // the SnackBar gating, but the CallbackAction surface
                // must return synchronously.
                // ignore: discarded_futures — by design.
                _handlePin(context: context, ref: ref, tool: tool);
                return null;
              },
            ),
          },
          child: Focus(
            autofocus: true,
            onKeyEvent: (_, event) => _handleAliasKey(event)
                ? KeyEventResult.handled
                : KeyEventResult.ignored,
            child: SafeArea(
              child: Center(
                child: ConstrainedBox(
                  constraints: const BoxConstraints(
                    maxWidth: 560,
                    maxHeight: 640,
                  ),
                  child: Container(
                    margin: const EdgeInsets.all(16),
                    decoration: BoxDecoration(
                      color: tokens.surface,
                      border: Border.all(color: tokens.line),
                      borderRadius: BorderRadius.circular(8),
                      boxShadow: const [
                        BoxShadow(
                          color: Color(0x66000000),
                          blurRadius: 60,
                          offset: Offset(0, 20),
                        ),
                      ],
                    ),
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        _ModalHeader(tokens: tokens, tool: tool),
                        Flexible(
                          child: SingleChildScrollView(
                            padding: const EdgeInsets.fromLTRB(16, 14, 16, 14),
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.stretch,
                              children: [
                                if (tool.description.isNotEmpty)
                                  Padding(
                                    padding: const EdgeInsets.only(bottom: 12),
                                    child: Text(
                                      tool.description,
                                      style: TextStyle(
                                        color: tokens.fg3,
                                        fontSize: 11,
                                        height: 1.45,
                                      ),
                                    ),
                                  ),
                                if (bespoke != null)
                                  bespoke(
                                    context,
                                    tool,
                                    _handleBespokeSubmit,
                                    _outcome?.primaryOutputText,
                                    widget.initialInput,
                                  )
                                else ...[
                                  GenericFormWidget(
                                    tool: tool,
                                    controller: _controller,
                                    onValidationChanged: (allOk) =>
                                        setState(() => _formAllOk = allOk),
                                  ),
                                  const SizedBox(height: 8),
                                  Align(
                                    alignment: Alignment.centerRight,
                                    child: ExpandedModalPrimaryButton(
                                      key: const Key('expanded-modal-run-btn'),
                                      label: t(ref, 'modal.action.run_short'),
                                      trailingHint: '[F1]',
                                      // Toolless tools (no required fields)
                                      // can always Run; otherwise gate on the
                                      // aggregate validation result. A request
                                      // in flight also disables Run so a slow
                                      // tool can't be double-fired.
                                      enabled:
                                          (tool.inputFields.isEmpty ||
                                              _formAllOk) &&
                                          !_phase.blocksRun,
                                      loading: _phase.showsSpinner,
                                      onPressed: _handleRun,
                                    ),
                                  ),
                                ],
                                if (_phase.showsSpinner) ...[
                                  const SizedBox(height: 12),
                                  _LiveRunBlock(
                                    tail: _tail,
                                    tokens: tokens,
                                    onCancel: _handleCancel,
                                  ),
                                ],
                                if (_outcome != null) ...[
                                  const SizedBox(height: 14),
                                  OutcomeBlock(
                                    outcome: _outcome!,
                                    outputFields: tool.outputFields,
                                    tokens: tokens,
                                  ),
                                ],
                              ],
                            ),
                          ),
                        ),
                        _ModalFooter(tokens: tokens, tool: tool),
                      ],
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// In-flight run strip: the tool's last few output lines plus the way
/// out of a run that is taking too long.
///
/// Only mounted while a dispatch is actually running, so an idle modal
/// looks exactly as it did before streaming existed.
class _LiveRunBlock extends ConsumerWidget {
  const _LiveRunBlock({
    required this.tail,
    required this.tokens,
    required this.onCancel,
  });

  final LiveTail tail;
  final UpegTokens tokens;
  final VoidCallback onCancel;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Container(
      key: modalLiveOutputTailKey,
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: tokens.bg2,
        border: Border.all(color: tokens.line),
        borderRadius: BorderRadius.circular(UpegSizing.radius2),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          Row(
            children: [
              Flexible(
                child: Text(
                  t(ref, 'modal.pill.live_output'),
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    fontFamily: upegMonoFontFamily,
                    fontFamilyFallback: upegMonoFontFamilyFallback,
                    fontSize: 10,
                    letterSpacing: 0.4,
                    color: tokens.fg3,
                  ),
                ),
              ),
              const Spacer(),
              TextButton(
                key: const Key('expanded-modal-cancel-btn'),
                style: TextButton.styleFrom(
                  foregroundColor: tokens.warn,
                  padding: const EdgeInsets.symmetric(
                    horizontal: 8,
                    vertical: 4,
                  ),
                  minimumSize: const Size(0, 24),
                  textStyle: const TextStyle(
                    fontSize: 11,
                    fontWeight: FontWeight.w500,
                  ),
                ),
                onPressed: onCancel,
                child: Text(t(ref, 'modal.action.cancel_run')),
              ),
            ],
          ),
          if (tail.isNotEmpty) ...[
            const SizedBox(height: 6),
            LiveOutputTailView(tail: tail, fontSize: 11),
          ],
        ],
      ),
    );
  }
}

String? _copyTextForOutcome(CanonicalToolResult? outcome) {
  if (outcome == null) return null;
  final candidates = [
    outcome.errorMessage,
    outcome.primaryOutputText,
    outcome.outputs.isEmpty ? null : outcome.canonicalJsonText(),
  ];
  for (final candidate in candidates) {
    if (candidate != null && candidate.isNotEmpty) return candidate;
  }
  return null;
}

class _ModalHeader extends ConsumerWidget {
  const _ModalHeader({required this.tokens, required this.tool});
  final UpegTokens tokens;
  final ToolDto tool;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Container(
      padding: const EdgeInsets.fromLTRB(16, 14, 16, 14),
      decoration: BoxDecoration(
        border: Border(bottom: BorderSide(color: tokens.lineSoft)),
      ),
      child: Row(
        children: [
          Icon(iconForTool(tool), color: tokens.accent, size: 14),
          const SizedBox(width: 10),
          Text(
            tool.label,
            style: TextStyle(
              fontFamily: upegMonoFontFamily,
              fontFamilyFallback: upegMonoFontFamilyFallback,
              fontSize: 13,
              color: tokens.fg,
              fontWeight: FontWeight.w500,
            ),
          ),
          if (tool.label != tool.id) ...[
            const SizedBox(width: 8),
            Flexible(
              child: Text(
                tool.id,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  fontFamily: upegMonoFontFamily,
                  fontFamilyFallback: upegMonoFontFamilyFallback,
                  fontSize: 10,
                  color: tokens.fg4,
                  letterSpacing: 0.4,
                ),
              ),
            ),
          ],
          const SizedBox(width: 8),
          KindBadge(pinKind: tool.pinKind),
          Expanded(child: Container()),
          OutlinedButton(
            key: const Key('expanded-modal-close-btn'),
            style: OutlinedButton.styleFrom(
              foregroundColor: tokens.fg2,
              side: BorderSide(color: tokens.line),
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(UpegSizing.radius1),
              ),
              minimumSize: const Size(0, 28),
              textStyle: const TextStyle(
                fontSize: 11,
                fontWeight: FontWeight.w500,
              ),
            ),
            onPressed: () => Navigator.of(context).pop(),
            child: Text(t(ref, 'modal.header.close')),
          ),
        ],
      ),
    );
  }
}

/// Pin the active tool onto the active board.
///
/// Reads [`currentBoardKeyProvider`] for the destination, dispatches through
/// the central pegboard mutation path, and shows a `SnackBar` reporting
/// success/failure. When no board is active (cold boot before `restore()`),
/// surfaces the "no active board" message instead of silently dropping the
/// click.
Future<void> _handlePin({
  required BuildContext context,
  required WidgetRef ref,
  required ToolDto tool,
}) async {
  final messenger = ScaffoldMessenger.of(context);
  final boardKey = ref.read(currentBoardKeyProvider);
  if (boardKey == null) {
    messenger.showSnackBar(
      SnackBar(content: Text(tRead(ref, 'modal.pin.no_active_board'))),
    );
    return;
  }
  final mutations = ref.read(pegboardMutationsProvider);
  try {
    await mutations.pin(boardKey, ToolId.parse(tool.id));
  } on Object catch (err) {
    messenger.showSnackBar(
      SnackBar(content: Text(tRead(ref, 'modal.pin.failed', {'msg': '$err'}))),
    );
    return;
  }
  messenger.showSnackBar(
    SnackBar(
      content: Text(
        tRead(ref, 'modal.pin.success', {
          'tool_id': tool.id,
          'board': '$boardKey',
        }),
      ),
    ),
  );
}

class _ModalFooter extends ConsumerWidget {
  const _ModalFooter({required this.tokens, required this.tool});
  final UpegTokens tokens;
  final ToolDto tool;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Container(
      padding: const EdgeInsets.fromLTRB(16, 10, 16, 10),
      decoration: BoxDecoration(
        border: Border(top: BorderSide(color: tokens.lineSoft)),
      ),
      child: Wrap(
        spacing: 14,
        runSpacing: 4,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          // Source is derived from the tool's real `Source` metadata
          // (`sourceDtoLabel`) — the previous hardcoded
          // `static · #[upeg::tool]` claimed a macro registration that
          // is false for MCP / TOML / WASM tools.
          RichText(
            text: TextSpan(
              children: [
                TextSpan(
                  text: t(ref, _footerSourcePrefixKey),
                  style: TextStyle(color: tokens.fg4, fontSize: 10),
                ),
                TextSpan(
                  text: sourceDtoLabel(tool.source),
                  style: TextStyle(color: tokens.fg3, fontSize: 10),
                ),
              ],
            ),
          ),
          Text(
            '${t(ref, _footerToolkitPrefixKey)}${tool.toolkit}',
            style: TextStyle(color: tokens.fg4, fontSize: 10),
          ),
          // Batch N7 (I16): invoker label round-trips so the user can
          // tell at a glance which back-end runs the tool.
          Text(
            '${t(ref, _footerInvokerPrefixKey)}${invokerDtoLabel(tool.invoker)}',
            style: TextStyle(color: tokens.fg4, fontSize: 10),
          ),
          // Batch O1 (I12): "+ pin" pushes the active tool onto the
          // currently-selected board, then reports success via a SnackBar
          // so the user gets immediate feedback.
          TextButton(
            key: const Key('expanded-modal-pin-btn'),
            style: TextButton.styleFrom(
              foregroundColor: tokens.accent,
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
              minimumSize: const Size(0, 24),
              textStyle: const TextStyle(
                fontSize: 11,
                fontWeight: FontWeight.w500,
              ),
            ),
            onPressed: () => _handlePin(context: context, ref: ref, tool: tool),
            child: Text(t(ref, 'desktop.tab.add_tool')),
          ),
        ],
      ),
    );
  }
}
