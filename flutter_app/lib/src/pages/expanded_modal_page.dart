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
import 'dart:convert';

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
import 'package:upeg/src/state/capability_provider.dart';
import 'package:upeg/src/state/dispatch_stream_provider.dart';
import 'package:upeg/src/state/diagnostics_provider.dart';
import 'package:upeg/src/widgets/diagnostics_section.dart';
import 'package:upeg/src/state/external_readiness_provider.dart';
import 'package:upeg/src/state/pin_activation_provider.dart';
import 'package:upeg/src/state/running_tools_provider.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/state/presentation_call_origin.dart';
import 'package:upeg/src/state/presentation_resolver_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/approval_confirm_dialog.dart';
import 'package:upeg/src/widgets/controlled_embed/surface.dart';
import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';
import 'package:upeg/src/widgets/expanded_modal/bespoke_forms/registry.dart';
import 'package:upeg/src/widgets/expanded_modal/generic_form.dart';
import 'package:upeg/src/widgets/expanded_modal/external_readiness_panel.dart';
import 'package:upeg/src/widgets/expanded_modal/external_run_controller.dart';
import 'package:upeg/src/widgets/expanded_modal/live_output_tail.dart';
import 'package:upeg/src/widgets/expanded_modal/modal_chrome.dart';
import 'package:upeg/src/widgets/expanded_modal/outcome_block.dart';
import 'package:upeg/src/widgets/expanded_modal/primary_button.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';
import 'package:upeg/src/widgets/expanded_modal/presentation_table.dart';
import 'package:upeg/src/widgets/palette_overlay.dart';
import 'package:upeg/src/widgets/settings_overlay.dart';

part '../widgets/expanded_modal/live_run_block.dart';
part '../widgets/expanded_modal/modal_shortcuts.dart';

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
  const ExpandedModalPage({
    required this.tool,
    this.initialInput,
    this.pinKey,
    this.presentationCalls,
    this.presentationCall,
    this.presentationAction,
    this.collapseBoundInputs = false,
    this.onRefresh,
    this.autoRunRead = false,
    super.key,
  });

  final ToolDto tool;

  /// Optional pre-fill consumed once at mount, then forgotten. The
  /// deep-link router (`LaunchIntentApplier`) parses the
  /// `upeg://open?input=…` JSON object and threads it here so the
  /// user opens the modal with their fields already populated.
  final ToolArgs? initialInput;
  final PinKey? pinKey;
  final PresentationCallOriginController? presentationCalls;
  final PresentationCallIdentity? presentationCall;
  final PresentationActionDto? presentationAction;
  final bool collapseBoundInputs;
  final ValueChanged<PresentationRefreshRequest>? onRefresh;
  final bool autoRunRead;

  /// Helper that pushes this page onto the navigator, used by
  /// Pin taps and palette hits. The `initialInput` flag is reserved
  /// for the deep-link path which builds the page directly.
  static Future<void> open(
    BuildContext context,
    ToolDto tool, {
    ToolArgs? initialInput,
    PinKey? pinKey,
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
        pageBuilder: (_, _, _) => ExpandedModalPage(
          tool: tool,
          initialInput: initialInput,
          pinKey: pinKey,
        ),
      ),
    );
  }

  @override
  ConsumerState<ExpandedModalPage> createState() => _ExpandedModalPageState();
}

