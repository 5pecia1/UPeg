/// Unit tests for [Accent] enum + [accentProvider] + [showHolesProvider].
///
/// The FRB layer hands `Tweaks.accent` to Dart as a string (`"Green"`,
/// `"Amber"`, `"Cyan"`, `"Pink"`). [Accent.fromFrb] is the single
/// boundary that decodes that string into the typed enum, so consumer
/// widgets never deal with stringly-typed accent values.
///
/// `tweaksLoaderProvider` is overridden so the FRB call is never
/// invoked — same pattern as `theme_mode_provider_test.dart`.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/rust/api/tweaks.dart';
import 'package:upeg/src/state/accent.dart';
import 'package:upeg/src/state/tweaks_provider.dart';

const TweaksDto _greenTweaks = TweaksDto(
  theme: 'Dark',
  accent: 'Green',
  showHoles: true,
  locale: 'En',
  localHttpHost: false,
);

const TweaksDto _pinkTweaksHidden = TweaksDto(
  theme: 'Dark',
  accent: 'Pink',
  showHoles: false,
  locale: 'En',
  localHttpHost: false,
);

void main() {
  group('Accent.fromFrb', () {
    test('Accent_fromFrb_parses_Cyan_as_cyan', () {
      expect(Accent.fromFrb('Cyan'), Accent.cyan);
    });

    test('Accent_fromFrb_maps_Green_Amber_Pink_respectively', () {
      expect(Accent.fromFrb('Green'), Accent.green);
      expect(Accent.fromFrb('Amber'), Accent.amber);
      expect(Accent.fromFrb('Pink'), Accent.pink);
    });

    test('Accent_fromFrb_falls_back_to_default_green_for_unknown_values', () {
      expect(kDefaultAccent, Accent.green);
      expect(Accent.fromFrb('Magenta'), kDefaultAccent);
      expect(Accent.fromFrb(''), kDefaultAccent);
    });

    test('Accent_frbName_returns_each_variant_FRB_label', () {
      expect(Accent.green.frbName, 'Green');
      expect(Accent.amber.frbName, 'Amber');
      expect(Accent.cyan.frbName, 'Cyan');
      expect(Accent.pink.frbName, 'Pink');
    });
  });

  group('accentProvider', () {
    test(
      'accentProvider_exposes_tweaks_accent_Green_as_Accent_green',
      () async {
        final container = ProviderContainer(
          overrides: [
            tweaksLoaderProvider.overrideWith(
              (ref) =>
                  () => _greenTweaks,
            ),
          ],
        );
        addTearDown(container.dispose);

        // Wait for the async TweaksController to settle into data.
        await container.read(tweaksProvider.future);
        expect(container.read(accentProvider), Accent.green);
      },
    );

    test(
      'accentProvider_falls_back_to_default_green_when_the_loader_throws',
      () {
        final container = ProviderContainer(
          overrides: [
            tweaksLoaderProvider.overrideWith(
              (ref) =>
                  () => throw UnimplementedError('loader failure in test'),
            ),
          ],
        );
        addTearDown(container.dispose);

        // `build` is synchronous, so the first read calls the loader at once
        // and its error lands as AsyncError; the fallback must kick in
        // instead of throwing.
        expect(container.read(accentProvider), Accent.green);
      },
    );
  });

  group('showHolesProvider', () {
    test('showHolesProvider_exposes_tweaks_showHoles_false_as_false', () async {
      final container = ProviderContainer(
        overrides: [
          tweaksLoaderProvider.overrideWith(
            (ref) =>
                () => _pinkTweaksHidden,
          ),
        ],
      );
      addTearDown(container.dispose);

      await container.read(tweaksProvider.future);
      expect(container.read(showHolesProvider), isFalse);
    });

    test('showHolesProvider_exposes_tweaks_showHoles_true_as_true', () async {
      final container = ProviderContainer(
        overrides: [
          tweaksLoaderProvider.overrideWith(
            (ref) =>
                () => _greenTweaks,
          ),
        ],
      );
      addTearDown(container.dispose);

      await container.read(tweaksProvider.future);
      expect(container.read(showHolesProvider), isTrue);
    });

    test(
      'showHolesProvider_falls_back_to_true_default_when_the_loader_throws',
      () {
        final container = ProviderContainer(
          overrides: [
            tweaksLoaderProvider.overrideWith(
              (ref) =>
                  () => throw UnimplementedError('loader failure in test'),
            ),
          ],
        );
        addTearDown(container.dispose);

        // Default matches the Rust default (show_holes: true).
        expect(container.read(showHolesProvider), isTrue);
      },
    );
  });
}
