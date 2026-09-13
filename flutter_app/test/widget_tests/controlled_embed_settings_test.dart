/// Unit tests for the controlled-embed browser settings resolver.
///
/// The resolver converts manifest `ControlledEmbedSettingsDto` into explicit
/// browser settings that `WebViewPanel` applies. Tests cover all combinations
/// of User-Agent and viewport settings, proving omitted != explicit default.
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/embed.dart' as frb;
import 'package:upeg/src/widgets/controlled_embed/settings.dart';
import 'package:upeg/src/widgets/expanded_modal/webview_panel.dart'
    show kMobileUserAgent;

void main() {
  group('User-Agent resolver', () {
    test('settings가_null이면_null을_반환한다', () {
      final result = resolveBrowserSettings(null);
      expect(result, isNull);
    });

    test('settings가_있지만_userAgent가_null이면_hasUserAgentOverride_false', () {
      final result = resolveBrowserSettings(
        const frb.ControlledEmbedSettingsDto(),
      );
      expect(result, isNotNull);
      expect(result!.hasUserAgentOverride, isFalse);
      expect(result.userAgent, isNull);
      expect(result.viewportSize, isNull);
    });

    test('userAgent_생략되면_userAgent_null_+_hasUserAgentOverride_false', () {
      final result = resolveBrowserSettings(
        const frb.ControlledEmbedSettingsDto(
          viewport: frb.ControlledEmbedViewportDto_Preset(
            preset: frb.ControlledEmbedViewportPresetDto.mobile,
          ),
        ),
      );
      expect(result!.hasUserAgentOverride, isFalse);
      expect(result.userAgent, isNull);
    });

    test('mobileSafari면_kMobileUserAgent이_전달된다', () {
      final result = resolveBrowserSettings(
        const frb.ControlledEmbedSettingsDto(
          userAgent: frb.ControlledEmbedUserAgentDto.mobileSafari(),
        ),
      );
      expect(result!.hasUserAgentOverride, isTrue);
      expect(result.userAgent, kMobileUserAgent);
    });

    test('custom_문자열이_정확히_전달된다', () {
      const customUa = 'CustomBot/1.0 (compatible)';
      final result = resolveBrowserSettings(
        const frb.ControlledEmbedSettingsDto(
          userAgent: frb.ControlledEmbedUserAgentDto.custom(value: customUa),
        ),
      );
      expect(result!.hasUserAgentOverride, isTrue);
      expect(result.userAgent, customUa);
    });

    test('explicit_default이면_hasUserAgentOverride_true_+_userAgent_null', () {
      final result = resolveBrowserSettings(
        const frb.ControlledEmbedSettingsDto(
          userAgent: frb.ControlledEmbedUserAgentDto.default_(),
        ),
      );
      expect(result!.hasUserAgentOverride, isTrue);
      expect(result.userAgent, isNull);
    });
  });

  group('viewport resolver', () {
    test('viewport가_null이면_viewportSize_null', () {
      final result = resolveBrowserSettings(
        const frb.ControlledEmbedSettingsDto(),
      );
      expect(result!.viewportSize, isNull);
    });

    test('preset_mobile이면_390x844', () {
      final result = resolveBrowserSettings(
        const frb.ControlledEmbedSettingsDto(
          viewport: frb.ControlledEmbedViewportDto_Preset(
            preset: frb.ControlledEmbedViewportPresetDto.mobile,
          ),
        ),
      );
      expect(result!.viewportSize, const Size(390, 844));
    });

    test('preset_tablet이면_768x1024', () {
      final result = resolveBrowserSettings(
        const frb.ControlledEmbedSettingsDto(
          viewport: frb.ControlledEmbedViewportDto_Preset(
            preset: frb.ControlledEmbedViewportPresetDto.tablet,
          ),
        ),
      );
      expect(result!.viewportSize, const Size(768, 1024));
    });

    test('preset_desktop이면_1366x768', () {
      final result = resolveBrowserSettings(
        const frb.ControlledEmbedSettingsDto(
          viewport: frb.ControlledEmbedViewportDto_Preset(
            preset: frb.ControlledEmbedViewportPresetDto.desktop,
          ),
        ),
      );
      expect(result!.viewportSize, const Size(1366, 768));
    });

    test('custom_viewport이면_정확한_width_height가_double로_전달된다', () {
      final result = resolveBrowserSettings(
        const frb.ControlledEmbedSettingsDto(
          viewport: frb.ControlledEmbedViewportDto_Custom(
            width: 1024,
            height: 600,
          ),
        ),
      );
      expect(result!.viewportSize, const Size(1024, 600));
    });
  });

  group('combined settings', () {
    test('mobileSafari_+_mobile_viewport를_동시에_해결한다', () {
      final result = resolveBrowserSettings(
        const frb.ControlledEmbedSettingsDto(
          userAgent: frb.ControlledEmbedUserAgentDto.mobileSafari(),
          viewport: frb.ControlledEmbedViewportDto_Preset(
            preset: frb.ControlledEmbedViewportPresetDto.mobile,
          ),
        ),
      );
      expect(result!.hasUserAgentOverride, isTrue);
      expect(result.userAgent, kMobileUserAgent);
      expect(result.viewportSize, const Size(390, 844));
    });

    test('custom_UA_+_custom_viewport를_동시에_해결한다', () {
      final result = resolveBrowserSettings(
        const frb.ControlledEmbedSettingsDto(
          userAgent: frb.ControlledEmbedUserAgentDto.custom(value: 'MyBot/2.0'),
          viewport: frb.ControlledEmbedViewportDto_Custom(
            width: 500,
            height: 800,
          ),
        ),
      );
      expect(result!.hasUserAgentOverride, isTrue);
      expect(result.userAgent, 'MyBot/2.0');
      expect(result.viewportSize, const Size(500, 800));
    });
  });
}
