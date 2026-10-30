/// Tile with shared typed inputs, source-driven runs, and a retained result.
library;

import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/identity.dart' show ToolId;
import 'package:upeg/src/rust/api/dispatch_stream.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/state/dispatch_stream_provider.dart';
import 'package:upeg/src/state/inline_draft_provider.dart';
import 'package:upeg/src/state/last_outcome_provider.dart';
import 'package:upeg/src/widgets/pin_save_status.dart';
import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';
import 'package:upeg/src/widgets/copyable_outcome_text.dart';
import 'package:upeg/src/state/pin_provider.dart' show PinKey;
import 'package:upeg/src/state/presentation_resolver_provider.dart';
import 'package:upeg/src/state/running_tools_provider.dart';
import 'package:upeg/src/state/toolkit_runtime_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/approval_confirm_dialog.dart';
import 'package:upeg/src/widgets/expanded_modal/generic_form.dart'
    show GenericFormController, GenericFormWidget;
import 'package:upeg/src/widgets/expanded_modal/live_output_tail.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart' show ToolArgs;

/// Debounce before an input change triggers a live dispatch — long enough
/// to coalesce fast typing, short enough to feel instant.
const Duration inlineRunDebounce = Duration(milliseconds: 120);

/// Short motion used to reveal a newly completed result inside a U1 pin.
const Duration inlineOutputRevealDuration = Duration(milliseconds: 160);

const Key inlineRunButtonKey = Key('inline-run-btn');

/// Widget key for the in-flight indicator (also exposes live semantics).
const Key inlineLoadingIndicatorKey = Key('inline-loading-indicator');

/// Widget key for the in-flight Cancel affordance.
const Key inlineCancelButtonKey = Key('inline-cancel-btn');

const Key inlineCopyResultButtonKey = Key('inline-copy-result-btn');

/// Catalog key for the in-flight Cancel affordance's label.
const String _inlineCancelRunKey = 'modal.action.cancel_run';

/// User-facing failure copy intentionally excludes backend paths and URLs.
const String inlineSafeDispatchErrorMessage = 'Unable to run this tool.';

/// Compact hint for a source whose value changes only outside this form.
const String inlineStaticSourceHint = 'static input';

/// Announced by each field, never drawn, when editing runs the tool.
const String inlineUserInputSourceHint = 'runs on input';

const String _inlineTimerHintPrefix = 'refreshes every';
const String _inlineShortcutHintPrefix = 'shortcut';
const String _inlineMillisecondsUnit = 'ms';

/// Largest interval accepted by browser `setInterval` implementations.
const int inlineMaximumTimerIntervalMilliseconds = 2_147_483_647;

/// Fixed copy for malformed timer metadata. It deliberately omits the value.
const String inlineInvalidTimerSourceHint = 'refresh disabled';

Duration? _browserSafeTimerInterval(BigInt intervalMs) {
  if (intervalMs <= BigInt.zero ||
      intervalMs > BigInt.from(inlineMaximumTimerIntervalMilliseconds)) {
    return null;
  }
  return Duration(milliseconds: intervalMs.toInt());
}

String inlineTimerSourceHint(BigInt intervalMs) {
  if (_browserSafeTimerInterval(intervalMs) == null) {
    return inlineInvalidTimerSourceHint;
  }
  return '$_inlineTimerHintPrefix $intervalMs $_inlineMillisecondsUnit';
}

String inlineShortcutSourceHint(String keys) =>
    '$_inlineShortcutHintPrefix: $keys';

const String _inlineDispatchErrorCode = 'inline_dispatch_failed';
const String _inlineLoadingSemanticsKey = 'a11y.inline.running';
const String _inlineOutputValueKeyPrefix = 'inline-output-value-';
const String _inlineFallbackOutputId = 'result';
const double _inlineLoadingSize = 16;
const double _inlineLoadingStrokeWidth = 2;
const double _inlineOutputGap = 4;
const double _inlineTailFontSize = 9;
const double _inlineOutputRowGap = 2;
const double _inlineCompactCopyGutter = 66;
const int _inlineSingleLine = 1;
const int _inlineErrorMaxLines = 3;

