/// Routes an in-process-unsupported tool through the paired daemon and
/// records the outcome (Task B3).
///
/// SoC: this notifier owns only the per-tool attach dispatch state. On a
/// successful remote run it records the canonical result into
/// `lastOutcomeProvider` — the same cache a native in-process dispatch
/// writes — so the board's `_PinBody` renders remote and local results
/// through one uniform path. Failure states stay here so the board can
/// render the matching honest notice.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/features/host_attach/attach_client.dart';
import 'package:upeg/src/features/host_attach/host_attach_config_provider.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/last_outcome_provider.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

/// The attach client bound to the current pairing. Tests override with a
/// fake [AttachClient].
final attachClientProvider = Provider<AttachClient>((ref) {
  final config = ref.watch(hostAttachConfigProvider);
  return HttpAttachClient(config: config);
});

/// Per-tool attach dispatch state. Absent key = never dispatched here.
final hostAttachDispatchProvider =
    NotifierProvider<
      HostAttachDispatchNotifier,
      Map<ToolId, AttachDispatchResult>
    >(HostAttachDispatchNotifier.new);

/// Convenience selector: the last attach result for [toolId], or `null`.
final pinAttachResultProvider = Provider.family<AttachDispatchResult?, ToolId>((
  ref,
  toolId,
) {
  return ref.watch(hostAttachDispatchProvider)[toolId];
});

class HostAttachDispatchNotifier
    extends Notifier<Map<ToolId, AttachDispatchResult>> {
  @override
  Map<ToolId, AttachDispatchResult> build() =>
      const <ToolId, AttachDispatchResult>{};

  /// Dispatch [toolId] with [args] against the paired daemon, store the
  /// result, and — on success — feed the canonical outcome into
  /// `lastOutcomeProvider` for inline rendering. The currently-selected
  /// board rides along so the daemon applies the same board gate +
  /// pin-preset merge an in-process dispatch would.
  Future<void> run(ToolId toolId, ToolArgs args) async {
    final client = ref.read(attachClientProvider);
    final result = await client.dispatch(
      toolId: toolId,
      args: args,
      boardKey: ref.read(currentBoardKeyProvider)?.value,
    );
    state = <ToolId, AttachDispatchResult>{...state, toolId: result};
    if (result is AttachDispatchOk) {
      ref.read(lastOutcomeProvider.notifier).record(toolId, result.result);
    }
  }
}
