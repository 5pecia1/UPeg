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
    test('Accent_fromFrb는_Cyan을_cyan으로_파싱한다', () {
      expect(Accent.fromFrb('Cyan'), Accent.cyan);
    });

    test('Accent_fromFrb는_Green_Amber_Pink를_각각_매핑한다', () {
      expect(Accent.fromFrb('Green'), Accent.green);
      expect(Accent.fromFrb('Amber'), Accent.amber);
      expect(Accent.fromFrb('Pink'), Accent.pink);
    });

    test('Accent_fromFrb는_미지의_값을_cyan으로_폴백한다', () {
      expect(Accent.fromFrb('Magenta'), Accent.cyan);
      expect(Accent.fromFrb(''), Accent.cyan);
    });

    test('Accent_frbName은_각_변형의_FRB_라벨을_반환한다', () {
      expect(Accent.green.frbName, 'Green');
      expect(Accent.amber.frbName, 'Amber');
      expect(Accent.cyan.frbName, 'Cyan');
      expect(Accent.pink.frbName, 'Pink');
    });
  });

  group('accentProvider', () {
    test('accentProvider는_tweaks_accent_Green을_Accent_green으로_노출한다', () async {
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
    });

    test('accentProvider는_로드_전에는_cyan_기본값으로_폴백한다', () {
      final container = ProviderContainer(
        overrides: [
          // Never-completing loader keeps the AsyncNotifier in loading.
          tweaksLoaderProvider.overrideWith(
            (ref) =>
                () => throw UnimplementedError('loader not invoked in test'),
          ),
        ],
      );
      addTearDown(container.dispose);

      // Read synchronously while tweaksProvider is still in `loading`
      // (the loader is never invoked here because we never await the
      // future); the fallback must kick in instead of throwing.
      expect(container.read(accentProvider), Accent.cyan);
    });
  });

  group('showHolesProvider', () {
    test('showHolesProvider는_tweaks_showHoles_false를_false로_노출한다', () async {
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

    test('showHolesProvider는_tweaks_showHoles_true를_true로_노출한다', () async {
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

    test('showHolesProvider는_로드_전에는_true_기본값으로_폴백한다', () {
      final container = ProviderContainer(
        overrides: [
          tweaksLoaderProvider.overrideWith(
            (ref) =>
                () => throw UnimplementedError('loader not invoked in test'),
          ),
        ],
      );
      addTearDown(container.dispose);

      // Default matches the Rust default (show_holes: true).
      expect(container.read(showHolesProvider), isTrue);
    });
  });
}