Key inlineOutputValueKey(String outputId) =>
    ValueKey<String>('$_inlineOutputValueKeyPrefix$outputId');

enum _RunTrigger { inputChange, periodic, manual }

class GenericInlinePinBody extends ConsumerStatefulWidget {
  const GenericInlinePinBody({
    required this.tool,
    required this.pinKey,
    super.key,
  });

  final ToolDto tool;
  final PinKey pinKey;

  @override
  ConsumerState<GenericInlinePinBody> createState() =>
      _GenericInlinePinBodyState();
}

class _GenericInlinePinBodyState extends ConsumerState<GenericInlinePinBody> {
  late final GenericFormController _form = GenericFormController(
    onChanged: _onFormChanged,
  );
  Timer? _debounce;
  Timer? _periodic;
  late final InlineDraftStore _draftStore;
  final ScrollController _scrollController = ScrollController();
  CanonicalToolResult? _result;
  ToolArgs? _resultArgs;
  int _formEpoch = 0;
  int _contextGeneration = 0;
  bool _contextChanged = false;
  bool _loadFailed = false;

  /// True once every required field is filled and valid. Guards dispatch so
  /// a half-typed multi-field tool does not fire per keystroke. A tool with
  /// no inputs has nothing to fill in, so it starts runnable — otherwise the
  /// gate would never open and the pin could never dispatch.
  late bool _allOk = widget.tool.inputFields.isEmpty;

  /// Incremented for every form change, including invalid input. A result is
  /// accepted only for the exact revision it dispatched.
  int _revision = 0;
  int? _runningRevision;
  int? _queuedRevision;
  bool _running = false;

  /// Last few lines the running tool has printed, capped short: a board
  /// tile shows a sign of life, not a log.
  LiveTail _tail = const LiveTail.empty(maxLines: inlineLiveTailMaxLines);

  /// Identity of the in-flight run, so Cancel can name it.
  DispatchRunId? _runId;

  bool get _isManual => widget.tool.source is SourceDto_Manual;

  /// This tool stops at a human-approval barrier.
  ///
  /// A gated tool never auto-runs — not on input change, not on a timer.
  /// An approval barrier is a question for a person, and a 120ms debounce
  /// or a periodic tick would re-ask it forever. Only the explicit Run
  /// button raises the dialog.
  bool get _requiresApproval => widget.tool.requiresApproval;

  /// A tool with no inputs has no on-change trigger to ride, so it needs
  /// an explicit Run affordance whatever its declared source says —
  /// same reason a `Manual` tool gets one, and the same reason a gated
  /// tool does.
  bool get _needsRunButton =>
      _isManual || _requiresApproval || widget.tool.inputFields.isEmpty;

  /// What every field announces: editing runs the tool (the default source).
  String? get _fieldHint => switch (widget.tool.source) {
    SourceDto_UserInput() when !_requiresApproval => inlineUserInputSourceHint,
    _ => null,
  };

  bool get _sourceRunsOnChange =>
      !_requiresApproval &&
      switch (widget.tool.source) {
        SourceDto_UserInput() || SourceDto_Timer() => true,
        SourceDto_Shortcut() ||
        SourceDto_Manual() ||
        SourceDto_Static() => false,
      };

  ToolId get _toolId => ToolId.parse(widget.tool.id);

  @override
  void initState() {
    super.initState();
    // Restore inline values from the shared draft so a board rebuild — or a
    // round-trip through the expanded modal — does not lose what was typed.
    _draftStore = ref.read(inlineDraftProvider);
    _contextGeneration = _draftStore.generation;
    final draft = _draftStore.read(widget.pinKey, toolId: _toolId);
    _loadFailed = _draftStore.hasLoadFailure(widget.pinKey);
    if (draft != null) {
      _form.seed(
        draft,
        fields: widget.tool.inputFields,
        preserveInvalidText: true,
      );
    }
    _draftStore.addListener(_onSharedDraftChanged);
    _configurePeriodic();
    _hydrateSavedResult();
  }

