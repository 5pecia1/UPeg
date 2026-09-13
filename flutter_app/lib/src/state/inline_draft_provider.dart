/// Per-tool store of the latest inline pin-body input snapshot.
///
/// The inline body writes its current form values here on every change;
/// opening the expanded modal reads them to seed the full form, so values
/// typed inline on the tile carry over into the modal (and back, since the
/// inline body re-seeds itself from the draft on rebuild).
///
/// Deliberately imperative (a plain map behind a `Provider`) — writes must
/// not trigger rebuilds, and nothing watches it reactively.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/state/pin_provider.dart' show PinKey;
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

class InlineDraftStore {
  final Map<PinKey, ToolArgs> _drafts = <PinKey, ToolArgs>{};

  void set(PinKey pinKey, ToolArgs args) => _drafts[pinKey] = args;

  ToolArgs? read(PinKey pinKey) => _drafts[pinKey];

  void clear(PinKey pinKey) => _drafts.remove(pinKey);
}

final inlineDraftProvider = Provider<InlineDraftStore>(
  (ref) => InlineDraftStore(),
);
