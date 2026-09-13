/// Wraps the BoardPage tree to drain [pendingActivationProvider].
///
/// The popup writes a tool id into [pendingActivationProvider] before
/// flipping the window to full. Once this bridge mounts (or whenever a
/// new id is queued while it's already mounted) it runs
/// `pinActivationFor(toolId, '{}')`, fans the result out to the same
/// dispatch handlers the board uses, and clears the slot.
///
/// Keeping the bridge separate from `BoardPage._onPinTap` means the
/// popup → board path has no implicit dependency on the BoardPage
/// state class — the bridge speaks the same activation switch but
/// only wires the `OpenModal` / `DispatchImmediate` / `OpenExternal`
/// callbacks through whatever the host page supplies.
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/pages/embed_page.dart';
import 'package:upeg/src/rust/api/pin_activation.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/pending_activation_provider.dart';
import 'package:upeg/src/state/pin_activation_provider.dart';
import 'package:upeg/src/widgets/expanded_modal_button.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

/// Args-json passed to `pinActivationFor` for the popup-routed path.
/// The popup never collects form input; an explicit empty object keeps
/// the FRB signature satisfied without forcing the call site to import
/// `dart:convert`.
const String _kPopupActivationArgsJson = ToolArgs.emptyJson;

class PendingActivationBridge extends ConsumerStatefulWidget {
  const PendingActivationBridge({required this.child, super.key});

  /// Tree to render below the bridge — typically the BoardPage scaffold.
  final Widget child;

  @override
  ConsumerState<PendingActivationBridge> createState() =>
      _PendingActivationBridgeState();
}

class _PendingActivationBridgeState
    extends ConsumerState<PendingActivationBridge> {
  @override
  void initState() {
    super.initState();
    // Drain any id written before mount (the popup→full transition
    // commonly lands here). The microtask defers until the first frame
    // so `ScaffoldMessenger.maybeOf(context)` can find an ancestor.
    Future<void>.microtask(_drain);
  }

  void _drain() {
    if (!mounted) return;
    final toolId = ref.read(pendingActivationProvider);
    if (toolId == null) return;
    final activation = ref.read(pinActivationProvider)(
      toolId: toolId,
      argsJson: _kPopupActivationArgsJson,
    );
    unawaited(_dispatch(activation));
    ref.read(pendingActivationProvider.notifier).clear();
  }

  Future<void> _dispatch(PinActivationDto activation) async {
    switch (activation) {
      case PinActivationDto_OpenEmbed(:final toolId):
        unawaited(_openEmbedForToolId(ToolId.parse(toolId)));
      case PinActivationDto_DispatchImmediate(:final toolId):
        final parsedToolId = ToolId.parse(toolId);
        final outcome = await dispatchToolAsync(
          toolId: parsedToolId.value,
          argsJson: _kPopupActivationArgsJson,
          boardKey: ref.read(currentBoardKeyProvider)?.value,
          // A deep link / popup activation is not a person
          // answering an approval barrier, so it never approves.
          approve: false,
        );
        if (!mounted) return;
        final messenger = ScaffoldMessenger.maybeOf(context);
        if (messenger == null) return;
        final presenterText = outcome.snackbarText('$parsedToolId: ok');
        messenger.showSnackBar(SnackBar(content: Text(presenterText)));
      case PinActivationDto_OpenModal(:final toolId):
        unawaited(_openModalForToolId(ToolId.parse(toolId)));
    }
  }

  Future<void> _openModalForToolId(ToolId toolId) async {
    await ref.read(toolsProvider.future);
    if (!mounted) return;
    await openExpandedModalForToolId(context, ref, toolId);
  }

  Future<void> _openEmbedForToolId(ToolId toolId) async {
    await ref.read(toolsProvider.future);
    if (!mounted) return;
    final ToolDto? tool = ref.read(toolByIdProvider(toolId));
    if (tool == null) return;
    if (!mounted) return;
    await EmbedPage.open(context, tool);
  }

  @override
  Widget build(BuildContext context) {
    // React to new writes while the bridge stays mounted (e.g. the
    // popup is reopened from a tray and the user taps a second pin).
    ref.listen<ToolId?>(pendingActivationProvider, (_, next) {
      if (next != null) _drain();
    });
    return widget.child;
  }
}
