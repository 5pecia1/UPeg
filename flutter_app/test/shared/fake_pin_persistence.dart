import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/last_outcomes.dart';
import 'package:upeg/src/rust/api/pin_drafts.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/inline_draft_provider.dart';
import 'package:upeg/src/state/last_outcome_provider.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

/// Storage independent from ProviderContainer lifetime, like the native store.
class FakePinPersistence {
  final drafts = <PinKey, PinDraftDto>{};
  bool failDraftLoads = false;
  final outcomes = <PinKey, LastOutcomeDto>{};

  List<Override> get overrides => [
    pinDraftLoadProvider.overrideWithValue(loadDrafts),
    pinDraftSaveProvider.overrideWithValue(saveDraft),
    lastOutcomeLoadProvider.overrideWithValue(loadOutcomes),
    lastOutcomePersistProvider.overrideWithValue(saveOutcome),
  ];

  List<PinDraftDto> loadDrafts(BoardKey board) {
    if (failDraftLoads) throw StateError('Saved drafts cannot be read');
    return [
      for (final entry in drafts.entries)
        if (entry.key.$1 == board) entry.value,
    ];
  }

  void saveDraft(PinKey pin, ToolId tool, ToolArgs args) {
    drafts[pin] = PinDraftDto(
      pinId: pin.$2.value,
      toolId: tool.value,
      argsJson: args.encodeJson(),
    );
  }

  List<LastOutcomeDto> loadOutcomes(String board) => [
    for (final entry in outcomes.entries)
      if (entry.key.$1.value == board) entry.value,
  ];

  void saveOutcome({
    required String boardKey,
    required String pinId,
    required String toolId,
    required CanonicalToolResult result,
    String? argsJson,
  }) {
    final pin = (BoardKey.parse(boardKey), PinId.parse(pinId));
    outcomes[pin] = LastOutcomeDto(
      pinId: pinId,
      toolId: toolId,
      argsJson: argsJson,
      result: result,
      truncated: false,
      updatedAtMs: 1000,
    );
  }
}
