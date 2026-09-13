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

/// WCAG 2.x 상대 휘도. 프로덕션 헬퍼(`Color.computeLuminance` /
/// `UpegTokens.contrastRatio`)에 기대지 않고 공식
/// (https://www.w3.org/TR/WCAG21/#dfn-relative-luminance)을 테스트
/// 안에서 독립 구현해 교차 검증한다.
double _relativeLuminance(Color c) {
  double linear(double channel) => channel <= 0.03928
      ? channel / 12.92
      : math.pow((channel + 0.055) / 1.055, 2.4).toDouble();
  return 0.2126 * linear(c.r) + 0.7152 * linear(c.g) + 0.0722 * linear(c.b);
}

/// WCAG 2.x 대비율 (1..21).
double _contrast(Color a, Color b) {
  final la = _relativeLuminance(a);
  final lb = _relativeLuminance(b);
  return (math.max(la, lb) + 0.05) / (math.min(la, lb) + 0.05);
}

void main() {
  group('UpegPinKind.tryParse', () {
    test('UpegPinKind는_대소문자에_상관없이_파싱된다', () {
      expect(UpegPinKind.tryParse('Inline'), UpegPinKind.inline);
      expect(UpegPinKind.tryParse('INLINE'), UpegPinKind.inline);
      expect(UpegPinKind.tryParse('llm'), UpegPinKind.llm);
    });

    test('UpegPinKind는_미상_변형에_null을_반환한다', () {
      expect(UpegPinKind.tryParse(null), isNull);
      expect(UpegPinKind.tryParse('NotAVariant'), isNull);
    });

    test('UpegPinKind_label은_대문자이름을_반환한다', () {
      expect(UpegPinKind.inline.label, 'INLINE');
      expect(UpegPinKind.embed.label, 'EMBED');
    });
  });

  group('UpegTokens.pinKindColor', () {
    test('pinKindColor는_각_변형을_고유_색으로_매핑한다', () {
      const tokens = UpegTokens.dark;
      expect(tokens.pinKindColor(UpegPinKind.inline), tokens.pinInline);
      expect(tokens.pinKindColor(UpegPinKind.launcher), tokens.pinLauncher);
      expect(tokens.pinKindColor(UpegPinKind.live), tokens.pinLive);
      expect(tokens.pinKindColor(UpegPinKind.action), tokens.pinAction);
      expect(tokens.pinKindColor(UpegPinKind.embed), tokens.pinEmbed);
      expect(tokens.pinKindColor(UpegPinKind.chain), tokens.pinChain);
      expect(tokens.pinKindColor(UpegPinKind.llm), tokens.pinLlm);
    });

    test('pinKindColor는_null_변형에_accent로_폴백한다', () {
      const tokens = UpegTokens.dark;
      expect(tokens.pinKindColor(null), tokens.accent);
    });
  });

  group('UpegTokens dark vs light', () {
    test('darkTheme과_lightTheme은_다른_배경색을_사용한다', () {
      final dark = UpegTheme.darkTheme().extension<UpegTokens>()!;
      final light = UpegTheme.lightTheme().extension<UpegTokens>()!;
      expect(dark.bg, isNot(equals(light.bg)));
    });
  });

  group('UpegTokens.onAccent / onWarn (WCAG AA)', () {
    test('onAccent는_모든_accent_밝기_조합에서_WCAG_AA_대비를_만족한다', () {
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
            reason: '$accent/$brightness 대비 $ratio:1',
          );
        }
      }
    });

    test('onWarn은_라이트_다크_warn_배경에서_WCAG_AA_대비를_만족한다', () {
      for (final tokens in [UpegTokens.dark, UpegTokens.light]) {
        final ratio = _contrast(tokens.warn, tokens.onWarn);
        expect(ratio, greaterThanOrEqualTo(UpegTokens.minAaContrast));
      }
    });

    test('기본_토큰백의_onAccent도_WCAG_AA_대비를_만족한다', () {
      // `context.upeg`가 테마 미설치 시 폴백하는 const 토큰백 경로.
      for (final tokens in [UpegTokens.dark, UpegTokens.light]) {
        final ratio = _contrast(tokens.accent, tokens.onAccent);
        expect(ratio, greaterThanOrEqualTo(UpegTokens.minAaContrast));
      }
    });

    test('다크_onAccent는_기존_잉크색을_유지한다', () {
      // test/goldens/*.png 다크 골든이 이 잉크 픽셀에 고정돼 있다 —
      // 값이 바뀌면 `flutter test --update-goldens` 재베이스라인 필요.
      expect(UpegTokens.dark.onAccent, const Color(0xFF07120A));
    });

    test('onFill은_잉크가_AA에_못_미치는_fill에서_흑백_중_고대비를_고른다', () {
      // 라이트 pink accent: 브랜드 잉크 3.45:1 → 흰색(5.53:1)으로 폴백.
      expect(UpegTokens.onFill(const Color(0xFFB2415A)), Colors.white);
      // 다크 green accent: 브랜드 잉크가 9.8:1로 통과 → 잉크 유지.
      expect(
        UpegTokens.onFill(const Color(0xFF5BD16C)),
        UpegTokens.dark.onAccent,
      );
      // 중간톤 회색도 흑백 중 최고 대비는 항상 AA를 만족한다.
      const midGray = Color(0xFF757575);
      expect(
        _contrast(midGray, UpegTokens.onFill(midGray)),
        greaterThanOrEqualTo(UpegTokens.minAaContrast),
      );
    });

    test('lerp와_copyWith는_onAccent_onWarn을_전달한다', () {
      final lerped = UpegTokens.dark.lerp(UpegTokens.light, 1.0);
      expect(lerped.onWarn, UpegTokens.light.onWarn);
      expect(lerped.onAccent, UpegTokens.light.onAccent);

      final overridden = UpegTokens.light.copyWith(onAccent: Colors.white);
      expect(UpegTokens.dark.lerp(overridden, 1.0).onAccent, Colors.white);
    });

    test('테마_onPrimary와_onError는_토큰을_따른다', () {
      for (final brightness in Brightness.values) {
        final theme = UpegTheme.forAccent(Accent.amber, brightness: brightness);
        final tokens = theme.extension<UpegTokens>()!;
        expect(theme.colorScheme.onPrimary, tokens.onAccent);
        expect(theme.colorScheme.onError, tokens.onWarn);
      }
    });
  });

  group('UpegTokens.focusRing / 상시 상태정보 대비 (WCAG 1.4.11)', () {
    test('focusRing은_양_테마의_bg_surface_대비_3대1_이상이다', () {
      for (final tokens in [UpegTokens.dark, UpegTokens.light]) {
        for (final backdrop in [tokens.bg, tokens.bg2, tokens.surface]) {
          expect(
            _contrast(tokens.focusRing, backdrop),
            greaterThanOrEqualTo(UpegTokens.minNonTextContrast),
            reason: 'focusRing 은 배경 위에서 비텍스트 최소 대비를 만족해야 한다',
          );
        }
      }
    });

    test('focusRing은_어떤_accent_선택과도_무관하게_동일하다', () {
      // 포커스 링은 pinColorOverride/accent 색 채널에서 분리된 전용
      // 토큰이다 — accent 를 바꿔도 값이 흔들리면 안 된다.
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

    test('상시_상태정보_잉크인_fg3는_status_bar_배경_bg2_대비_3대1_이상이다', () {
      // status_bar / pin footer 가 fg4 대신 fg3 를 쓰는 근거 — fg4 는
      // 이 하한을 만족하지 못한다(회귀 시 이 테스트가 먼저 무너진다).
      for (final tokens in [UpegTokens.dark, UpegTokens.light]) {
        for (final backdrop in [tokens.bg2, tokens.surface]) {
          expect(
            _contrast(tokens.fg3, backdrop),
            greaterThanOrEqualTo(UpegTokens.minNonTextContrast),
          );
        }
        // 전제 검증: fg4 는 실제로 하한 미달이라 상태정보에 쓸 수 없다.
        expect(
          _contrast(tokens.fg4, tokens.bg2),
          lessThan(UpegTokens.minNonTextContrast),
        );
      }
    });

    test('lerp와_copyWith는_focusRing을_전달한다', () {
      final lerped = UpegTokens.dark.lerp(UpegTokens.light, 1.0);
      expect(lerped.focusRing, UpegTokens.light.focusRing);

      final overridden = UpegTokens.dark.copyWith(focusRing: Colors.white);
      expect(overridden.focusRing, Colors.white);
    });
  });
}
