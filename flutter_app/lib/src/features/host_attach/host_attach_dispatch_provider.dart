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
import 'package:upeg/src/rust/api/tools.dart' show CanonicalToolError;
import 'package:upeg/src/state/layout_provider.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/last_outcome_provider.dart';
import 'package:upeg/src/state/pin_provider.dart';
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
      Map<PinKey, AttachDispatchResult>
    >(HostAttachDispatchNotifier.new);

/// Convenience selector: the last attach result for [toolId], or `null`.
final pinAttachResultProvider = Provider.family<AttachDispatchResult?, PinKey>((
  ref,
  pinKey,
) {
  return ref.watch(hostAttachDispatchProvider)[pinKey];
});

class HostAttachDispatchNotifier
    extends Notifier<Map<PinKey, AttachDispatchResult>> {
  @override
  Map<PinKey, AttachDispatchResult> build() =>
      const <PinKey, AttachDispatchResult>{};

  /// Dispatch [toolId] with [args] against the paired daemon, store the
  /// result, and — on success — feed the canonical outcome into
  /// `lastOutcomeProvider` for inline rendering. The currently-selected
  /// board rides along so the daemon applies the same board gate +
  /// pin-preset merge an in-process dispatch would.
  Future<AttachDispatchResult> run(
    ToolId toolId,
    ToolArgs args, {
    PinKey? pinKey,
  }) async {
    ToolArgs effectiveArgs = args;
    if (pinKey != null) {
      try {
        final layout = ref.read(layoutLoaderProvider)(
          LayoutQuery.all(pinKey.$1),
        );
        final placement = layout.placements
            .where(
              (pin) =>
                  pin.pinId == pinKey.$2.value && pin.toolId == toolId.value,
            )
            .firstOrNull;
        if (placement == null) {
          return _recordPinError(pinKey);
        }
        final presetJson = placement.argsPresetJson;
        final preset = presetJson == null
            ? ToolArgs.empty
            : ToolArgs.tryDecodeObject(presetJson);
        if (preset == null) {
          return _recordPinError(pinKey);
        }
        effectiveArgs = ToolArgs.fromJsonObject({
          ...preset.toJsonObject(),
          ...args.toJsonObject(),
        });
      } on Object {
        return _recordPinError(pinKey);
      }
    }
    final client = ref.read(attachClientProvider);
    final result = await client.dispatch(
      toolId: toolId,
      args: effectiveArgs,
      boardKey: pinKey?.$1.value ?? ref.read(currentBoardKeyProvider)?.value,
    );
    if (pinKey != null) {
      state = <PinKey, AttachDispatchResult>{...state, pinKey: result};
    }
    if (result is AttachDispatchOk && pinKey != null) {
      ref
          .read(lastOutcomeProvider.notifier)
          .record(pinKey, toolId, result.result);
    }
    return result;
  }

  AttachDispatchToolError _recordPinError(PinKey pinKey) {
    final result = AttachDispatchToolError(
      const CanonicalToolError(code: kAttachInvalidPinErrorCode, message: ''),
    );
    state = <PinKey, AttachDispatchResult>{...state, pinKey: result};
    return result;
  }
}
