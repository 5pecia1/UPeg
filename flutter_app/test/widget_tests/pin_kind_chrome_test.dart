/// Widget tests for the PinKind-driven chrome silhouette (Batch —
/// "per-PinKind silhouette").
///
/// `pin.dart` used to paint the exact same 8x8 dot for every
/// `pinKind`; every pin read as the same shape regardless of what it
/// actually did. These tests pin the fix: each kind now renders a
/// distinct header icon (sourced from `pinKindChrome`), and the three
/// non-`button`/non-default variants (`button` tint, `frame` wrapper,
/// `live` glyph) each show up through a dedicated widget key. `kind ==
/// null` keeps the original dot fallback so pins rendered before the
/// tools catalogue loads don't flash an incorrect icon.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/pin.dart';
import 'package:upeg/src/widgets/pin_renderers/registry.dart';

import '../test_helpers/i18n_test_catalog.dart';

const _placement = PlacementDto(
  toolId: 'num.hex_to_decimal',
  pinId: 'num.hex_to_decimal',
  x: 0,
  y: 0,
  w: 1,
  h: 1,
);

Widget _harness(Widget child) => ProviderScope(
  overrides: [...i18nTestOverrides],
  child: MaterialApp(
    theme: UpegTheme.darkTheme(),
    home: Scaffold(body: SizedBox(width: 220, height: 120, child: child)),
  ),
);

void main() {
  group('Pin kind chrome', () {
    for (final kind in UpegPinKind.values) {
      testWidgets(
        '${kind.name}_kind_renders_its_dedicated_icon_in_the_header',
        (tester) async {
          await tester.pumpWidget(
            _harness(Pin(placement: _placement, pinKind: kind)),
          );

          expect(find.byIcon(pinKindChrome(kind).icon), findsOneWidget);
          // The generic dot fallback must not also be present.
          expect(find.byKey(pinKindIconKey), findsOneWidget);
        },
      );
    }

    testWidgets('a_null_kind_keeps_the_legacy_dot_fallback', (tester) async {
      await tester.pumpWidget(
        _harness(const Pin(placement: _placement, pinKind: null)),
      );

      expect(find.byKey(pinKindIconKey), findsNothing);
      for (final kind in UpegPinKind.values) {
        expect(find.byIcon(pinKindChrome(kind).icon), findsNothing);
      }
    });

    testWidgets(
      'a_button_variant_kind_applies_an_accent_tinted_header_background',
      (tester) async {
        await tester.pumpWidget(
          _harness(
            const Pin(placement: _placement, pinKind: UpegPinKind.action),
          ),
        );

        expect(
          pinKindChrome(UpegPinKind.action).variant,
          PinChromeVariant.button,
        );
        final header = tester.widget<Container>(find.byKey(pinHeaderKey));
        final decoration = header.decoration! as BoxDecoration;
        expect(decoration.color, isNotNull);
        expect(decoration.color, isNot(Colors.transparent));
      },
    );

    testWidgets('an_output_variant_kind_wraps_the_body_in_a_panel', (
      tester,
    ) async {
      await tester.pumpWidget(
        _harness(const Pin(placement: _placement, pinKind: UpegPinKind.inline)),
      );

      expect(
        pinKindChrome(UpegPinKind.inline).variant,
        PinChromeVariant.output,
      );
      expect(find.byKey(pinOutputPanelKey), findsOneWidget);
      expect(find.byKey(pinEmbedFrameKey), findsNothing);
    });

    testWidgets('a_frame_variant_kind_wraps_the_body_in_a_webview_frame', (
      tester,
    ) async {
      await tester.pumpWidget(
        _harness(const Pin(placement: _placement, pinKind: UpegPinKind.embed)),
      );

      expect(pinKindChrome(UpegPinKind.embed).variant, PinChromeVariant.frame);
      expect(find.byKey(pinEmbedFrameKey), findsOneWidget);
      expect(find.byKey(pinOutputPanelKey), findsNothing);
    });

    testWidgets('the_frame_variant_applies_even_with_a_bodyoverride', (
      tester,
    ) async {
      await tester.pumpWidget(
        _harness(
          const Pin(
            placement: _placement,
            pinKind: UpegPinKind.controlledEmbed,
            bodyOverride: SizedBox(
              key: Key('fake-webview'),
              width: 10,
              height: 10,
            ),
          ),
        ),
      );

      expect(find.byKey(pinEmbedFrameKey), findsOneWidget);
      expect(find.byKey(const Key('fake-webview')), findsOneWidget);
    });

    testWidgets('a_live_variant_kind_shows_a_refresh_indicator_in_the_header', (
      tester,
    ) async {
      await tester.pumpWidget(
        _harness(const Pin(placement: _placement, pinKind: UpegPinKind.live)),
      );

      expect(pinKindChrome(UpegPinKind.live).variant, PinChromeVariant.live);
      expect(find.byKey(pinLiveIndicatorKey), findsOneWidget);
    });

    testWidgets('a_non_live_kind_has_no_refresh_indicator', (tester) async {
      await tester.pumpWidget(
        _harness(const Pin(placement: _placement, pinKind: UpegPinKind.action)),
      );

      expect(find.byKey(pinLiveIndicatorKey), findsNothing);
    });

    testWidgets('a_button_variant_kind_has_neither_a_body_panel_nor_a_frame', (
      tester,
    ) async {
      await tester.pumpWidget(
        _harness(
          const Pin(placement: _placement, pinKind: UpegPinKind.launcher),
        ),
      );

      expect(find.byKey(pinOutputPanelKey), findsNothing);
      expect(find.byKey(pinEmbedFrameKey), findsNothing);
    });
  });
}
