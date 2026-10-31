/// Tweaks (theme / accent / locale / peg-holes) state.
///
/// Mirrors the Rust `Tweaks` record via FRB. The current persisted
/// value is loaded eagerly through [tweaksProvider]; the Settings
/// form reads + writes via [TweaksController.update], which calls
/// `saveTweaks` on the Rust side and updates the in-memory copy on
/// success. Failures surface as an `AsyncError` so the Settings UI
/// can show them inline.
library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/rust/api/tweaks.dart';
import 'package:upeg/src/state/accent.dart';

/// Indirection so widget tests can override the FRB call without
/// loading the native dylib.
typedef TweaksLoader = TweaksDto Function();
typedef TweaksSaver = void Function(TweaksDto tweaks);
typedef LocaleListProvider = List<String> Function();
typedef ThemeListProvider = List<String> Function();
typedef AccentListProvider = List<String> Function();

final tweaksLoaderProvider = Provider<TweaksLoader>((ref) => loadTweaks);

final tweaksSaverProvider = Provider<TweaksSaver>(
  (ref) =>
      (TweaksDto t) => saveTweaks(tweaks: t),
);

final supportedLocalesProvider = Provider<LocaleListProvider>(
  (ref) => supportedLocales,
);

final supportedThemesProvider = Provider<ThemeListProvider>(
  (ref) => supportedThemes,
);

final supportedAccentsProvider = Provider<AccentListProvider>(
  (ref) => supportedAccents,
);

/// Async controller for the persisted tweaks.
///
/// `build()` calls into the Rust dylib via [tweaksLoaderProvider]
/// (or its test override) to seed the initial state. [update] sends
/// the new value to Rust and, on success, swaps the in-memory copy
/// so listeners re-render.
class TweaksController extends AsyncNotifier<TweaksDto> {
  @override
  FutureOr<TweaksDto> build() {
    final load = ref.watch(tweaksLoaderProvider);
    // The loader is synchronous, so return its value instead of wrapping
    // it in a Future. A non-Future return makes the very first read
    // `AsyncData`, so the first frame paints the saved theme and accent
    // with no loading frame. A throwing loader becomes `AsyncError` and
    // consumers fall back to their defaults.
    return load();
  }

  /// Persist a new tweaks record. Returns the new state on success;
  /// throws (and pushes an `AsyncError`) on failure.
  ///
  /// Named `save` (not `update`) to avoid shadowing
  /// `AsyncNotifier.update`, which has a different signature.
  Future<void> save(TweaksDto next) async {
    final saver = ref.read(tweaksSaverProvider);
    state = const AsyncValue.loading();
    state = await AsyncValue.guard(() async {
      saver(next);
      return next;
    });
  }
}

final tweaksProvider = AsyncNotifierProvider<TweaksController, TweaksDto>(
  TweaksController.new,
);

/// Fallback `show_holes` flag when no tweaks value is available (a
/// failed load). Matches the Rust default
/// (`Tweaks::default().show_holes == true`).
const bool kDefaultShowHoles = true;

/// Typed accent for consumer widgets. Decodes the stringly-typed
/// `Tweaks.accent` via [Accent.fromFrb] so the rest of the app
/// switches on the enum, not the raw FRB label.
///
/// Folds the loading + error cases into [kDefaultAccent] so MaterialApp
/// never has to wait on an `AsyncValue` before painting.
final accentProvider = Provider<Accent>((ref) {
  final asyncTweaks = ref.watch(tweaksProvider);
  return asyncTweaks.maybeWhen(
    data: (tweaks) => Accent.fromFrb(tweaks.accent),
    orElse: () => kDefaultAccent,
  );
});

/// Whether the pegboard background should draw its peg-hole grid.
/// Folds loading/error into [kDefaultShowHoles] so BoardCanvas never
/// waits on an `AsyncValue`.
final showHolesProvider = Provider<bool>((ref) {
  final asyncTweaks = ref.watch(tweaksProvider);
  return asyncTweaks.maybeWhen(
    data: (tweaks) => tweaks.showHoles,
    orElse: () => kDefaultShowHoles,
  );
});

/// Saved pin move affordance, with the product default used while tweaks
/// are loading or unavailable.
final pinHandleStyleProvider = Provider<PinHandleStyleDto>((ref) {
  final asyncTweaks = ref.watch(tweaksProvider);
  return asyncTweaks.maybeWhen(
    data: (tweaks) => tweaks.pinHandleStyle,
    orElse: () => PinHandleStyleDto.peg,
  );
});
