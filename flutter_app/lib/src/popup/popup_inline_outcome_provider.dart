/// Popup-local inline run outcomes.
///
/// The popup renders both OK and ERROR outcomes in place (chrome-ext
/// popup parity), while the board's shared [`lastOutcomeProvider`]
/// cache keeps its existing contract of recording only successful
/// dispatches. This provider owns the popup-side cache: every inline
/// run — success or failure — lands here so the tapped cell can show
/// an honest OK/ERROR badge plus output preview.
///
/// `lastRun` tracks the most recent inline run so the F2 shortcut can
/// copy "the result I just produced" without the user re-selecting the
/// cell.
library;

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/tools.dart' show CanonicalToolResult;
import 'package:upeg/src/rust/canonical_tool_result_view.dart';

/// Snapshot of inline outcomes for the current popup session.
@immutable
final class PopupInlineOutcomes {
  const PopupInlineOutcomes({
    this.byTool = const <ToolId, CanonicalToolResult>{},
    this.lastRun,
  });

  /// Tool id → most recent inline outcome (OK or ERROR).
  final Map<ToolId, CanonicalToolResult> byTool;

  /// The tool of the most recent inline run, or `null` before the
  /// first run. Drives the F2 copy shortcut.
  final ToolId? lastRun;

  /// The outcome of the most recent inline run, or `null`.
  CanonicalToolResult? get lastOutcome =>
      lastRun == null ? null : byTool[lastRun];
}

class PopupInlineOutcomeNotifier extends Notifier<PopupInlineOutcomes> {
  @override
  PopupInlineOutcomes build() => const PopupInlineOutcomes();

  /// Record [outcome] for [toolId] and mark it as the latest run.
  void record(ToolId toolId, CanonicalToolResult outcome) {
    state = PopupInlineOutcomes(
      byTool: <ToolId, CanonicalToolResult>{...state.byTool, toolId: outcome},
      lastRun: toolId,
    );
  }
}

final popupInlineOutcomeProvider =
    NotifierProvider<PopupInlineOutcomeNotifier, PopupInlineOutcomes>(
      PopupInlineOutcomeNotifier.new,
    );

/// Per-tool selector so a single popup cell rebuilds only when its own
/// slot changes (mirrors `pinLastOutcomeProvider`).
final popupInlineOutcomeForProvider =
    Provider.family<CanonicalToolResult?, ToolId>((ref, toolId) {
      return ref.watch(popupInlineOutcomeProvider).byTool[toolId];
    });

/// Clipboard payload for an inline outcome: the primary output text on
/// success, the canonical error message on failure. `null` when there
/// is nothing meaningful to copy (copy affordances disable themselves).
String? popupCopyTextFor(CanonicalToolResult outcome) {
  final text = outcome.ok
      ? outcome.primaryOutputText
      : (outcome.errorMessage ?? '');
  return text.isEmpty ? null : text;
}
