/// Root observer for [launchIntentProvider].
///
/// Drains the pending [LaunchIntent] and routes each sub-action to the
/// canonical handler:
///   * `intent.board != null` ⇒ swap [currentBoardKeyProvider].
///   * `intent.tool != null` ⇒ run `pinActivationFor(tool, args)` and
///     dispatch the result through the same switch the board page
///     uses on a regular pin tap.
///
/// Clears the provider once the intent is applied so a stale value
/// doesn't re-fire on the next rebuild. Three intent sources
/// (cold-boot argv via `AppInitReport.launch`, second-instance
/// `app_links` event, in-process popup encode) all share this one
/// observer so the routing policy lives in exactly one place.
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/pages/embed_page.dart';
import 'package:upeg/src/platform/window_summon.dart' as platform_summon;
import 'package:upeg/src/pages/expanded_modal_page.dart';
import 'package:upeg/src/rust/api/pin_activation.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/launch_intent_provider.dart';
import 'package:upeg/src/state/pin_activation_provider.dart';
import 'package:upeg/src/state/tools_provider.dart';
import 'package:upeg/src/state/window_mode_provider.dart';
import 'package:upeg/src/widgets/controlled_embed/surface.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

/// Args-json passed to `pinActivationFor` when the intent doesn't
/// carry a pre-fill. An explicit empty object satisfies the FRB
/// signature without forcing the call site to import `dart:convert`.
const String _kEmptyArgsJson = ToolArgs.emptyJson;

/// Foregrounding seam. Production hands in
/// `platform/window_summon.dart::summonWindow`; tests pass a recording
/// closure.
typedef SummonWindowFn = Future<void> Function();

class LaunchIntentApplier extends ConsumerStatefulWidget {
  const LaunchIntentApplier({
    required this.child,
    this.summonWindow = platform_summon.summonWindow,
    super.key,
  });

  /// Tree to render below the observer — typically the BoardPage shell.
  final Widget child;

  /// Injected foregrounding call, fired for every non-empty intent.
  /// A deep link must surface the window even when the target mode
  /// equals the current one — the mode pipeline dedupes equal states,
  /// so show+focus needs this unconditional path (launcher
  /// immediacy). Defaults to the production `summonWindow`, which
  /// no-ops on unsupported platforms.
  final SummonWindowFn summonWindow;

  @override
  ConsumerState<LaunchIntentApplier> createState() =>
      _LaunchIntentApplierState();
}

class _LaunchIntentApplierState extends ConsumerState<LaunchIntentApplier> {
  @override
  void initState() {
    super.initState();
    // Drain any intent seeded before mount (cold-boot path: `app.dart`
    // writes the intent before the first frame). The microtask defers
    // until after the first build so `ScaffoldMessenger.maybeOf` can
    // find an ancestor, mirroring `PendingActivationBridge._drain`.
    Future<void>.microtask(_maybeApply);
  }

  void _maybeApply() {
    if (!mounted) return;
    final LaunchIntent? intent = ref.read(launchIntentProvider);
    if (intent == null || intent.isEmpty) return;

    // Foreground first: a deep link that arrives while the window is
    // hidden (popup auto-hide) or minimized must surface it even when
    // the window-mode write below dedupes to a no-op.
    unawaited(_summonBestEffort());

    if (intent.board != null) {
      ref
          .read(currentBoardKeyProvider.notifier)
          .select(BoardKey.parse(intent.board!));
    }

    if (intent.tool != null) {
      // Batch E (row D04): a tool-bearing intent always belongs on the
      // full BoardPage — popup is search-first and can't host the
      // expanded modal. Force the window mode before dispatching so
      // the activation lands on the right surface even when the user
      // was in popup at the moment the URL arrived.
      ref.read(windowModeProvider.notifier).set(WindowMode.full);

      final activation = ref.read(pinActivationProvider)(
        toolId: intent.tool!,
        argsJson: intent.inputJson ?? _kEmptyArgsJson,
      );
      _dispatch(activation, intent.inputJson ?? _kEmptyArgsJson);
    }

    ref.read(launchIntentProvider.notifier).clear();
  }

