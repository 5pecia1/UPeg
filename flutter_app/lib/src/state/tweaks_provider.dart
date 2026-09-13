/// Tweaks (theme / accent / locale / peg-holes) state.
///
/// Mirrors the Rust `Tweaks` record via FRB. The current persisted
/// value is loaded eagerly through [tweaksProvider]; the Settings
/// form reads + writes via [TweaksController.update], which calls
/// `saveTweaks` on the Rust side and updates the in-memory copy on
/// success. Failures surface as an `AsyncError` so the Settings UI
/// can show them inline.
library;

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
  Future<TweaksDto> build() async {
    final load = ref.watch(tweaksLoaderProvider);
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

/// Default accent painted before tweaks load. Cyan was the canonical
/// accent in the original CSS design tokens — a default inherited from
/// the retired Dioxus surface.
const Accent kDefaultAccent = Accent.cyan;

/// Default `show_holes` flag painted before tweaks load. Matches the
/// Rust default (`Tweaks::default().show_holes == true`).
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
