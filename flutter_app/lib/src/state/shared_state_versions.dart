/// Shared native state version polling.
///
/// Native durable state is stored once under the shared config root. This
/// module only observes file versions and asks existing providers to reload;
/// it does not own a separate copy of settings, boards, layouts, or selection.
library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

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
  @override
  Future<SharedStateVersionsDto> build() async {
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

  unawaited(refreshBestEffort());
  final timer = Timer.periodic(
    kSharedStatePollInterval,
    (_) => unawaited(refreshBestEffort()),
  );
  ref.onDispose(timer.cancel);
});