  /// Foregrounding is fire-and-forget and must never sink an intent:
  /// a missing plugin (widget tests, web) degrades to "no summon",
  /// mirroring `WindowModeApplier._applyBestEffort`.
  Future<void> _summonBestEffort() async {
    try {
      await widget.summonWindow();
    } on Object catch (err, stack) {
      debugPrint('upeg: summonWindow on launch intent failed: $err');
      debugPrint(stack.toString());
    }
  }

  void _dispatch(PinActivationDto activation, String argsJson) {
    switch (activation) {
      case PinActivationDto_OpenEmbed(:final toolId):
        unawaited(_openEmbedForToolId(ToolId.parse(toolId)));
      case PinActivationDto_DispatchImmediate(:final toolId):
        unawaited(_dispatchImmediateForToolId(ToolId.parse(toolId), argsJson));
      case PinActivationDto_OpenModal(:final toolId):
        unawaited(_openModalForToolId(ToolId.parse(toolId), argsJson));
    }
  }

  /// Run a no-form activation with the deep link's input.
  ///
  /// Awaits the tool catalog for the same reason
  /// [_openModalForToolId] does — and for one more: the args this
  /// dispatch is given are whatever [parseLaunchIntentInput] can justify
  /// against the tool's **declared input fields**, and that check has no
  /// answer until the tool is resolved. Handing the deep link's verbatim
  /// text to `dispatchTool` instead let a URL write reserved keys into
  /// the call envelope; `_upeg.approvedSteps` is caller-preserved
  /// (`upeg-runtime/src/execution.rs`), so a link could approve a gated
  /// Chain step on a surface whose approvals count.
  Future<void> _dispatchImmediateForToolId(
    ToolId toolId,
    String argsJson,
  ) async {
    await ref.read(toolsProvider.future);
    if (!mounted) return;
    final ToolDto? tool = ref.read(toolByIdProvider(toolId));
    if (tool == null) return;
    final ToolArgs args =
        parseLaunchIntentInput(argsJson, tool) ?? ToolArgs.empty;
    final outcome = await dispatchToolAsync(
      toolId: toolId.value,
      argsJson: args.encodeJson(),
      boardKey: ref.read(currentBoardKeyProvider)?.value,
      // A deep link / popup activation is not a person
      // answering an approval barrier, so it never approves.
      approve: false,
    );
    if (!mounted) return;
    final messenger = ScaffoldMessenger.maybeOf(context);
    if (messenger == null) return;
    final text = outcome.snackbarText('$toolId: ok');
    messenger.showSnackBar(SnackBar(content: Text(text)));
  }

