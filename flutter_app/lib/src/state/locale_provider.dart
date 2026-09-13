/// Active-locale provider.
///
/// Derives the typed [LocaleDto] from `tweaksProvider.locale`, which
/// carries the serde-encoded variant name as a `String` (`"En"` /
/// `"Ko"`) at the FRB boundary. Widgets that need locale-aware
/// strings watch this provider — not `tweaksProvider` — so they
/// rebuild only when the locale actually changes, not on every theme
/// or accent flip.
///
/// While `tweaksProvider` is still loading, the provider returns
/// [LocaleDto.en] so first-frame renders fall back to the canonical
/// English catalog. An unknown serde name also falls back to English;
/// the FRB validator on `save_tweaks` already rejects unknown values
/// so this branch is a defensive default, not a runtime contract.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/rust/api/i18n.dart' show LocaleDto;
import 'package:upeg/src/rust/api/tweaks.dart' show TweaksDto;
import 'package:upeg/src/state/tweaks_provider.dart' show tweaksProvider;

final localeProvider = Provider<LocaleDto>((ref) {
  final tw = ref.watch(tweaksProvider);
  return tw.maybeWhen(data: _decodeLocale, orElse: () => LocaleDto.en);
});

/// Maps the serde-encoded `Locale` variant name to the typed FRB enum.
LocaleDto _decodeLocale(TweaksDto tweaks) {
  return switch (tweaks.locale) {
    'En' => LocaleDto.en,
    'Ko' => LocaleDto.ko,
    _ => LocaleDto.en,
  };
}
