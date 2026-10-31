import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:upeg/src/state/inline_draft_provider.dart';
import 'package:upeg/src/state/last_outcome_provider.dart';

/// Keep modal/inline widget tests independent from the native storage bridge.
final pinPersistenceTestOverrides = <Override>[
  pinDraftLoadProvider.overrideWithValue((_) => const []),
  pinDraftSaveProvider.overrideWithValue((pin, tool, args) {}),
  lastOutcomeLoadProvider.overrideWithValue((_) => const []),
  lastOutcomePersistProvider.overrideWithValue(
    ({
      required boardKey,
      required pinId,
      required toolId,
      required result,
      argsJson,
    }) {},
  ),
];
