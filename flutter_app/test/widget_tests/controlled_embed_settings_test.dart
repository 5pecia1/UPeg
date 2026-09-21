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
    test('null_settings_return_null', () {
      final result = resolveBrowserSettings(null);
      expect(result, isNull);
    });

    test('settings_with_a_null_userAgent_yield_hasUserAgentOverride_false', () {
      final result = resolveBrowserSettings(
        const frb.ControlledEmbedSettingsDto(),
      );
      expect(result, isNotNull);
      expect(result!.hasUserAgentOverride, isFalse);
      expect(result.userAgent, isNull);
      expect(result.viewportSize, isNull);
    });

    test(
      'an_omitted_userAgent_yields_userAgent_null_+_hasUserAgentOverride_false',
      () {
        final result = resolveBrowserSettings(
          const frb.ControlledEmbedSettingsDto(
            viewport: frb.ControlledEmbedViewportDto_Preset(
              preset: frb.ControlledEmbedViewportPresetDto.mobile,
            ),
          ),
        );
        expect(result!.hasUserAgentOverride, isFalse);
        expect(result.userAgent, isNull);
      },
    );

    test('mobileSafari_passes_kMobileUserAgent', () {
      final result = resolveBrowserSettings(
        const frb.ControlledEmbedSettingsDto(
          userAgent: frb.ControlledEmbedUserAgentDto.mobileSafari(),
        ),
      );
      expect(result!.hasUserAgentOverride, isTrue);
      expect(result.userAgent, kMobileUserAgent);
    });

    test('a_custom_string_is_passed_through_verbatim', () {
      const customUa = 'CustomBot/1.0 (compatible)';
      final result = resolveBrowserSettings(
        const frb.ControlledEmbedSettingsDto(
          userAgent: frb.ControlledEmbedUserAgentDto.custom(value: customUa),
        ),
      );
      expect(result!.hasUserAgentOverride, isTrue);
      expect(result.userAgent, customUa);
    });

    test(
      'an_explicit_default_yields_hasUserAgentOverride_true_+_userAgent_null',
      () {
        final result = resolveBrowserSettings(
          const frb.ControlledEmbedSettingsDto(
            userAgent: frb.ControlledEmbedUserAgentDto.default_(),
          ),
        );
        expect(result!.hasUserAgentOverride, isTrue);
        expect(result.userAgent, isNull);
      },
    );
  });

  group('viewport resolver', () {
    test('a_null_viewport_yields_viewportSize_null', () {
      final result = resolveBrowserSettings(
        const frb.ControlledEmbedSettingsDto(),
      );
      expect(result!.viewportSize, isNull);
    });

    test('preset_mobile_yields_390x844', () {
      final result = resolveBrowserSettings(
        const frb.ControlledEmbedSettingsDto(
          viewport: frb.ControlledEmbedViewportDto_Preset(
            preset: frb.ControlledEmbedViewportPresetDto.mobile,
          ),
        ),
      );
      expect(result!.viewportSize, const Size(390, 844));
    });

    test('preset_tablet_yields_768x1024', () {
      final result = resolveBrowserSettings(
        const frb.ControlledEmbedSettingsDto(
          viewport: frb.ControlledEmbedViewportDto_Preset(
            preset: frb.ControlledEmbedViewportPresetDto.tablet,
          ),
        ),
      );
      expect(result!.viewportSize, const Size(768, 1024));
    });

    test('preset_desktop_yields_1366x768', () {
      final result = resolveBrowserSettings(
        const frb.ControlledEmbedSettingsDto(
          viewport: frb.ControlledEmbedViewportDto_Preset(
            preset: frb.ControlledEmbedViewportPresetDto.desktop,
          ),
        ),
      );
      expect(result!.viewportSize, const Size(1366, 768));
    });

    test('a_custom_viewport_passes_the_exact_width_and_height_as_doubles', () {
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
    test('mobileSafari_and_a_mobile_viewport_resolve_together', () {
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

    test('a_custom_UA_and_a_custom_viewport_resolve_together', () {
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
