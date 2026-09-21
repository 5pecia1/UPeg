/// Widget tests for [UpegTheme.forAccent] + MaterialApp accent wiring.
///
/// K03: changing `tweaks.accent` must flip the live MaterialApp accent
/// (`ColorScheme.primary`) without a restart. The factory
/// `UpegTheme.forAccent(Accent, brightness:)` is the single place where
/// accent variants resolve to a token palette; consumer widgets just
/// `ref.watch(accentProvider)`.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/rust/api/tweaks.dart';
import 'package:upeg/src/state/accent.dart';
import 'package:upeg/src/state/tweaks_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';

import '../test_helpers/color_contrast.dart';

const TweaksDto _greenDark = TweaksDto(
  theme: 'Dark',
  accent: 'Green',
  showHoles: true,
  locale: 'En',
  localHttpHost: false,
);

const TweaksDto _pinkDark = TweaksDto(
  theme: 'Dark',
  accent: 'Pink',
  showHoles: true,
  locale: 'En',
  localHttpHost: false,
);

const TweaksDto _cyanLight = TweaksDto(
  theme: 'Light',
  accent: 'Cyan',
  showHoles: true,
  locale: 'En',
  localHttpHost: false,
);

/// Minimal harness that paints `MaterialApp.theme` from `forAccent` +
/// `accentProvider`. Mirrors the production `UpegApp.build` wiring but
/// stays small enough for assertion convenience.
class _AccentHarness extends ConsumerWidget {
  const _AccentHarness();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final accent = ref.watch(accentProvider);
    final theme = UpegTheme.forAccent(accent, brightness: Brightness.dark);
    final lightTheme = UpegTheme.forAccent(
      accent,
      brightness: Brightness.light,
    );
    return MaterialApp(
      key: const Key('harness-app'),
      theme: lightTheme,
      darkTheme: theme,
      themeMode: ThemeMode.dark,
      home: const Scaffold(body: SizedBox.shrink()),
    );
  }
}

void main() {
  group('UpegTheme.forAccent', () {
    test('forAccent_returns_the_green_palette_accent_for_Green_Dark', () {
      final theme = UpegTheme.forAccent(
        Accent.green,
        brightness: Brightness.dark,
      );
      final tokens = theme.extension<UpegTokens>()!;
      // Green dark palette mirrors the pre-batch hardcoded accent
      // (oklch(0.78 0.16 145) ≈ #5BD16C).
      expect(tokens.accent, const Color(0xFF5BD16C));
    });

    test('forAccent_returns_the_pink_palette_accent_for_Pink_Dark', () {
      final theme = UpegTheme.forAccent(
        Accent.pink,
        brightness: Brightness.dark,
      );
      final tokens = theme.extension<UpegTokens>()!;
      // Pink dark must be visually distinct from Green dark.
      expect(tokens.accent, isNot(equals(const Color(0xFF5BD16C))));
    });

    test('forAccent_returns_the_cyan_palette_accent_for_Cyan_Light', () {
      final theme = UpegTheme.forAccent(
        Accent.cyan,
        brightness: Brightness.light,
      );
      final tokens = theme.extension<UpegTokens>()!;
      // Cyan light must differ from Green light (the previous hardcoded
      // value #2E8C44).
      expect(tokens.accent, isNot(equals(const Color(0xFF2E8C44))));
    });

    test(
      'forAccent_returns_mutually_distinct_accent_colors_for_all_four_variants',
      () {
        final greens = UpegTheme.forAccent(
          Accent.green,
          brightness: Brightness.dark,
        ).extension<UpegTokens>()!.accent;
        final amber = UpegTheme.forAccent(
          Accent.amber,
          brightness: Brightness.dark,
        ).extension<UpegTokens>()!.accent;
        final cyan = UpegTheme.forAccent(
          Accent.cyan,
          brightness: Brightness.dark,
        ).extension<UpegTokens>()!.accent;
        final pink = UpegTheme.forAccent(
          Accent.pink,
          brightness: Brightness.dark,
        ).extension<UpegTokens>()!.accent;
        final all = {greens, amber, cyan, pink};
        expect(all.length, 4, reason: 'each variant must be distinct');
      },
    );

    test(
      'every_light_theme_accent_foreground_contrasts_at_least_4_5_with_the_real_primary_background',
      () {
        for (final accent in Accent.values) {
          final scheme = UpegTheme.forAccent(
            accent,
            brightness: Brightness.light,
          ).colorScheme;

          expect(
            colorContrastRatio(scheme.onPrimary, scheme.primary),
            greaterThanOrEqualTo(minimumNormalTextContrastRatio),
            reason: '${accent.name} Run foreground must remain readable',
          );
        }
      },
    );

    test(
      'the_compact_selected_chip_secondary_foreground_background_pair_contrasts_at_least_4_5',
      () {
        for (final brightness in Brightness.values) {
          final scheme = UpegTheme.forAccent(
            Accent.green,
            brightness: brightness,
          ).colorScheme;

          expect(
            colorContrastRatio(scheme.onSecondary, scheme.secondary),
            greaterThanOrEqualTo(minimumNormalTextContrastRatio),
            reason:
                '${brightness.name} selected chip pair must remain readable',
          );
        }
      },
    );
  });

  group('MaterialApp accent wiring', () {
    testWidgets('the_App_uses_the_green_primary_when_tweaks_accent_is_Green', (
      tester,
    ) async {
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            tweaksLoaderProvider.overrideWith(
              (ref) =>
                  () => _greenDark,
            ),
          ],
          child: const _AccentHarness(),
        ),
      );
      await tester.pumpAndSettle();

      final ctx = tester.element(find.byType(Scaffold).first);
      final primary = Theme.of(ctx).colorScheme.primary;
      expect(primary, const Color(0xFF5BD16C));
    });

    testWidgets('the_App_uses_a_non_green_primary_when_tweaks_accent_is_Pink', (
      tester,
    ) async {
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            tweaksLoaderProvider.overrideWith(
              (ref) =>
                  () => _pinkDark,
            ),
          ],
          child: const _AccentHarness(),
        ),
      );
      await tester.pumpAndSettle();

      final ctx = tester.element(find.byType(Scaffold).first);
      final primary = Theme.of(ctx).colorScheme.primary;
      expect(primary, isNot(equals(const Color(0xFF5BD16C))));
    });

    testWidgets('the_App_primary_updates_after_a_tweaks_accent_save', (
      tester,
    ) async {
      final container = ProviderContainer(
        overrides: [
          tweaksLoaderProvider.overrideWith(
            (ref) =>
                () => _greenDark,
          ),
          tweaksSaverProvider.overrideWith((ref) => (_) {}),
        ],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const _AccentHarness(),
        ),
      );
      await tester.pumpAndSettle();

      final ctx1 = tester.element(find.byType(Scaffold).first);
      expect(Theme.of(ctx1).colorScheme.primary, const Color(0xFF5BD16C));

      await container.read(tweaksProvider.notifier).save(_cyanLight);
      await tester.pumpAndSettle();

      final ctx2 = tester.element(find.byType(Scaffold).first);
      expect(
        Theme.of(ctx2).colorScheme.primary,
        isNot(equals(const Color(0xFF5BD16C))),
      );
    });
  });
}
