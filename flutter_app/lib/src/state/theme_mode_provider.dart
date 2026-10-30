/// Derives the active [`ThemeMode`] from [`tweaksProvider`].
///
/// `MaterialApp.themeMode` cannot wait on an `AsyncValue`, so this
/// provider folds the loading + error cases into [kDefaultThemeMode].
/// `tweaksProvider` returns its value synchronously, so the first frame
/// already paints the saved theme.
/// Saving Tweaks → `tweaksProvider` re-emits → this re-emits →
/// `UpegApp.build` re-renders with the new mode, no restart.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/state/tweaks_provider.dart';

/// Fallback mode when no tweaks value is available (a failed load) and
/// for unknown variant strings. Same as the Rust default,
/// `Tweaks::default_const` in `upeg-core/src/prefs/tweaks.rs` (Light).
const ThemeMode kDefaultThemeMode = ThemeMode.light;

/// Map a serialized Tweaks `theme` variant to a Flutter [`ThemeMode`].
///
/// Public so widget tests can pin the mapping without spinning up the
/// full provider scope. The shipping Rust set is `"Dark"` and `"Light"`;
/// `"System"` is accepted defensively in case future Tweaks variants
/// surface it.
ThemeMode themeModeFromTweakValue(String value) {
  switch (value) {
    case 'Light':
      return ThemeMode.light;
    case 'Dark':
      return ThemeMode.dark;
    case 'System':
      return ThemeMode.system;
    default:
      return kDefaultThemeMode;
  }
}

final themeModeProvider = Provider<ThemeMode>((ref) {
  final asyncTweaks = ref.watch(tweaksProvider);
  return asyncTweaks.maybeWhen(
    data: (tweaks) => themeModeFromTweakValue(tweaks.theme),
    orElse: () => kDefaultThemeMode,
  );
});
