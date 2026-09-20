/// Widget tests for the `t()` translation helper.
///
/// The contract: a `ConsumerWidget` that renders `t(ref, key)` rebuilds
/// the instant `localeProvider` reports a new value. Because the helper
/// watches the provider — not a global — it stays test-friendly without
/// loading the native dylib.
///
/// The FRB `translate` call is swapped via [i18nTranslateOverride]
/// so the test runs without the Rust catalog being linked. The fake
/// echoes the catalog mapping for `settings.section.theme`.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/rust/api/i18n.dart' show LocaleDto;
import 'package:upeg/src/rust/api/tweaks.dart';
import 'package:upeg/src/state/tweaks_provider.dart';

const TweaksDto _enDto = TweaksDto(
  theme: 'Light',
  accent: 'Green',
  showHoles: true,
  locale: 'En',
  localHttpHost: false,
);

String _fakeTranslate(String key, LocaleDto locale) {
  if (key == 'settings.section.theme') {
    return switch (locale) {
      LocaleDto.en => 'theme',
      LocaleDto.ko => '테마',
    };
  }
  return key;
}

class _Probe extends ConsumerWidget {
  const _Probe();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Text(t(ref, 'settings.section.theme'));
  }
}

void main() {
  testWidgets(
    'the_t_helper_switches_from_english_to_korean_when_localeprovider_changes',
    (tester) async {
      TweaksDto saved = _enDto;
      final container = ProviderContainer(
        overrides: [
          tweaksLoaderProvider.overrideWith(
            (ref) =>
                () => _enDto,
          ),
          tweaksSaverProvider.overrideWith(
            (ref) =>
                (TweaksDto next) => saved = next,
          ),
          i18nTranslateOverride.overrideWithValue(_fakeTranslate),
        ],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(home: Scaffold(body: _Probe())),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text('theme'), findsOneWidget);
      expect(find.text('테마'), findsNothing);

      // Flip locale to Ko via tweaks save — localeProvider re-derives.
      await container
          .read(tweaksProvider.notifier)
          .save(
            const TweaksDto(
              theme: 'Light',
              accent: 'Green',
              showHoles: true,
              locale: 'Ko',
              localHttpHost: false,
            ),
          );
      await tester.pumpAndSettle();

      expect(saved.locale, 'Ko');
      expect(find.text('theme'), findsNothing);
      expect(find.text('테마'), findsOneWidget);
    },
  );
}
