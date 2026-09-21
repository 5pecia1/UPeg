/// Unit tests for the [UpegPinKind] parser + [UpegTokens.pinKindColor] mapping.
///
/// The Rust side hands the variant name as a string (e.g. `"Inline"`);
/// the Dart parser is case-insensitive and falls back to `null` so an
/// unknown variant degrades to the default accent colour without
/// crashing the UI.
library;

import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/state/accent.dart';
import 'package:upeg/src/theme/upeg_theme.dart';

/// WCAG 2.x relative luminance. Reimplemented independently inside the
/// test from the formula
/// (https://www.w3.org/TR/WCAG21/#dfn-relative-luminance) instead of
/// relying on the production helpers (`Color.computeLuminance` /
/// `UpegTokens.contrastRatio`), so it cross-validates them.
double _relativeLuminance(Color c) {
  double linear(double channel) => channel <= 0.03928
      ? channel / 12.92
      : math.pow((channel + 0.055) / 1.055, 2.4).toDouble();
  return 0.2126 * linear(c.r) + 0.7152 * linear(c.g) + 0.0722 * linear(c.b);
}

/// WCAG 2.x contrast ratio (1..21).
double _contrast(Color a, Color b) {
  final la = _relativeLuminance(a);
  final lb = _relativeLuminance(b);
  return (math.max(la, lb) + 0.05) / (math.min(la, lb) + 0.05);
}

