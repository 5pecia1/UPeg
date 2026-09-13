/// Popup activation side-effects.
///
/// Single seam consumed by both the popup grid cells (tap) and the
/// popup keyboard handler (Enter): resolve the shared
/// `pin_activation_for` verdict, route it through the pure
/// [decidePopupActivationRoute], then either run the tool inline or
/// hand off to the full dashboard via the existing
/// `pendingActivationProvider` bridge.
///
/// Inline runs mirror the board's inline-first dispatch: the FRB async
/// worker executes the tool (a slow tool never freezes the popup), the
/// in-flight state flows through the shared `runningToolsProvider`, and
/// successful outcomes are recorded into the shared
/// `lastOutcomeProvider` cache — returning to the board shows the same
/// result on the pin.
library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/popup/popup_activation_route.dart';
import 'package:upeg/src/popup/popup_inline_outcome_provider.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/last_outcome_provider.dart';
import 'package:upeg/src/state/live_outcome_provider.dart';
import 'package:upeg/src/state/pending_activation_provider.dart';
import 'package:upeg/src/state/pin_activation_provider.dart';
import 'package:upeg/src/state/running_tools_provider.dart';
import 'package:upeg/src/state/window_mode_provider.dart';
import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

final popupActivationControllerProvider = Provider<PopupActivationController>(
  PopupActivationController.new,
);

class PopupActivationController {
  PopupActivationController(this._ref);

  final Ref _ref;

  /// Activate [toolId] from the popup: run inline when the shared
  /// policy says the tool needs no input, otherwise flip to the full
  /// dashboard and queue the activation for the board-side bridge.
  Future<void> activate(ToolId toolId) async {
    final activation = _ref.read(pinActivationProvider)(
      toolId: toolId,
      argsJson: ToolArgs.emptyJson,
    );
    final route = decidePopupActivationRoute(
      activation: activation,
      tool: _ref.read(toolByIdProvider(toolId)),
    );
    switch (route) {
      case PopupOpenFull(:final toolId):
        // Order matters: flip the mode first, then queue the pending
        // id — writing the id before the flip would race a board page
        // that is already mounted from a prior session (see
        // pending_activation_provider.dart).
        _ref.read(windowModeProvider.notifier).set(WindowMode.full);
        _ref.read(pendingActivationProvider.notifier).set(toolId);
      case PopupRunInline(:final toolId):
        await _runInline(toolId);
    }
  }

  Future<void> _runInline(ToolId toolId) async {
    // Double-dispatch gate (same contract as the modal Run button): a
    // second tap while the tool is still resolving is a no-op.
    if (_ref.read(runningToolsProvider).contains(toolId)) return;
    final running = _ref.read(runningToolsProvider.notifier);
    final dispatch = _ref.read(liveDispatchToolFnProvider);
    final runningLease = running.begin(toolId);
    final outcome = await dispatch(
      toolId: toolId,
      args: ToolArgs.empty,
    ).whenComplete(() => running.end(runningLease));
    _ref.read(popupInlineOutcomeProvider.notifier).record(toolId, outcome);
    if (outcome.ok) {
      // Shared cache with the board: switching to the full surface
      // shows the same result inline on the pin.
      _ref.read(lastOutcomeProvider.notifier).record(toolId, outcome);
    }
  }

  /// Copy the most recent inline result to the clipboard (F2).
  ///
  /// Returns `true` when a copy was started so the key handler can
  /// report the event as consumed; `false` when there is no result (or
  /// nothing copyable) and the key should stay unhandled.
  bool copyLatestResult() {
    final outcome = _ref.read(popupInlineOutcomeProvider).lastOutcome;
    if (outcome == null) return false;
    final text = popupCopyTextFor(outcome);
    if (text == null) return false;
    // Fire and forget: clipboard writes have no user-visible failure
    // mode worth blocking the key handler on.
    unawaited(_ref.read(clipboardWriterProvider).write(text));
    return true;
  }
}