class _ExpandedModalPageState extends ConsumerState<ExpandedModalPage> {
  final GenericFormController _controller = GenericFormController();
  final PresentationCallOriginController _presentationCalls =
      PresentationCallOriginController();
  PresentationCallIdentity? _presentationCall;
  CanonicalToolResult? _outcome;
  DispatchRunId? _outcomeRunId;

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
  bool _cancelRequested = false;

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
    if (_shouldAutoRunRead) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) unawaited(_dispatchToolArgs(_controller.snapshot()));
      });
    }
  }

  bool get _shouldAutoRunRead =>
      widget.autoRunRead ||
      (widget.tool.effect == ToolEffectDto.read &&
          widget.tool.presentation != null &&
          !widget.tool.inputFields.any((field) => field.required_));

  /// The one funnel every Run in this modal passes through: approval
  /// gate, then one streamed dispatch.
  Future<void> _dispatchToolArgs(
    ToolArgs args, {
    PresentationCallIdentity? forcedCall,
    PresentationRefreshRun? refreshRun,
  }) async {
    // Single-flight: a second Run tap / F1 while the first request is
    // still resolving is ignored so the user can't double-fire a slow
    // tool — nor stack two approval dialogs on one barrier.
    if (_phase.blocksRun) return;
    setState(() => _phase = _RunPhase.awaitingApproval);
    final external = ExternalRunController(ref);
    if (widget.tool.invoker == InvokerDto.external_) {
      try {
        final inspection = await external.inspect(widget.tool);
        if (!mounted) return;
        if (inspection is ExternalReadinessHostUnpaired ||
            inspection is ExternalReadinessInspected && inspection.blocksRun) {
          setState(() => _phase = _RunPhase.idle);
          return;
        }
      } on Object {
        if (mounted) setState(() => _phase = _RunPhase.idle);
        return;
      }
    }
    final verdict = await _askApproval();
    if (!mounted) return;
    if (verdict is! ApprovalGranted) {
      setState(() => _phase = _RunPhase.idle);
      if (refreshRun != null) {
        (widget.presentationCalls ?? _presentationCalls).refreshFailed(
          refreshRun,
        );
      }
      return;
    }
    if (external.usesRemoteHost(widget.tool)) {
      await _attachDispatch(external, args);
      return;
    }
    if (ref.read(isWasmRuntimeProvider)) {
      await _webDispatch(args, approve: verdict.approve);
      return;
    }
    await _streamDispatch(
      args,
      approve: verdict.approve,
      forcedCall: forcedCall,
      refreshRun: refreshRun,
    );
  }

  Future<ApprovalVerdict> _askApproval() => resolveApprovalBeforeDispatch(
    context: context,
    ref: ref,
    tool: widget.tool,
  );

  Future<void> _attachDispatch(
    ExternalRunController external,
    ToolArgs args,
  ) async {
    setState(() => _phase = _RunPhase.dispatching);
    final result = await external.dispatchRemote(
      widget.tool,
      args,
      pinKey: widget.pinKey,
    );
    if (mounted) {
      setState(() {
        _phase = _RunPhase.idle;
        _outcome = result;
        _outcomeRunId = null;
      });
    }
  }

  Future<void> _webDispatch(ToolArgs args, {required bool approve}) async {
    setState(() => _phase = _RunPhase.dispatching);
    final result = await ref.read(toolkitDispatchProvider)(
      toolId: ToolId.parse(widget.tool.id),
      args: args,
      boardKey:
          widget.pinKey?.$1.value ?? ref.read(currentBoardKeyProvider)?.value,
      pinId: widget.pinKey?.$2.value,
      approve: approve,
    );
    if (!mounted) return;
    setState(() {
      _phase = _RunPhase.idle;
      _outcome = result;
      _outcomeRunId = null;
      _runId = null;
    });
  }

  Future<void> _streamDispatch(
    ToolArgs args, {
    required bool approve,
    PresentationCallIdentity? forcedCall,
    PresentationRefreshRun? refreshRun,
  }) async {
    final toolId = ToolId.parse(widget.tool.id);
    final dispatch = ref.read(dispatchStreamFnProvider);
    final running = ref.read(runningToolsProvider.notifier);
    final runId = nextDispatchRunId();
    final calls = widget.presentationCalls ?? _presentationCalls;
    final firstInjectedCall = _presentationCall == null
        ? widget.presentationCall
        : null;
    final call =
        forcedCall ??
        firstInjectedCall ??
        (calls.origin == null
            ? calls.begin(
                toolId: widget.tool.id,
                args: args,
                host: ref.read(currentBoardKeyProvider)?.value,
              )
            : calls.beginFollowup(
                toolId: widget.tool.id,
                args: args,
                host: ref.read(currentBoardKeyProvider)?.value,
              ));
    _presentationCall = call;
    setState(() {
      _phase = _RunPhase.dispatching;
      _runId = runId;
      _outcomeRunId = null;
      _tail = const LiveTail.empty(maxLines: modalLiveTailMaxLines);
      _cancelRequested = false;
    });
    CanonicalToolResult? completed;
    var streamFailed = false;
    // The modal can be opened from palette/deep links without a placement;
    // it intentionally does not claim a board-pin running indicator.
    final runningLease = widget.pinKey == null
        ? null
        : running.begin(widget.pinKey!);
    try {
      final events = dispatch(
        pinKey: widget.pinKey,
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
            completed = result;
            ExternalRunController(
              ref,
            ).invalidateReadinessOnMissing(widget.tool, result);
            final accepted = refreshRun == null
                ? calls.acceptResult(call)
                : calls.acceptRefresh(refreshRun);
            if (accepted && (refreshRun == null || result.ok)) {
              setState(() {
                _outcome = result;
                _outcomeRunId = runId;
              });
            }
        }
      }
    } on Object catch (err) {
      streamFailed = true;
      // The stream itself failed (the bridge refused the run, the
      // channel broke). Nothing settled it, so the modal has to: an
      // unhandled async error would leave the person looking at a
      // spinner that never resolves.
      if (mounted && refreshRun == null) {
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
      final action = widget.presentationAction;
      if (action?.onSuccess == ActionSuccessDto.refreshOrigin) {
        final result = completed;
        if (streamFailed || _cancelRequested || result == null) {
          calls.writeUnconfirmed(call);
        } else if (!result.ok) {
          calls.writeFailed(call);
        } else {
          final request = calls.writeSucceeded(
            write: call,
            refreshOrigin: true,
          );
          if (request != null) widget.onRefresh?.call(request);
        }
      } else if (refreshRun != null &&
          (streamFailed || completed == null || !completed.ok)) {
        calls.refreshFailed(refreshRun);
      }
      if (runningLease != null) running.end(runningLease);
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
    _cancelRequested = true;
    ref.read(cancelDispatchFnProvider)(runId: runId);
  }

  void _handleRun() => unawaited(_dispatchToolArgs(_controller.snapshot()));

  void _handleBespokeSubmit(ToolArgs args) =>
      unawaited(_dispatchToolArgs(args));

  void _handleRefreshRequest(PresentationRefreshRequest request) {
    unawaited(_runRefreshRequest(request));
  }

  Future<void> _runRefreshRequest(PresentationRefreshRequest request) async {
    final calls = widget.presentationCalls ?? _presentationCalls;
    final currentHost = ref.read(currentBoardKeyProvider)?.value;
    if (request.origin.toolId != widget.tool.id ||
        request.origin.host != currentHost) {
      final rejected = calls.beginRefresh(request);
      if (rejected != null) calls.refreshFailed(rejected);
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(
              tRead(ref, 'modal.presentation.board_changed_not_refreshed'),
            ),
          ),
        );
      }
      return;
    }
    final refresh = calls.beginRefresh(request);
    if (refresh == null) return;
    await _dispatchToolArgs(
      request.origin.args,
      forcedCall: refresh.call,
      refreshRun: refresh,
    );
    if (mounted && calls.writeStatus is PresentationRefreshFailed) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(tRead(ref, 'modal.presentation.write_refresh_failed')),
        ),
      );
    }
  }

  /// A presentation action only opens the target's existing form. Its Rust
  /// resolver preserves JSON types and diagnoses incomplete/invalid bindings;
  /// the normal Run path still owns approval and dispatch.
  void _handlePresentationAction(
    PresentationActionDto action,
    PresentationTableRow? row, {
    bool singleClick = false,
  }) {
    final outcome = _outcome;
    if (outcome == null) return;
    final resolved = ref.read(presentationBindingsResolverProvider)(
      toolId: widget.tool.id,
      actionId: action.id,
      currentInputsJson: _controller.snapshot().encodeJson(),
      selectedRowJson: row?.rawJson,
      outputsJson: jsonEncode(outcome.jsonValues),
    );
    if (resolved.diagnostics.isNotEmpty) {
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(resolved.diagnostics.join('\n'))));
      return;
    }
    final view = ref.read(presentationViewResolverProvider)(
      toolId: widget.tool.id,
      outputsJson: jsonEncode(outcome.jsonValues),
    );
    final collapseBoundInputs =
        view.diagnostics.isEmpty &&
        (view.title != null ||
            view.status != null ||
            view.summary.isNotEmpty ||
            view.notices.isNotEmpty ||
            view.detail != null);
    final target = ref.read(toolByIdProvider(ToolId.parse(action.targetTool)));
    final initial =
        ToolArgs.tryDecodeObject(resolved.valuesJson) ?? ToolArgs.empty;
    if (target == null) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            tRead(ref, 'common.unknown_tool', {'tool_id': action.targetTool}),
          ),
        ),
      );
      return;
    }
    // Single-click navigation is intentionally narrow: the target must have
    // declared itself read-only and every required value must already have
    // been validated and bound. Any write, unknown effect, or incomplete
    // target stays on the explicit action + Run path.
    if (singleClick &&
        (target.effect != ToolEffectDto.read ||
            resolved.unboundRequiredInputs.isNotEmpty)) {
      return;
    }
    final calls = widget.presentationCalls ?? _presentationCalls;
    final active = widget.presentationCall ?? _presentationCall;
    if (active != null) {
      final host = ref.read(currentBoardKeyProvider)?.value;
      if (!calls.activeHostMatches(host)) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(
              tRead(ref, 'modal.presentation.board_changed_run_again'),
            ),
          ),
        );
        return;
      }
      if (action.scope == ActionScopeDto.row &&
          !calls.captureOriginForRowAction(
            active,
            originEffectIsRead: widget.tool.effect == ToolEffectDto.read,
          )) {
        return;
      }
      final childCall = calls.beginFollowup(
        toolId: target.id,
        args: initial,
        host: host,
      );
      unawaited(
        _openPresentationChild(
          target: target,
          initial: initial,
          calls: calls,
          parentCall: active,
          childCall: childCall,
          action: action,
          collapseBoundInputs: collapseBoundInputs,
          autoRunRead:
              target.effect == ToolEffectDto.read &&
              resolved.unboundRequiredInputs.isEmpty,
        ),
      );
      return;
    }
    // An outcome without an invocation identity is a persisted ToolId-only
    // result. It may stay visible, but cannot authorize a follow-up action.
  }

  Future<void> _openPresentationChild({
    required ToolDto target,
    required ToolArgs initial,
    required PresentationCallOriginController calls,
    required PresentationCallIdentity parentCall,
    required PresentationCallIdentity childCall,
    required PresentationActionDto action,
    required bool collapseBoundInputs,
    required bool autoRunRead,
  }) async {
    await Navigator.of(context).push<void>(
      PageRouteBuilder<void>(
        opaque: false,
        pageBuilder: (_, _, _) => ExpandedModalPage(
          tool: target,
          initialInput: initial,
          presentationCalls: calls,
          presentationCall: childCall,
          presentationAction: action,
          collapseBoundInputs: collapseBoundInputs,
          onRefresh: widget.onRefresh ?? _handleRefreshRequest,
          autoRunRead: autoRunRead,
        ),
      ),
    );
    if (calls.active == childCall) calls.replaceActive(parentCall);
  }

  Future<void> _handleCopy() async {
    final writer = ref.read(clipboardWriterProvider);
    if (_outcome?.ok == false && _outcomeRunId != null) {
      try {
        final api = ref.read(diagnosticsApiProvider);
        final report = await diagnosticForRun(api, _outcomeRunId!.value);
        if (report != null) {
          await writer.write(await api.export(report.id, debug: false));
          return;
        }
      } on Object {
        // Results can also come from a host without retained diagnostics.
      }
    }
    final text = _copyTextForOutcome(_outcome);
    if (text == null) return;
    await writer.write(text);
  }

  Future<void> _openPalette() {
    return showPaletteOverlay(context, onPick: _handlePaletteHit);
  }

  void _handlePaletteHit(PaletteHit hit, PinKey? pinKey) {
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
      unawaited(ControlledEmbedSurface.open(context, tool, pinKey: pinKey));
      return;
    }
    unawaited(ExpandedModalPage.open(context, tool, pinKey: pinKey));
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
    if ((widget.tool.inputFields.isEmpty || _formAllOk) &&
        _readinessAllowsRun()) {
      _handleRun();
    }
    return true;
  }

  bool _readinessAllowsRun() {
    final tool = widget.tool;
    if (tool.invoker != InvokerDto.external_) return true;
    final target = readinessTargetFor(
      tool,
      boardKey: ref.read(currentBoardKeyProvider),
    );
    return externalReadinessAllowsRun(
      ref.read(externalReadinessProvider(target)),
    );
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
    // Presentation actions resolve a statically declared target synchronously
    // on tap. Warm the cached catalogue while the form is visible so the
    // first action cannot observe the FutureProvider's loading frame.
    ref.watch(toolsProvider);
    final tokens = context.upeg;
    final tool = widget.tool;
    final toolId = ToolId.parse(tool.id);
    final bespoke = bespokeFor(toolId);
    final readinessAllowsRun = tool.invoker != InvokerDto.external_
        ? true
        : externalReadinessAllowsRun(
            ref.watch(
              externalReadinessProvider(
                readinessTargetFor(
                  tool,
                  boardKey: ref.watch(currentBoardKeyProvider),
                ),
              ),
            ),
          );
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
                if ((tool.inputFields.isEmpty || _formAllOk) &&
                    readinessAllowsRun) {
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
                // Fire-and-forget: `pinToolFromModal` awaits internally for
                // the SnackBar gating, but the CallbackAction surface
                // must return synchronously.
                // ignore: discarded_futures — by design.
                pinToolFromModal(context: context, ref: ref, tool: tool);
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
              child: LayoutBuilder(
                builder: (context, constraints) {
                  // A resolved collection needs enough room for its list and
                  // selected detail. Plain forms retain the compact dialog.
                  final presentationWidth = tool.presentation == null
                      ? 560.0
                      : 1000.0;
                  final presentationHeight = tool.presentation == null
                      ? 640.0
                      : 720.0;
                  return Center(
                    child: ConstrainedBox(
                      constraints: BoxConstraints(
                        maxWidth: presentationWidth,
                        maxHeight: presentationHeight,
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
                            ExpandedModalHeader(tokens: tokens, tool: tool),
                            Flexible(
                              child: SingleChildScrollView(
                                padding: const EdgeInsets.fromLTRB(
                                  16,
                                  14,
                                  16,
                                  14,
                                ),
                                child: Column(
                                  crossAxisAlignment:
                                      CrossAxisAlignment.stretch,
                                  children: [
                                    if (tool.description.isNotEmpty)
                                      Padding(
                                        padding: const EdgeInsets.only(
                                          bottom: 12,
                                        ),
                                        child: Text(
                                          tool.description,
                                          style: TextStyle(
                                            color: tokens.fg3,
                                            fontSize: 11,
                                            height: 1.45,
                                          ),
                                        ),
                                      ),
                                    ExternalReadinessPanel(tool: tool),
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
                                        boundInputKeys:
                                            widget.collapseBoundInputs
                                            ? widget
                                                      .presentationAction
                                                      ?.bindings
                                                      .map(
                                                        (binding) =>
                                                            binding.target,
                                                      )
                                                      .toSet() ??
                                                  const {}
                                            : const {},
                                        onValidationChanged: (allOk) =>
                                            setState(() => _formAllOk = allOk),
                                      ),
                                      const SizedBox(height: 8),
                                      Align(
                                        alignment: Alignment.centerRight,
                                        child: ExpandedModalPrimaryButton(
                                          key: const Key(
                                            'expanded-modal-run-btn',
                                          ),
                                          label: t(
                                            ref,
                                            'modal.action.run_short',
                                          ),
                                          trailingHint: '[F1]',
                                          // Toolless tools (no required fields)
                                          // can always Run; otherwise gate on the
                                          // aggregate validation result. A request
                                          // in flight also disables Run so a slow
                                          // tool can't be double-fired.
                                          enabled:
                                              (tool.inputFields.isEmpty ||
                                                  _formAllOk) &&
                                              readinessAllowsRun &&
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
                                        tool: tool,
                                        tokens: tokens,
                                        onRowAction: _handlePresentationAction,
                                        onResultAction: (action) =>
                                            _handlePresentationAction(
                                              action,
                                              null,
                                            ),
                                        onReadRowNavigate: (action, row) =>
                                            _handlePresentationAction(
                                              action,
                                              row,
                                              singleClick: true,
                                            ),
                                      ),
                                      if (!_outcome!.ok &&
                                          _outcomeRunId != null)
                                        DiagnosticRunButton(
                                          runId: _outcomeRunId!.value,
                                        ),
                                    ],
                                  ],
                                ),
                              ),
                            ),
                            ExpandedModalFooter(tokens: tokens, tool: tool),
                          ],
                        ),
                      ),
                    ),
                  );
                },
              ),
            ),
          ),
        ),
      ),
    );
  }
}
