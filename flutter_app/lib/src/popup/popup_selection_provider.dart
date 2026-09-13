/// Riverpod cursor for the popup hit grid.
///
/// The notifier wraps at the list bounds so the user can press arrow
/// keys forever without hitting an invalid index. Callers pass the
/// current `hitCount` explicitly because the notifier doesn't own the
/// hit list — the list lives in `popupEffectiveHitsProvider`.
///
/// Kept distinct from the popup widget so a future hover/selection
/// indicator can rebuild only the affected cell instead of the whole
/// page.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/popup/popup_selection.dart';

class PopupSelectionNotifier extends Notifier<PopupSelection> {
  @override
  PopupSelection build() => const PopupSelection();

  /// Advance the cursor by one, wrapping at [hitCount]. No-op when
  /// [hitCount] is 0 — there is nothing to select.
  void next({required int hitCount}) {
    if (hitCount <= 0) return;
    state = PopupSelection(index: (state.index + 1) % hitCount);
  }

  /// Step the cursor back by one, wrapping at 0. No-op when [hitCount]
  /// is 0.
  void prev({required int hitCount}) {
    if (hitCount <= 0) return;
    state = PopupSelection(index: (state.index - 1 + hitCount) % hitCount);
  }

  /// Force the cursor to a specific index. Used by the M08 widget test
  /// and by future "scroll to hit" affordances.
  void resetTo(int index) {
    state = PopupSelection(index: index);
  }
}

final popupSelectionProvider =
    NotifierProvider<PopupSelectionNotifier, PopupSelection>(
      PopupSelectionNotifier.new,
    );
