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
import 'package:upeg/src/widgets/diagnostics_section.dart';
import 'package:upeg/src/state/external_readiness_provider.dart';
import 'package:upeg/src/state/focused_pin_provider.dart';
import 'package:upeg/src/state/pin_activation_provider.dart';
import 'package:upeg/src/state/running_tools_provider.dart';
import 'package:upeg/src/state/inline_draft_provider.dart';
import 'package:upeg/src/state/last_outcome_provider.dart';
import 'package:upeg/src/widgets/pin_save_status.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/state/presentation_call_origin.dart';
import 'package:upeg/src/state/presentation_resolver_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/approval_confirm_dialog.dart';
import 'package:upeg/src/widgets/controlled_embed/surface.dart';
import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';
import 'package:upeg/src/widgets/copyable_outcome_text.dart';
import 'package:upeg/src/widgets/expanded_modal/bespoke_forms/registry.dart';
import 'package:upeg/src/widgets/expanded_modal/generic_form.dart';
import 'package:upeg/src/widgets/expanded_modal/image_conversion_form_policy.dart';
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
part '../widgets/expanded_modal/modal_frame.dart';

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
  late final GenericFormController _controller;
  late final InlineDraftStore _drafts;
  late final int _draftGeneration;
  PinKey? _editingPin;
  ToolArgs? _bespokeInput;
  ToolArgs? _outcomeArgs;
  ToolArgs? _outcomeInput;
  ToolArgs? _runInput;
  bool _restoredOutcome = false;
  bool _truncatedOutcome = false;
  bool _savingDraft = false;
  bool _publishingOutcome = false;
  bool _pinPending = false;

  void _updatePinState(VoidCallback change) {
    if (mounted) setState(change);
  }

  int _formRevision = 0;
  PresentationCallOriginController _presentationCalls =
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
    _controller = GenericFormController(onChanged: _handleInputChanged);
    _drafts = ref.read(inlineDraftProvider);
    _draftGeneration = _drafts.generation;
    _editingPin = widget.pinKey;
    final pin = _editingPin;
    final saved = pin == null
        ? null
        : _drafts.read(pin, toolId: ToolId.parse(widget.tool.id));
    final initial = widget.initialInput ?? saved;
    if (initial != null) {
      _seedInput(initial, preserveInvalidText: widget.initialInput == null);
    }
    _drafts.addListener(_handleSharedDraftChanged);
    if (pin != null) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted || _projectChanged || _editingPin != pin) return;
        ref.read(lastOutcomeProvider.notifier).hydrateForBoard(pin.$1);
        _adoptOutcome(ref.read(pinLastOutcomeProvider(pin)));
      });
    }
    if (_shouldAutoRunRead) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) unawaited(_dispatchToolArgs(_currentInput));
      });
    }
  }

  @override
  void didUpdateWidget(ExpandedModalPage oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (_projectChanged) return;
    if (oldWidget.pinKey == widget.pinKey &&
        oldWidget.tool.id == widget.tool.id) {
      return;
    }
    _editingPin = widget.pinKey;
    _outcome = null;
    _outcomeArgs = null;
    _outcomeInput = null;
    _outcomeRunId = null;
    _presentationCall = null;
    _presentationCalls = PresentationCallOriginController();
    _restoredOutcome = false;
    _truncatedOutcome = false;
    _formAllOk = false;
    final pin = _editingPin;
    final saved = pin == null
        ? null
        : _drafts.read(pin, toolId: ToolId.parse(widget.tool.id));
    _seedInput(widget.initialInput ?? saved ?? ToolArgs.empty);
    _formRevision++;
    if (pin != null) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted || _projectChanged || _editingPin != pin) return;
        ref.read(lastOutcomeProvider.notifier).hydrateForBoard(pin.$1);
        _adoptOutcome(ref.read(pinLastOutcomeProvider(pin)));
      });
    }
  }

  bool get _shouldAutoRunRead =>
      widget.autoRunRead ||
      (_editingPin == null &&
          widget.tool.effect == ToolEffectDto.read &&
          widget.tool.presentation != null &&
          !widget.tool.inputFields.any((field) => field.required_));

  @override
  void dispose() {
    _drafts.removeListener(_handleSharedDraftChanged);
    super.dispose();
  }

  bool get _projectChanged => _draftGeneration != _drafts.generation;

  bool get _draftLoadFailed =>
      _editingPin != null && _drafts.hasLoadFailure(_editingPin!);

  ToolArgs get _currentInput => bespokeFor(ToolId.parse(widget.tool.id)) == null
      ? _controller.snapshot()
      : _bespokeInput ?? ToolArgs.empty;

  ResultInputRelation get _outcomeInputRelation =>
      resultInputRelation(_outcomeInput, _currentInput);

  bool get _outcomeStale =>
      _outcome != null && _outcomeInputRelation == ResultInputRelation.previous;

  bool get _outcomeInputUnknown =>
      _outcome != null && _outcomeInputRelation == ResultInputRelation.unknown;

  void _seedInput(ToolArgs args, {bool preserveInvalidText = true}) {
    _controller.seed(
      args,
      fields: widget.tool.inputFields,
      preserveInvalidText: preserveInvalidText,
    );
    _bespokeInput = args;
  }

  void _handleInputChanged() => _saveInput(_controller.snapshot());

  void _handleBespokeInputChanged(ToolArgs args) {
    _bespokeInput = args;
    _saveInput(args);
  }

  void _saveInput(ToolArgs args) {
    if (_projectChanged || _draftLoadFailed) return;
    final pin = _editingPin;
    if (pin != null) {
      _savingDraft = true;
      try {
        _drafts.set(pin, args, toolId: ToolId.parse(widget.tool.id));
      } finally {
        _savingDraft = false;
      }
    }
    if (mounted) setState(() {});
  }

  void _handleSharedDraftChanged() {
    if (!mounted) return;
    if (_projectChanged) {
      _closeModalRoute(context);
      return;
    }
    final pin = _editingPin;
    if (_savingDraft || pin == null) return;
    final args = _drafts.read(pin, toolId: ToolId.parse(widget.tool.id));
    setState(() {
      if (args != null && args != _currentInput) {
        _seedInput(args);
        _formRevision++;
      }
    });
  }

  void _adoptOutcome(PinOutcome? outcome) {
    if (!mounted ||
        _projectChanged ||
        _publishingOutcome ||
        outcome == null ||
        _phase.blocksRun) {
      return;
    }
    if (identical(_outcome, outcome.result)) return;
    setState(() {
      _outcome = outcome.result;
      _outcomeArgs = outcome.args;
      _outcomeInput = outcome.args;
      _outcomeRunId = null;
      _presentationCall = null;
      _restoredOutcome = outcome is RestoredOutcome;
      _truncatedOutcome = outcome is RestoredOutcome && outcome.truncated;
    });
  }

  void _acceptOutcome(
    CanonicalToolResult result,
    ToolArgs args, {
    DispatchRunId? runId,
    PinKey? expectedPin,
  }) {
    if (!mounted || _projectChanged || expectedPin != _editingPin) return;
    setState(() {
      _outcome = result;
      _outcomeArgs = args;
      _outcomeInput = _runInput ?? args;
      _outcomeRunId = runId;
      _restoredOutcome = false;
      _truncatedOutcome = false;
    });
  }

  bool _persistOutcome(
    LastOutcomeNotifier recorder,
    PinKey? pin,
    ToolId toolId,
    CanonicalToolResult result,
    ToolArgs args,
    int? runToken,
  ) {
    if (pin == null) return true;
    if (runToken != null && !recorder.isLatestRun(pin, runToken)) return false;
    _publishingOutcome = true;
    try {
      recorder.record(pin, toolId, result, args: args, runToken: runToken);
    } finally {
      _publishingOutcome = false;
    }
    return true;
  }

  void _handleFormValidation(bool allOk) {
    if (mounted) setState(() => _formAllOk = allOk);
  }

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
    if (_phase.blocksRun ||
        _projectChanged ||
        _draftLoadFailed ||
        _pinPending) {
      return;
    }
    _runInput = _currentInput;
    setState(() => _phase = _RunPhase.awaitingApproval);
    try {
      final external = ExternalRunController(ref);
      if (widget.tool.invoker == InvokerDto.external_) {
        try {
          final inspection = await external.inspect(widget.tool);
          if (!mounted || _projectChanged) return;
          if (inspection is ExternalReadinessHostUnpaired ||
              inspection is ExternalReadinessInspected &&
                  inspection.blocksRun) {
            setState(() => _phase = _RunPhase.idle);
            return;
          }
        } on Object {
          if (mounted) setState(() => _phase = _RunPhase.idle);
          return;
        }
      }
      final verdict = await _askApproval();
      if (!mounted || _projectChanged) return;
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
    } on Object catch (error) {
      _acceptOutcome(_failedDispatch(error), args, expectedPin: _editingPin);
    } finally {
      if (mounted) {
        setState(() => _phase = _RunPhase.idle);
        final pin = _editingPin;
        if (pin != null) _adoptOutcome(ref.read(pinLastOutcomeProvider(pin)));
      }
    }
  }

  CanonicalToolResult _failedDispatch(Object error) => CanonicalToolResult(
    ok: false,
    outputs: const <CanonicalOutputEntry>[],
    error: CanonicalToolError(
      code: _dispatchStreamFailedCode,
      message: '$error',
    ),
  );

  Future<ApprovalVerdict> _askApproval() => resolveApprovalBeforeDispatch(
    context: context,
    ref: ref,
    tool: widget.tool,
  );

  Future<void> _attachDispatch(
    ExternalRunController external,
    ToolArgs args,
  ) async {
    final pin = _editingPin;
    final toolId = ToolId.parse(widget.tool.id);
    final recorder = ref.read(lastOutcomeProvider.notifier);
    final runToken = pin == null ? null : recorder.beginRun(pin);
    setState(() {
      _phase = _RunPhase.dispatching;
      _runId = null;
      _cancelRequested = false;
      _tail = const LiveTail.empty(maxLines: modalLiveTailMaxLines);
    });
    CanonicalToolResult result;
    try {
      result = await external.dispatchRemote(widget.tool, args, pinKey: pin);
    } on Object catch (error) {
      result = _failedDispatch(error);
    }
    if (_persistOutcome(recorder, pin, toolId, result, args, runToken)) {
      _acceptOutcome(result, args, expectedPin: pin);
    }
  }

  Future<void> _webDispatch(ToolArgs args, {required bool approve}) async {
    final pin = _editingPin;
    final toolId = ToolId.parse(widget.tool.id);
    final recorder = ref.read(lastOutcomeProvider.notifier);
    final runToken = pin == null ? null : recorder.beginRun(pin);
    final dispatch = ref.read(toolkitDispatchProvider);
    final boardKey = pin?.$1.value ?? ref.read(currentBoardKeyProvider)?.value;
    setState(() {
      _phase = _RunPhase.dispatching;
      _runId = null;
      _cancelRequested = false;
      _tail = const LiveTail.empty(maxLines: modalLiveTailMaxLines);
    });
    CanonicalToolResult result;
    try {
      result = await dispatch(
        toolId: toolId,
        args: args,
        boardKey: boardKey,
        pinId: pin?.$2.value,
        approve: approve,
      );
    } on Object catch (error) {
      result = _failedDispatch(error);
    }
    if (_persistOutcome(recorder, pin, toolId, result, args, runToken)) {
      _acceptOutcome(result, args, expectedPin: pin);
    }
  }

  Future<void> _streamDispatch(
    ToolArgs args, {
    required bool approve,
    PresentationCallIdentity? forcedCall,
    PresentationRefreshRun? refreshRun,
  }) async {
    final toolId = ToolId.parse(widget.tool.id);
    final pin = _editingPin;
    final recorder = ref.read(lastOutcomeProvider.notifier);
    final runToken = pin == null ? null : recorder.beginRun(pin);
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
    final runningLease = pin == null ? null : running.begin(pin);
    try {
      final events = dispatch(
        pinKey: pin,
        toolId: toolId,
        args: args,
        approve: approve,
        runId: runId,
      );
      await for (final event in events) {
        switch (event) {
          case DispatchStreamEventDto_Chunk(:final chunk):
            if (mounted) setState(() => _tail = _tail.append(chunk));
          case DispatchStreamEventDto_Done(:final result):
            completed = result;
            if (mounted) {
              ExternalRunController(
                ref,
              ).invalidateReadinessOnMissing(widget.tool, result);
            }
            final accepted = refreshRun == null
                ? calls.acceptResult(call)
                : calls.acceptRefresh(refreshRun);
            if (accepted && (refreshRun == null || result.ok)) {
              if (_persistOutcome(
                recorder,
                pin,
                toolId,
                result,
                args,
                runToken,
              )) {
                _acceptOutcome(result, args, runId: runId, expectedPin: pin);
              }
            }
        }
      }
    } on Object catch (err) {
      streamFailed = true;
      // The stream itself failed (the bridge refused the run, the
      // channel broke). Nothing settled it, so the modal has to: an
      // unhandled async error would leave the person looking at a
      // spinner that never resolves.
      if (refreshRun == null) {
        final failure = _failedDispatch(err);
        if (_persistOutcome(recorder, pin, toolId, failure, args, runToken)) {
          _acceptOutcome(failure, args, expectedPin: pin);
        }
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
    setState(() => _cancelRequested = true);
    ref.read(cancelDispatchFnProvider)(runId: runId);
  }

  void _handleRun() => unawaited(_dispatchToolArgs(_currentInput));

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
      currentInputsJson: (_outcomeArgs ?? ToolArgs.empty).encodeJson(),
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

  Future<void> _handleCopy() => _copyModalOutcome(this);

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
  Widget build(BuildContext context) => _buildModalFrame(this, context);
}
