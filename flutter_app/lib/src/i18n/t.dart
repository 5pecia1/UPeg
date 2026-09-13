/// Locale-aware translation helper.
///
/// `t(ref, key, [args])` reads the active [LocaleDto] via
/// [localeProvider] and dispatches to the FRB catalog. Because the
/// helper *watches* the provider, any widget that calls `t()` rebuilds
/// the moment the locale flips — that's the K05 live-rerender contract.
///
/// SoC: the catalog itself lives in Rust (`upeg-pegboard-ui/src/i18n.rs`).
/// This helper is the thin Dart adapter that joins the FRB sync calls
/// to the Riverpod-driven active locale.
///
/// The [i18nTranslateOverride] / [i18nTranslateArgsOverride] providers
/// let tests swap the FRB calls for fakes so widget tests run without
/// the native dylib being linked.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/rust/api/i18n.dart'
    as frb
    show translate, translateArgs;
import 'package:upeg/src/rust/api/i18n.dart' show LocaleDto;
import 'package:upeg/src/state/locale_provider.dart' show localeProvider;

/// Plain `translate(key, locale) -> String` function shape.
typedef I18nTranslate = String Function(String key, LocaleDto locale);

/// `translateArgs` function shape — args are positional parallel arrays
/// so the indirection mirrors the FRB call exactly.
typedef I18nTranslateArgs =
    String Function(
      String key,
      LocaleDto locale,
      List<String> argKeys,
      List<String> argVals,
    );

/// FRB `translate` adapter — swap in tests via [overrideWithValue].
final i18nTranslateOverride = Provider<I18nTranslate>(
  (ref) =>
      (key, locale) => frb.translate(key: key, locale: locale),
);

/// FRB `translateArgs` adapter.
final i18nTranslateArgsOverride = Provider<I18nTranslateArgs>(
  (ref) =>
      (key, locale, argKeys, argVals) => frb.translateArgs(
        key: key,
        locale: locale,
        argKeys: argKeys,
        argVals: argVals,
      ),
);

/// Translate `key` against the active locale. Pass `args` to substitute
/// `{name}` placeholders.
///
/// Re-rendering: `ref.watch(localeProvider)` is what guarantees the
/// containing widget rebuilds when the user flips Settings ↔ language.
String t(WidgetRef ref, String key, [Map<String, String>? args]) {
  final locale = ref.watch(localeProvider);
  return _translate(ref, locale, key, args);
}

/// Event-handler variant of [t].
///
/// `ref.watch` is a build-phase API, so callbacks (context menus,
/// dialogs launched from taps, key handlers) must not call [t]. This
/// variant reads the locale once via `ref.read` — no rebuild
/// subscription, which is fine because the produced string leaves the
/// widget tree immediately (menu labels, SnackBar copy, …).
String tRead(WidgetRef ref, String key, [Map<String, String>? args]) {
  final locale = ref.read(localeProvider);
  return _translate(ref, locale, key, args);
}

String _translate(
  WidgetRef ref,
  LocaleDto locale,
  String key,
  Map<String, String>? args,
) {
  if (args == null || args.isEmpty) {
    final translate = ref.read(i18nTranslateOverride);
    return translate(key, locale);
  }
  final translateArgs = ref.read(i18nTranslateArgsOverride);
  return translateArgs(
    key,
    locale,
    args.keys.toList(growable: false),
    args.values.toList(growable: false),
  );
}