  /// Push `ExpandedModalPage` for a modal-routed activation.
  ///
  /// Batch N6 (I06): the observer opens the modal directly with the
  /// deep link's parsed `input` so the form opens pre-filled. Awaits
  /// the tool catalog first (`toolsProvider.future`) — on cold boot the
  /// catalog is still loading, so a synchronous `toolByIdProvider`
  /// lookup would return null and silently drop a valid intent. Mirrors
  /// `PendingActivationBridge._openModalForToolId`. Unknown tool ids
  /// fall through silently once the catalog has resolved.
  Future<void> _openModalForToolId(ToolId toolId, String argsJson) async {
    await ref.read(toolsProvider.future);
    if (!mounted) return;
    final ToolDto? tool = ref.read(toolByIdProvider(toolId));
    if (tool == null) return;
    if (!mounted) return;
    // ControlledEmbed tools are inline-only — fall back to the
    // full-screen surface that mounts the inline tile inside a
    // Scaffold. The standard `ExpandedModalPage` cannot drive the
    // selector pipeline in GUI builds (its dispatchTool path hits
    // the Noop backend).
    if (tool.pinKind == PinKindDto.controlledEmbed) {
      unawaited(ControlledEmbedSurface.open(context, tool));
      return;
    }
    final ToolArgs? initial = parseLaunchIntentInput(argsJson, tool);
    if (!mounted) return;
    unawaited(
      Navigator.of(context).push<void>(
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
              ExpandedModalPage(tool: tool, initialInput: initial),
        ),
      ),
    );
  }

  /// Push `EmbedPage` for an embed-routed activation. Deep links to
  /// embed tools skip the modal entirely (mirrors `BoardPage.OpenEmbed`).
  Future<void> _openEmbedForToolId(ToolId toolId) async {
    await ref.read(toolsProvider.future);
    if (!mounted) return;
    final ToolDto? tool = ref.read(toolByIdProvider(toolId));
    if (tool == null) return;
    if (!mounted) return;
    unawaited(EmbedPage.open(context, tool));
  }

  @override
  Widget build(BuildContext context) {
    // React to new writes while the observer stays mounted (second
    // intent source: `app_links` event after cold boot). The listen
    // sits in `build` so widget tests that flip the provider through
    // `container.read(...).set(...)` see the listener fire.
    ref.listen<LaunchIntent?>(launchIntentProvider, (_, next) {
      if (next != null) _maybeApply();
    });
    return widget.child;
  }
}

/// Decode a deep link's `input` into the args [tool] may actually be
/// called with.
///
/// A `upeg://open` URL is **untrusted text**: anyone who can hand the OS
/// a URL can write it, and it arrives verbatim in
/// `LaunchIntentDto.input_json`. So the tool's declared input fields are
/// the whole vocabulary a link is allowed to speak, and everything else
/// is dropped rather than forwarded. The key that made this load-bearing
/// is `_upeg.approvedSteps`: unlike every other execution-context key it
/// is preserved from the caller (`upeg-runtime/src/execution.rs`), so a
/// link carrying `{"_upeg":{"approvedSteps":["gate"]}}` used to reach the
/// Chain approval gate as a real approval, from a surface whose
/// approvals count.
///
/// Extension links carry a *raw scalar* input (e.g. `0x2a`), not a JSON
/// object, so the three shapes are:
///   * a JSON object — kept, filtered down to declared field keys;
///   * an object with nothing declared left in it (and the empty-object
///     form) — `null`, "no pre-fill";
///   * any other raw value — mapped into the tool's first input field,
///     parity with the desktop surface's
///     `InitialToolInput { tool_id, input }` mapping.
///
/// Exposed `@visibleForTesting` because it is the deep link's validation
/// boundary: both the modal pre-fill and the immediate dispatch route
/// through it, and the dispatch branch cannot be exercised in a widget
/// test without the native library.
@visibleForTesting
ToolArgs? parseLaunchIntentInput(String argsJson, ToolDto tool) {
  final ToolArgs? decoded = ToolArgs.tryDecodeObject(argsJson);
  if (decoded != null) {
    final ToolArgs declared = _onlyDeclaredFields(decoded, tool);
    return declared.isEmpty ? null : declared;
  }
  final String raw = argsJson.trim();
  if (raw.isEmpty || tool.inputFields.isEmpty) return null;
  final String fieldKey = tool.inputFields.first.key;
  return ToolArgs.fromJsonObject(<String, Object?>{fieldKey: raw});
}

/// The subset of [args] whose keys the tool declares as inputs.
ToolArgs _onlyDeclaredFields(ToolArgs args, ToolDto tool) {
  final Set<String> declared = <String>{
    for (final InputFieldDto field in tool.inputFields) field.key,
  };
  return ToolArgs.fromJsonObject(<String, Object?>{
    for (final MapEntry<String, Object?> entry in args.toJsonObject().entries)
      if (declared.contains(entry.key)) entry.key: entry.value,
  });
}
