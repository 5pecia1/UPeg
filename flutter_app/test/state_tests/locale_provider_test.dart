/// Unit tests for [localeProvider].
///
/// The provider derives the typed [LocaleDto] from `tweaksProvider.locale`
/// (a serde-encoded string at the FRB boundary). Tests drive the
/// `tweaksProvider` controller via its FRB-loader override so no native
/// dylib is loaded — the seam is the same as `tweaks_provider_test.dart`.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/rust/api/i18n.dart';
import 'package:upeg/src/rust/api/tweaks.dart';
import 'package:upeg/src/state/locale_provider.dart';
import 'package:upeg/src/state/tweaks_provider.dart';

const TweaksDto _enDto = TweaksDto(
  theme: 'Light',
  accent: 'Green',
  showHoles: true,
  locale: 'En',
  localHttpHost: false,
);

const TweaksDto _koDto = TweaksDto(
  theme: 'Light',
  accent: 'Green',
  showHoles: true,
  locale: 'Ko',
  localHttpHost: false,
);

ProviderContainer _container({required TweaksDto initial}) {
  return ProviderContainer(
    overrides: [
      tweaksLoaderProvider.overrideWith(
        (ref) =>
            () => initial,
      ),
      tweaksSaverProvider.overrideWith((ref) => (TweaksDto _) {}),
    ],
  );
}

void main() {
  group('localeProvider', () {
    test(
      'localeProvider_returns_LocaleDto_en_when_tweaks_locale_is_En',
      () async {
        final container = _container(initial: _enDto);
        addTearDown(container.dispose);

        // Force the tweaks AsyncNotifier to build (seed) the initial value
        // before reading the derived provider.
        await container.read(tweaksProvider.future);

        expect(container.read(localeProvider), LocaleDto.en);
      },
    );

    test(
      'localeProvider_returns_LocaleDto_ko_when_tweaks_locale_is_Ko',
      () async {
        final container = _container(initial: _koDto);
        addTearDown(container.dispose);

        await container.read(tweaksProvider.future);

        expect(container.read(localeProvider), LocaleDto.ko);
      },
    );

    test('localeProvider_updates_when_tweaks_locale_changes', () async {
      final container = _container(initial: _enDto);
      addTearDown(container.dispose);

      await container.read(tweaksProvider.future);
      expect(container.read(localeProvider), LocaleDto.en);

      await container.read(tweaksProvider.notifier).save(_koDto);

      expect(container.read(localeProvider), LocaleDto.ko);
    });
  });
}