void main() {
  group('UpegPinKind.tryParse', () {
    test('UpegPinKind_parses_regardless_of_case', () {
      expect(UpegPinKind.tryParse('Inline'), UpegPinKind.inline);
      expect(UpegPinKind.tryParse('INLINE'), UpegPinKind.inline);
      expect(UpegPinKind.tryParse('llm'), UpegPinKind.llm);
    });

    test('UpegPinKind_returns_null_for_an_unknown_variant', () {
      expect(UpegPinKind.tryParse(null), isNull);
      expect(UpegPinKind.tryParse('NotAVariant'), isNull);
    });

    test('the_UpegPinKind_label_returns_the_uppercase_name', () {
      expect(UpegPinKind.inline.label, 'INLINE');
      expect(UpegPinKind.embed.label, 'EMBED');
    });
  });

  group('UpegTokens.pinKindColor', () {
    test('pinKindColor_maps_each_variant_to_its_own_color', () {
      const tokens = UpegTokens.dark;
      expect(tokens.pinKindColor(UpegPinKind.inline), tokens.pinInline);
      expect(tokens.pinKindColor(UpegPinKind.launcher), tokens.pinLauncher);
      expect(tokens.pinKindColor(UpegPinKind.live), tokens.pinLive);
      expect(tokens.pinKindColor(UpegPinKind.action), tokens.pinAction);
      expect(tokens.pinKindColor(UpegPinKind.embed), tokens.pinEmbed);
      expect(tokens.pinKindColor(UpegPinKind.chain), tokens.pinChain);
      expect(tokens.pinKindColor(UpegPinKind.llm), tokens.pinLlm);
    });

    test('pinKindColor_falls_back_to_accent_for_a_null_variant', () {
      const tokens = UpegTokens.dark;
      expect(tokens.pinKindColor(null), tokens.accent);
    });
  });

  group('UpegTokens dark vs light', () {
    test('darkTheme_and_lightTheme_use_different_background_colors', () {
      final dark = UpegTheme.darkTheme().extension<UpegTokens>()!;
      final light = UpegTheme.lightTheme().extension<UpegTokens>()!;
      expect(dark.bg, isNot(equals(light.bg)));
    });
  });

  group('UpegTokens.onAccent / onWarn (WCAG AA)', () {
    test(
      'onAccent_satisfies_WCAG_AA_contrast_for_every_accent_brightness_combination',
      () {
        for (final accent in Accent.values) {
          for (final brightness in Brightness.values) {
            final tokens = UpegTheme.forAccent(
              accent,
              brightness: brightness,
            ).extension<UpegTokens>()!;
            final ratio = _contrast(tokens.accent, tokens.onAccent);
            expect(
              ratio,
              greaterThanOrEqualTo(UpegTokens.minAaContrast),
              reason: '$accent/$brightness contrast $ratio:1',
            );
          }
        }
      },
    );

    test(
      'onWarn_satisfies_WCAG_AA_contrast_on_the_light_and_dark_warn_backgrounds',
      () {
        for (final tokens in [UpegTokens.dark, UpegTokens.light]) {
          final ratio = _contrast(tokens.warn, tokens.onWarn);
          expect(ratio, greaterThanOrEqualTo(UpegTokens.minAaContrast));
        }
      },
    );

    test('the_default_token_bag_onAccent_also_satisfies_WCAG_AA_contrast', () {
      // The const token-bag path `context.upeg` falls back to when no
      // theme is installed.
      for (final tokens in [UpegTokens.dark, UpegTokens.light]) {
        final ratio = _contrast(tokens.accent, tokens.onAccent);
        expect(ratio, greaterThanOrEqualTo(UpegTokens.minAaContrast));
      }
    });

    test('dark_onAccent_keeps_the_existing_ink_color', () {
      // The dark goldens in test/goldens/*.png are pinned to this ink
      // pixel — changing the value requires a
      // `flutter test --update-goldens` re-baseline.
      expect(UpegTokens.dark.onAccent, const Color(0xFF07120A));
    });

    test(
      'onFill_picks_the_higher_contrast_of_black_or_white_on_fills_where_the_ink_falls_short_of_AA',
      () {
        // Light pink accent: brand ink 3.45:1 → falls back to white
        // (5.53:1).
        expect(UpegTokens.onFill(const Color(0xFFB2415A)), Colors.white);
        // Dark green accent: brand ink passes at 9.8:1 → keep the ink.
        expect(
          UpegTokens.onFill(const Color(0xFF5BD16C)),
          UpegTokens.dark.onAccent,
        );
        // For a mid-tone gray too, the best of black/white always
        // satisfies AA.
        const midGray = Color(0xFF757575);
        expect(
          _contrast(midGray, UpegTokens.onFill(midGray)),
          greaterThanOrEqualTo(UpegTokens.minAaContrast),
        );
      },
    );

    test('lerp_and_copyWith_propagate_onAccent_and_onWarn', () {
      final lerped = UpegTokens.dark.lerp(UpegTokens.light, 1.0);
      expect(lerped.onWarn, UpegTokens.light.onWarn);
      expect(lerped.onAccent, UpegTokens.light.onAccent);

      final overridden = UpegTokens.light.copyWith(onAccent: Colors.white);
      expect(UpegTokens.dark.lerp(overridden, 1.0).onAccent, Colors.white);
    });

    test('theme_onPrimary_and_onError_follow_the_tokens', () {
      for (final brightness in Brightness.values) {
        final theme = UpegTheme.forAccent(Accent.amber, brightness: brightness);
        final tokens = theme.extension<UpegTokens>()!;
        expect(theme.colorScheme.onPrimary, tokens.onAccent);
        expect(theme.colorScheme.onError, tokens.onWarn);
      }
    });
  });

  group('UpegTokens.focusRing / always-on status info contrast (WCAG 1.4.11)', () {
    test(
      'focusRing_has_at_least_3_to_1_contrast_against_both_themes_bg_and_surface',
      () {
        for (final tokens in [UpegTokens.dark, UpegTokens.light]) {
          for (final backdrop in [tokens.bg, tokens.bg2, tokens.surface]) {
            expect(
              _contrast(tokens.focusRing, backdrop),
              greaterThanOrEqualTo(UpegTokens.minNonTextContrast),
              reason:
                  'focusRing must satisfy the non-text minimum contrast over the background',
            );
          }
        }
      },
    );

    test('focusRing_is_identical_regardless_of_the_accent_choice', () {
      // The focus ring is a dedicated token decoupled from the
      // pinColorOverride/accent color channels — changing the accent
      // must not move its value.
      for (final brightness in Brightness.values) {
        final base = brightness == Brightness.dark
            ? UpegTokens.dark
            : UpegTokens.light;
        for (final accent in Accent.values) {
          final tokens = UpegTheme.forAccent(
            accent,
            brightness: brightness,
          ).extension<UpegTokens>()!;
          expect(tokens.focusRing, base.focusRing);
        }
      }
    });

    test(
      'fg3_the_always_on_status_info_ink_has_at_least_3_to_1_contrast_against_the_status_bar_bg2',
      () {
        // Why status_bar / pin footer use fg3 instead of fg4 — fg4 misses
        // this floor (on a regression this test falls first).
        for (final tokens in [UpegTokens.dark, UpegTokens.light]) {
          for (final backdrop in [tokens.bg2, tokens.surface]) {
            expect(
              _contrast(tokens.fg3, backdrop),
              greaterThanOrEqualTo(UpegTokens.minNonTextContrast),
            );
          }
          // Premise check: fg4 really is below the floor, so it cannot
          // carry status info.
          expect(
            _contrast(tokens.fg4, tokens.bg2),
            lessThan(UpegTokens.minNonTextContrast),
          );
        }
      },
    );

    test('lerp_and_copyWith_propagate_focusRing', () {
      final lerped = UpegTokens.dark.lerp(UpegTokens.light, 1.0);
      expect(lerped.focusRing, UpegTokens.light.focusRing);

      final overridden = UpegTokens.dark.copyWith(focusRing: Colors.white);
      expect(overridden.focusRing, Colors.white);
    });
  });
}