  @override
  void didUpdateWidget(GenericInlinePinBody oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (_contextChanged ||
        widget.pinKey != oldWidget.pinKey ||
        widget.tool.id != oldWidget.tool.id) {
      _contextChanged = false;
      _debounce?.cancel();
      _queuedRevision = null;
      _revision += 1;
      _running = false;
      _runningRevision = null;
      _runId = null;
      _result = null;
      _resultArgs = null;
      _allOk = widget.tool.inputFields.isEmpty;
      _form.seed(
        _draftStore.read(widget.pinKey, toolId: _toolId) ?? ToolArgs.empty,
        fields: widget.tool.inputFields,
        preserveInvalidText: true,
      );
      _formEpoch += 1;
      _configurePeriodic();
      _hydrateSavedResult();
      return;
    }
    if (widget.tool.source != oldWidget.tool.source) {
      _debounce?.cancel();
      _queuedRevision = null;
      _revision += 1;
      _result = null;
      _configurePeriodic();
    }
  }

  void _hydrateSavedResult() {
    final pin = widget.pinKey;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || widget.pinKey != pin) return;
      ref.read(lastOutcomeProvider.notifier).ensureHydrated(pin.$1);
    });
  }

  @override
  void dispose() {
    _debounce?.cancel();
    _periodic?.cancel();
    _revision += 1;
    _draftStore.removeListener(_onSharedDraftChanged);
    _scrollController.dispose();
    super.dispose();
  }

  void _onSharedDraftChanged() {
    if (_contextGeneration != _draftStore.generation) {
      _contextGeneration = _draftStore.generation;
      _contextChanged = true;
      _debounce?.cancel();
      _periodic?.cancel();
      _queuedRevision = null;
      _revision += 1;
      return;
    }
    final draft = _draftStore.read(widget.pinKey, toolId: _toolId);
    final loadFailed = _draftStore.hasLoadFailure(widget.pinKey);
    final loadChanged = loadFailed != _loadFailed;
    _loadFailed = loadFailed;
    if (!mounted) return;
    if (draft == null || draft == _form.snapshot()) {
      if (loadChanged) setState(() {});
      return;
    }
    _debounce?.cancel();
    setState(() {
      _revision += 1;
      _queuedRevision = null;
      _form.seed(
        draft,
        fields: widget.tool.inputFields,
        preserveInvalidText: true,
      );
      _formEpoch += 1;
    });
  }

  /// (Re)build the interval poller from the tool's `Source`. Only a
  /// `Timer` source runs a poller; every other source cancels it.
  void _configurePeriodic() {
    _periodic?.cancel();
    if (_requiresApproval) {
      _periodic = null;
      return;
    }
    final source = widget.tool.source;
    final interval = source is SourceDto_Timer
        ? _browserSafeTimerInterval(source.intervalMs)
        : null;
    if (interval == null) {
      _periodic = null;
      return;
    }
    _periodic = Timer.periodic(interval, (_) {
      _requestRun(_RunTrigger.periodic);
    });
  }

  void _onFormChanged() {
    if (_draftStore.hasLoadFailure(widget.pinKey)) return;
    // Always record the current inputs so tapping the pin can seed the
    // expanded modal with them — even for manual tools that do not auto-run.
    _draftStore.set(widget.pinKey, _form.snapshot(), toolId: _toolId);
    _debounce?.cancel();
    setState(() {
      _revision += 1;
      _queuedRevision = null;
    });
    if (!_sourceRunsOnChange) return;
    _debounce = Timer(inlineRunDebounce, () {
      _debounce = null;
      _requestRun(_RunTrigger.inputChange);
    });
  }

  void _requestRun(_RunTrigger trigger) {
    if (_contextChanged || _draftStore.hasLoadFailure(widget.pinKey)) return;
    if (!_allOk) return;
    if (_requiresApproval && trigger != _RunTrigger.manual) return;
    if (trigger == _RunTrigger.periodic) {
      // The first periodic tick owns this revision instead of racing its
      // pending input debounce. If an older revision is still running, the
      // tick transfers that ownership to one queued run of the latest input.
      _cancelDebounce();
      if (_running) {
        if (_runningRevision != _revision) {
          _queuedRevision = _revision;
        }
        return;
      }
    }
    if (_running) {
      if (trigger == _RunTrigger.inputChange && _runningRevision != _revision) {
        _queuedRevision = _revision;
      }
      return;
    }
    unawaited(_run(_revision, _form.snapshot()));
  }

  void _cancelDebounce() {
    _debounce?.cancel();
    _debounce = null;
  }

  Future<void> _run(int revision, ToolArgs args) async {
    final pinKey = widget.pinKey;
    final toolId = _toolId;
    // `_running` flips before the first await so a second trigger — and a
    // second approval dialog — cannot slip in behind this one.
    setState(() {
      _running = true;
      _runningRevision = revision;
      _tail = const LiveTail.empty(maxLines: inlineLiveTailMaxLines);
    });
    final verdict = await resolveApprovalBeforeDispatch(
      context: context,
      ref: ref,
      tool: widget.tool,
    );
    if (!mounted || widget.pinKey != pinKey || _toolId != toolId) return;
    if (verdict is! ApprovalGranted) {
      setState(() {
        _running = false;
        _runningRevision = null;
      });
      return;
    }
    await _streamRun(revision, args, approve: verdict.approve);
  }

  Future<void> _streamRun(
    int revision,
    ToolArgs args, {
    required bool approve,
  }) async {
    if (kIsWeb) {
      await _webRun(revision, args, approve: approve);
      return;
    }
    final pinKey = widget.pinKey;
    final toolId = _toolId;
    final dispatch = ref.read(dispatchStreamFnProvider);
    final runningTools = ref.read(runningToolsProvider.notifier);
    final recorder = ref.read(lastOutcomeProvider.notifier);
    final runToken = recorder.beginRun(pinKey);
    final runId = nextDispatchRunId();
    setState(() => _runId = runId);
    final runningLease = runningTools.begin(pinKey);

    CanonicalToolResult outcome = _safeFailureResult();
    try {
      final events = dispatch(
        pinKey: pinKey,
        toolId: toolId,
        args: args,
        approve: approve,
        runId: runId,
      );
      await for (final event in events) {
        switch (event) {
          case DispatchStreamEventDto_Chunk(:final chunk):
            if (mounted && widget.pinKey == pinKey) {
              setState(() => _tail = _tail.append(chunk));
            }
          case DispatchStreamEventDto_Done(:final result):
            outcome = result;
        }
      }
    } on Object {
      outcome = _safeFailureResult();
    } finally {
      runningTools.end(runningLease);
    }
    recorder.record(pinKey, toolId, outcome, args: args, runToken: runToken);
    if (!mounted || widget.pinKey != pinKey || _toolId != toolId) return;

    final acceptResult =
        revision == _revision && recorder.isLatestRun(pinKey, runToken);
    final rerunLatest =
        _queuedRevision == _revision && _allOk && _sourceRunsOnChange;
    setState(() {
      _running = false;
      _runningRevision = null;
      _queuedRevision = null;
      _runId = null;
      if (acceptResult) {
        _result = outcome;
        _resultArgs = args;
      }
    });
    if (acceptResult) {
      _revealOutput();
    }
    if (rerunLatest) {
      scheduleMicrotask(() => _requestRun(_RunTrigger.inputChange));
    }
  }

  Future<void> _webRun(
    int revision,
    ToolArgs args, {
    required bool approve,
  }) async {
    final pinKey = widget.pinKey;
    final toolId = _toolId;
    final runningTools = ref.read(runningToolsProvider.notifier);
    final recorder = ref.read(lastOutcomeProvider.notifier);
    final runToken = recorder.beginRun(pinKey);
    final runningLease = runningTools.begin(pinKey);
    CanonicalToolResult outcome;
    try {
      outcome = await ref.read(toolkitDispatchProvider)(
        toolId: toolId,
        args: args,
        boardKey: pinKey.$1.value,
        pinId: pinKey.$2.value,
        approve: approve,
      );
    } on Object {
      outcome = _safeFailureResult();
    } finally {
      runningTools.end(runningLease);
    }
    recorder.record(pinKey, toolId, outcome, args: args, runToken: runToken);
    if (!mounted || widget.pinKey != pinKey || _toolId != toolId) return;
    final acceptResult =
        revision == _revision && recorder.isLatestRun(pinKey, runToken);
    final rerunLatest =
        _queuedRevision == _revision && _allOk && _sourceRunsOnChange;
    setState(() {
      _running = false;
      _runningRevision = null;
      _queuedRevision = null;
      _runId = null;
      if (acceptResult) {
        _result = outcome;
        _resultArgs = args;
      }
    });
    if (acceptResult) {
      _revealOutput();
    }
    if (rerunLatest) {
      scheduleMicrotask(() => _requestRun(_RunTrigger.inputChange));
    }
  }

  /// Ask the in-flight run to stop. The run still ends with its own
  /// `Done` event, so the tile keeps one completion path.
  void _handleCancel() {
    final runId = _runId;
    if (runId == null) return;
    ref.read(cancelDispatchFnProvider)(runId: runId);
  }

  CanonicalToolResult _safeFailureResult() => const CanonicalToolResult(
    ok: false,
    outputs: <CanonicalOutputEntry>[],
    error: CanonicalToolError(
      code: _inlineDispatchErrorCode,
      message: inlineSafeDispatchErrorMessage,
    ),
  );

  void _revealOutput() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_scrollController.hasClients) return;
      unawaited(
        _scrollController.animateTo(
          _scrollController.position.maxScrollExtent,
          duration: inlineOutputRevealDuration,
          curve: Curves.easeOut,
        ),
      );
    });
  }

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    final saved = ref.watch(pinLastOutcomeProvider(widget.pinKey));
    final result = saved?.result ?? _result;
    final resultArgs = saved == null ? _resultArgs : saved.args;
    final relation = resultInputRelation(resultArgs, _form.snapshot());
    final compactEligible =
        result?.ok == true &&
        result!.outputs.length == 1 &&
        widget.tool.presentation == null;
    final fieldType = widget.tool.inputFields.firstOrNull?.fieldType;
    final simpleInput =
        fieldType is InputFieldType_Text ||
        fieldType is InputFieldType_Number ||
        fieldType is InputFieldType_Integer;
    final besideInput =
        compactEligible &&
        widget.tool.inputFields.length == 1 &&
        simpleInput &&
        !_needsRunButton;
    final besideRun =
        compactEligible &&
        widget.tool.inputFields.isEmpty &&
        _needsRunButton &&
        widget.tool.source is! SourceDto_Timer;
    final compactResult = besideInput || besideRun;
    final sourceHint = _sourceHint(tokens);
    final copyGutter = EdgeInsets.only(
      right: besideInput ? _inlineCompactCopyGutter : 0,
    );
    return Padding(
      padding: UpegSizing.pinBodyPadding,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          PinSaveStatus(pinKey: widget.pinKey, toolId: _toolId, compact: true),
          Expanded(
            child: SingleChildScrollView(
              controller: _scrollController,
              child: Stack(
                children: [
                  Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      if (widget.tool.inputFields.isNotEmpty)
                        Padding(
                          padding: copyGutter,
                          child: PinDraftEditor(
                            pinKey: widget.pinKey,
                            child: GenericFormWidget(
                              key: ValueKey(_formEpoch),
                              tool: widget.tool,
                              controller: _form,
                              compact: true,
                              semanticsHint: _fieldHint,
                              onValidationChanged: (ok) {
                                if (ok != _allOk) setState(() => _allOk = ok);
                              },
                            ),
                          ),
                        ),
                      if (_needsRunButton)
                        if (besideRun)
                          Row(
                            children: [
                              Expanded(child: _runButton()),
                              _copyResultButton(
                                result,
                                relation,
                                compact: true,
                              ),
                            ],
                          )
                        else
                          _runButton(),
                      if (sourceHint != null &&
                          !compactResult &&
                          result?.ok != false)
                        Padding(padding: copyGutter, child: sourceHint),
                      if (_runId != null) _runningStrip(),
                      if (_runId != null && _tail.isNotEmpty)
                        Padding(
                          key: inlineLiveOutputTailKey,
                          padding: const EdgeInsets.only(
                            top: _inlineOutputRowGap,
                          ),
                          child: LiveOutputTailView(
                            tail: _tail,
                            fontSize: _inlineTailFontSize,
                          ),
                        ),
                      const SizedBox(height: _inlineOutputGap),
                      _output(
                        tokens,
                        result,
                        saved,
                        relation,
                        compactResult,
                        compactSourceHint: compactResult || result?.ok == false
                            ? sourceHint
                            : null,
                      ),
                    ],
                  ),
                  if (besideInput)
                    Positioned(
                      top: 0,
                      right: 0,
                      child: _copyResultButton(result, relation, compact: true),
                    ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }

  /// The in-flight strip offers Cancel and running status.
  ///
  /// Mounted only while a dispatch is actually running — keyed on
  /// [_runId] rather than `_running`, because the approval question in
  /// front of a gated run has started nothing yet: there is no progress
  /// to report and nothing to cancel.
  Widget _runningStrip() {
    return Row(
      children: [
        Flexible(child: _cancelButton()),
        const SizedBox(width: _inlineOutputGap),
        _loadingIndicator(),
      ],
    );
  }

  /// Sized to the indicator's own height so appearing mid-run costs the
  /// tile no vertical space — a board tile has none to spare.
  Widget _cancelButton() {
    return TextButton(
      key: inlineCancelButtonKey,
      style: TextButton.styleFrom(
        padding: EdgeInsets.zero,
        minimumSize: const Size(0, _inlineLoadingSize),
        tapTargetSize: MaterialTapTargetSize.shrinkWrap,
        textStyle: Theme.of(context).textTheme.labelSmall,
      ),
      onPressed: _handleCancel,
      child: Text(
        t(ref, _inlineCancelRunKey),
        maxLines: _inlineSingleLine,
        overflow: TextOverflow.ellipsis,
      ),
    );
  }

  Widget _loadingIndicator() {
    return Semantics(
      label: t(ref, _inlineLoadingSemanticsKey),
      liveRegion: true,
      child: const Align(
        alignment: Alignment.centerRight,
        child: SizedBox.square(
          key: inlineLoadingIndicatorKey,
          dimension: _inlineLoadingSize,
          child: CircularProgressIndicator(
            strokeWidth: _inlineLoadingStrokeWidth,
          ),
        ),
      ),
    );
  }

  Widget _runButton() {
    final tokens = context.upeg;
    final colorScheme = Theme.of(context).colorScheme;
    return Align(
      alignment: Alignment.centerRight,
      child: FilledButton(
        key: inlineRunButtonKey,
        style: FilledButton.styleFrom(
          backgroundColor: colorScheme.primary,
          foregroundColor: colorScheme.onPrimary,
          overlayColor: upegHighContrastForeground(colorScheme.onPrimary),
          disabledBackgroundColor: tokens.surface2,
          disabledForegroundColor: tokens.fg2,
          minimumSize: const Size.square(kMinInteractiveDimension),
          tapTargetSize: MaterialTapTargetSize.padded,
          textStyle: Theme.of(context).textTheme.labelSmall,
        ),
        onPressed: _allOk && !_running && !_loadFailed && !_contextChanged
            ? () => _requestRun(_RunTrigger.manual)
            : null,
        child: Text(t(ref, 'inline.run')),
      ),
    );
  }

  Widget? _sourceHint(UpegTokens tokens) {
    // Only the exceptions are drawn; the default source is announced by the
    // fields ([_fieldHint]). A gated tool, or a non-Timer tool without
    // inputs, says nothing.
    final text = switch (widget.tool.source) {
      _ when _requiresApproval => null,
      SourceDto_Timer(:final intervalMs) => inlineTimerSourceHint(intervalMs),
      _ when widget.tool.inputFields.isEmpty => null,
      SourceDto_Shortcut(:final keys) => inlineShortcutSourceHint(keys),
      SourceDto_Static() => inlineStaticSourceHint,
      SourceDto_UserInput() || SourceDto_Manual() => null,
    };
    if (text == null) return null;
    return Text(
      text,
      maxLines: _inlineSingleLine,
      overflow: TextOverflow.ellipsis,
      style: Theme.of(context).textTheme.labelSmall?.copyWith(
        color: tokens.fg3,
        fontFamily: upegMonoFontFamily,
        fontFamilyFallback: upegMonoFontFamilyFallback,
      ),
    );
  }

  Widget _output(
    UpegTokens tokens,
    CanonicalToolResult? result,
    PinOutcome? saved,
    ResultInputRelation relation,
    bool compact, {
    Widget? compactSourceHint,
  }) {
    final stale = relation == ResultInputRelation.previous;
    if (result == null) {
      return const SizedBox.shrink();
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        if (!result.ok) _renderResult(result, tokens),
        if (compact)
          Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                t(ref, switch (relation) {
                  ResultInputRelation.current => 'inline.result.current_short',
                  ResultInputRelation.previous =>
                    'inline.result.previous_short',
                  ResultInputRelation.unknown => 'inline.result.unknown_short',
                }),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(color: tokens.fg3, fontSize: 9, height: 1),
              ),
              Text(
                result.primaryOutputText,
                key: inlineOutputValueKey(
                  result.primaryOutputId ?? _inlineFallbackOutputId,
                ),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(color: tokens.fg, fontSize: 11, height: 1),
              ),
            ],
          ),
        ?compactSourceHint,
        if (stale)
          Text(
            t(ref, 'modal.outcome.stale'),
            key: const Key('inline-result-stale'),
          ),
        if (relation == ResultInputRelation.unknown)
          Text(
            t(ref, 'modal.outcome.input_unknown'),
            key: const Key('inline-result-input-unknown'),
          ),
        if (saved is RestoredOutcome && (!compact || saved.truncated))
          Text(
            t(
              ref,
              saved.truncated
                  ? 'modal.outcome.truncated'
                  : 'modal.outcome.restored',
            ),
            key: const Key('inline-result-restored'),
          ),
        if (!compact) ...[
          Align(
            alignment: Alignment.centerRight,
            child: _copyResultButton(result, relation),
          ),
          if (result.ok) _renderResult(result, tokens),
        ],
      ],
    );
  }

  Widget _copyResultButton(
    CanonicalToolResult? result,
    ResultInputRelation relation, {
    bool compact = false,
  }) {
    final copyText = copyableOutcomeText(result);
    final stale = relation == ResultInputRelation.previous;
    return Tooltip(
      message: t(
        ref,
        copyText == null
            ? 'clipboard.no_text_result'
            : stale
            ? 'inline.copy_previous_result'
            : relation == ResultInputRelation.unknown
            ? 'modal.outcome.input_unknown'
            : 'inline.copy_result',
      ),
      child: TextButton(
        key: inlineCopyResultButtonKey,
        style: compact
            ? TextButton.styleFrom(
                fixedSize: const Size(56, 48),
                padding: EdgeInsets.zero,
                textStyle: const TextStyle(fontSize: 9, height: 1),
                tapTargetSize: MaterialTapTargetSize.shrinkWrap,
              )
            : null,
        onPressed: copyText == null
            ? null
            : () => unawaited(
                copyWithFeedback(
                  context: context,
                  ref: ref,
                  write: () =>
                      ref.read(clipboardWriterProvider).write(copyText),
                  successKey: copySuccessKeyForRelation(relation),
                ),
              ),
        child: Text(
          t(
            ref,
            compact || !stale
                ? 'inline.copy_result'
                : 'inline.copy_previous_result',
          ),
          maxLines: compact ? 2 : null,
          textAlign: compact ? TextAlign.center : null,
        ),
      ),
    );
  }

  Widget _renderResult(CanonicalToolResult result, UpegTokens tokens) {
    if (!result.ok) {
      return Text(
        t(ref, 'inline.dispatch_failed'),
        maxLines: _inlineErrorMaxLines,
        overflow: TextOverflow.ellipsis,
        style: Theme.of(context).textTheme.labelSmall?.copyWith(
          fontFamily: upegMonoFontFamily,
          fontFamilyFallback: upegMonoFontFamilyFallback,
          color: tokens.warn,
        ),
      );
    }
    final presentation = widget.tool.presentation == null
        ? null
        : ref.watch(presentationViewResolverProvider)(
            toolId: widget.tool.id,
            outputsJson: jsonEncode(result.jsonValues),
          );
    if (presentation != null &&
        presentation.diagnostics.isEmpty &&
        (presentation.summary.isNotEmpty || presentation.status != null)) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (presentation.status case final status?)
            Text(
              status.label,
              style: TextStyle(color: tokens.fg3, fontSize: 10),
            ),
          if (presentation.summary.isNotEmpty)
            Text(
              '${presentation.summary.first.label}: ${presentation.summary.first.value}',
              key: const Key('inline-rich-summary'),
              maxLines: _inlineSingleLine,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                color: tokens.fg,
                fontSize: 11,
                fontWeight: FontWeight.w600,
              ),
            ),
        ],
      );
    }
    // Tools that declare no `outputs` still yield a single fallback value
    // with no matching output field, so `displayRows` is empty for them —
    // fall back to the primary output text (mirrors the default pin body).
    final rows = result.displayRows(widget.tool.outputFields);
    final List<({String id, String label, String value})> values =
        rows.isNotEmpty
        ? <({String id, String label, String value})>[
            for (final row in rows)
              (id: row.id, label: row.label, value: row.value),
          ]
        : result.primaryOutputText.isEmpty
        ? const <({String id, String label, String value})>[]
        : <({String id, String label, String value})>[
            (
              id: result.primaryOutputId ?? _inlineFallbackOutputId,
              label: result.primaryOutput?.label ?? _inlineFallbackOutputId,
              value: result.primaryOutputText,
            ),
          ];
    if (values.isEmpty) return const SizedBox.shrink();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        for (final row in values)
          Padding(
            padding: const EdgeInsets.only(bottom: _inlineOutputRowGap),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.baseline,
              textBaseline: TextBaseline.alphabetic,
              children: [
                Flexible(
                  child: Text(
                    '${row.label}: ',
                    maxLines: _inlineSingleLine,
                    overflow: TextOverflow.ellipsis,
                    style: Theme.of(context).textTheme.labelSmall?.copyWith(
                      fontFamily: upegMonoFontFamily,
                      fontFamilyFallback: upegMonoFontFamilyFallback,
                      color: tokens.fg3,
                    ),
                  ),
                ),
                Expanded(
                  child: Text(
                    key: inlineOutputValueKey(row.id),
                    row.value,
                    maxLines: _inlineSingleLine,
                    overflow: TextOverflow.ellipsis,
                    style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      fontFamily: upegMonoFontFamily,
                      fontFamilyFallback: upegMonoFontFamilyFallback,
                      color: tokens.fg,
                    ),
                  ),
                ),
              ],
            ),
          ),
      ],
    );
  }
}
