/// Generic inline pin body: renders a tool's inputs + outputs directly on
/// the tile and runs it according to the tool's declared `Source`.
///
/// The input form is the SAME [GenericFormWidget] the expanded modal uses —
/// it renders each field by its type for any number of fields — so there is
/// no per-tool "template"; the body simply adapts to the tool's input
/// signature. The canonical result renders as compact output rows beneath.
///
/// The re-run trigger is a property of the TOOL, declared by its author in
/// the `#[upeg::tool(source = …)]` attribute / TOML manifest — not a
/// per-pin user setting. It is read from `tool.source` ([SourceDto]):
///   * `UserInput` (default) → on-change (debounced).
///   * `Timer { intervalMs }` → additionally poll on a timer (for tools
///     whose output drifts over time on the same input).
///   * `Manual` → never auto-run; an explicit Run button dispatches.
///   * `Shortcut` / `Static` → never dispatch from form changes.
///
/// The form's fields are real `EditableText`s, so the board's key-yield
/// guard routes plain-letter typing here instead of into board shortcuts
/// (same as [MemoPinBody] / ControlledEmbedTile) — no extra keyboard wiring.
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/identity.dart' show ToolId;
import 'package:upeg/src/rust/api/dispatch_stream.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/state/dispatch_stream_provider.dart';
import 'package:upeg/src/state/inline_draft_provider.dart';
import 'package:upeg/src/state/pin_provider.dart' show PinKey;
import 'package:upeg/src/state/running_tools_provider.dart';
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

/// Widget key for the manual-mode Run button (for tests).
const Key inlineRunButtonKey = Key('inline-run-btn');

/// Widget key for the in-flight indicator (also exposes live semantics).
const Key inlineLoadingIndicatorKey = Key('inline-loading-indicator');

/// Widget key for the in-flight Cancel affordance.
const Key inlineCancelButtonKey = Key('inline-cancel-btn');

/// Catalog key for the in-flight Cancel affordance's label.
const String _inlineCancelRunKey = 'modal.action.cancel_run';

/// User-facing failure copy intentionally excludes backend paths and URLs.
const String inlineSafeDispatchErrorMessage = 'Unable to run this tool.';

/// Compact hint for a source whose value changes only outside this form.
const String inlineStaticSourceHint = 'static input';

/// Compact hint for debounced on-change tools.
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
    final draft = _draftStore.read(widget.pinKey);
    if (draft != null) {
      _form.seed(draft, fields: widget.tool.inputFields);
    }
    _configurePeriodic();
  }

  @override
  void didUpdateWidget(GenericInlinePinBody oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.tool.source != oldWidget.tool.source) {
      _debounce?.cancel();
      _queuedRevision = null;
      _revision += 1;
      _result = null;
      _configurePeriodic();
    }
  }

  @override
  void dispose() {
    _debounce?.cancel();
    _periodic?.cancel();
    _revision += 1;
    _draftStore.clear(widget.pinKey);
    _scrollController.dispose();
    super.dispose();
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
    // Always record the current inputs so tapping the pin can seed the
    // expanded modal with them — even for manual tools that do not auto-run.
    _draftStore.set(widget.pinKey, _form.snapshot());
    _debounce?.cancel();
    setState(() {
      _revision += 1;
      _queuedRevision = null;
      _result = null;
    });
    if (!_sourceRunsOnChange) return;
    _debounce = Timer(inlineRunDebounce, () {
      _debounce = null;
      _requestRun(_RunTrigger.inputChange);
    });
  }

  void _requestRun(_RunTrigger trigger) {
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
    // `_running` flips before the first await so a second trigger — and a
    // second approval dialog — cannot slip in behind this one.
    setState(() {
      _running = true;
      _runningRevision = revision;
      _result = null;
      _tail = const LiveTail.empty(maxLines: inlineLiveTailMaxLines);
    });
    final verdict = await resolveApprovalBeforeDispatch(
      context: context,
      ref: ref,
      tool: widget.tool,
    );
    if (!mounted) return;
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
    final dispatch = ref.read(dispatchStreamFnProvider);
    final runningTools = ref.read(runningToolsProvider.notifier);
    final runId = nextDispatchRunId();
    setState(() => _runId = runId);
    final runningLease = runningTools.begin(_toolId);

    CanonicalToolResult outcome = _safeFailureResult();
    try {
      final events = dispatch(
        toolId: _toolId,
        args: args,
        approve: approve,
        runId: runId,
      );
      await for (final event in events) {
        switch (event) {
          case DispatchStreamEventDto_Chunk(:final chunk):
            if (mounted) setState(() => _tail = _tail.append(chunk));
          case DispatchStreamEventDto_Done(:final result):
            outcome = result;
        }
      }
    } on Exception {
      outcome = _safeFailureResult();
    } finally {
      runningTools.end(runningLease);
    }
    if (!mounted) return;

    final acceptResult = revision == _revision;
    final rerunLatest =
        _queuedRevision == _revision && _allOk && _sourceRunsOnChange;
    setState(() {
      _running = false;
      _runningRevision = null;
      _queuedRevision = null;
      _runId = null;
      if (acceptResult) {
        _result = outcome.ok ? outcome : _safeFailureResult();
      }
    });
    if (acceptResult) _revealOutput();
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
    // One scroll view for the whole body: the shared GenericFormWidget is a
    // plain (non-scrolling) Column sized for a modal, so on a short tile it
    // would overflow — letting the body scroll absorbs any input/output that
    // does not fit, and the tile footprint never changes.
    return Padding(
      padding: UpegSizing.pinBodyPadding,
      child: SingleChildScrollView(
        controller: _scrollController,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            if (widget.tool.inputFields.isNotEmpty)
              GenericFormWidget(
                tool: widget.tool,
                controller: _form,
                compact: true,
                onValidationChanged: (ok) {
                  if (ok != _allOk) setState(() => _allOk = ok);
                },
              ),
            if (_needsRunButton) _runButton(),
            ?_sourceHint(tokens),
            if (_runId != null) _runningStrip(),
            if (_runId != null && _tail.isNotEmpty)
              Padding(
                key: inlineLiveOutputTailKey,
                padding: const EdgeInsets.only(top: _inlineOutputRowGap),
                child: LiveOutputTailView(
                  tail: _tail,
                  fontSize: _inlineTailFontSize,
                ),
              ),
            const SizedBox(height: _inlineOutputGap),
            _output(tokens),
          ],
        ),
      ),
    );
  }

  /// The in-flight strip: the way out of a run that is taking too long,
  /// plus the indicator.
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
        onPressed: _allOk && !_running
            ? () => _requestRun(_RunTrigger.manual)
            : null,
        child: Text(t(ref, 'inline.run')),
      ),
    );
  }

  Widget? _sourceHint(UpegTokens tokens) {
    // "runs on input" is a lie for a tool that declares no inputs; the
    // Run button already says what triggers it. Same for a gated tool:
    // its declared source never fires, only the Run button does.
    if (_requiresApproval) return null;
    if (widget.tool.inputFields.isEmpty &&
        widget.tool.source is! SourceDto_Timer) {
      return null;
    }
    final text = switch (widget.tool.source) {
      SourceDto_UserInput() => inlineUserInputSourceHint,
      SourceDto_Timer(:final intervalMs) => inlineTimerSourceHint(intervalMs),
      SourceDto_Shortcut(:final keys) => inlineShortcutSourceHint(keys),
      SourceDto_Static() => inlineStaticSourceHint,
      SourceDto_Manual() => null,
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

  Widget _output(UpegTokens tokens) {
    final result = _result;
    if (result == null) {
      return const SizedBox.shrink();
    }
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
