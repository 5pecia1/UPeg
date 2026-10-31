/// Shared native state version polling.
///
/// Native durable state is stored once under the shared config root. This
/// module only observes file versions and asks existing providers to reload;
/// it does not own a separate copy of settings, boards, layouts, or selection.
///
/// Polling pauses while the window is hidden or minimized
/// ([windowShownProvider]) and refreshes once, immediately, when it is shown
/// again.
library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/platform/window_visibility.dart';
import 'package:upeg/src/rust/api/shared_state.dart';
import 'package:upeg/src/state/boards_provider.dart';
import 'package:upeg/src/state/layout_provider.dart';
import 'package:upeg/src/state/pegboard_selection_provider.dart';
import 'package:upeg/src/state/tag_provider.dart';
import 'package:upeg/src/state/tweaks_provider.dart';

const Duration kSharedStatePollInterval = Duration(milliseconds: 750);

typedef SharedStateVersionLoader = SharedStateVersionsDto Function();
typedef SharedStateRefresher = Future<void> Function();

final sharedStateVersionLoaderProvider = Provider<SharedStateVersionLoader>(
  (ref) => loadSharedStateVersions,
);

class SharedStateSyncController extends AsyncNotifier<SharedStateVersionsDto> {
  /// The loader is synchronous, so the first read of this provider captures
  /// the baseline versions at once ([AsyncData], no loading state). Later
  /// [refresh] calls diff against it.
  @override
  FutureOr<SharedStateVersionsDto> build() {
    final load = ref.watch(sharedStateVersionLoaderProvider);
    return load();
  }

  Future<void> refresh() async {
    final previous = switch (state) {
      AsyncData(:final value) => value,
      _ => null,
    };
    final load = ref.read(sharedStateVersionLoaderProvider);
    try {
      final next = load();
      if (previous != null) {
        await _applyChangedVersions(previous, next);
      }
      state = AsyncData(next);
    } on Object catch (err, stack) {
      state = AsyncError(err, stack);
      rethrow;
    }
  }

  Future<void> _applyChangedVersions(
    SharedStateVersionsDto previous,
    SharedStateVersionsDto next,
  ) async {
    if (previous.tweaks != next.tweaks) {
      ref.invalidate(tweaksProvider);
    }
    if (previous.pegboard != next.pegboard) {
      ref.invalidate(boardsProvider);
      ref.invalidate(layoutProvider);
      ref.invalidate(boardTagCountsProvider);
      ref.invalidate(tagOptionsForBoardProvider);
      ref.invalidate(countForBoardTagProvider);
      final boards = await ref.read(boardsProvider.future);
      await ref.read(pegboardSelectionProvider.notifier).restore(boards);
    }
  }
}

final sharedStateSyncProvider =
    AsyncNotifierProvider<SharedStateSyncController, SharedStateVersionsDto>(
      SharedStateSyncController.new,
    );

final sharedStateRefreshProvider = Provider<SharedStateRefresher>(
  (ref) =>
      () => ref.read(sharedStateSyncProvider.notifier).refresh(),
);

final sharedStatePollingProvider = Provider<void>((ref) {
  Future<void> refreshBestEffort() async {
    try {
      await ref.read(sharedStateSyncProvider.notifier).refresh();
    } on Object {
      // Polling is best-effort. The explicit refresh provider still surfaces
      // errors to tests/callers, but background polling must not crash boot.
    }
  }

  // Builds the baseline probe if no earlier reader has; this never adds a
  // second probe at start-up.
  ref.read(sharedStateSyncProvider);
  // A hidden or minimized window shows nothing, so the ticks below are
  // skipped; catch up with one refresh as soon as it is shown again.
  ref.listen<bool>(windowShownProvider, (previous, shown) {
    if (previous == false && shown) unawaited(refreshBestEffort());
  });
  final timer = Timer.periodic(kSharedStatePollInterval, (_) {
    if (ref.read(windowShownProvider)) unawaited(refreshBestEffort());
  });
  ref.onDispose(timer.cancel);
});
