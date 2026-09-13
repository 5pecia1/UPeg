/// Widget tests for the PinKind-driven chrome silhouette (Batch —
/// "PinKind별 실루엣").
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
      testWidgets('${kind.name}_kind은_${kind.name}_전용_아이콘을_헤더에_렌더한다', (
        tester,
      ) async {
        await tester.pumpWidget(
          _harness(Pin(placement: _placement, pinKind: kind)),
        );

        expect(find.byIcon(pinKindChrome(kind).icon), findsOneWidget);
        // The generic dot fallback must not also be present.
        expect(find.byKey(pinKindIconKey), findsOneWidget);
      });
    }

    testWidgets('kind가_null이면_기존_dot_폴백을_유지한다', (tester) async {
      await tester.pumpWidget(
        _harness(const Pin(placement: _placement, pinKind: null)),
      );

      expect(find.byKey(pinKindIconKey), findsNothing);
      for (final kind in UpegPinKind.values) {
        expect(find.byIcon(pinKindChrome(kind).icon), findsNothing);
      }
    });

    testWidgets('button_variant_kind는_헤더에_accent_틴트_배경을_적용한다', (tester) async {
      await tester.pumpWidget(
        _harness(const Pin(placement: _placement, pinKind: UpegPinKind.action)),
      );

      expect(
        pinKindChrome(UpegPinKind.action).variant,
        PinChromeVariant.button,
      );
      final header = tester.widget<Container>(find.byKey(pinHeaderKey));
      final decoration = header.decoration! as BoxDecoration;
      expect(decoration.color, isNotNull);
      expect(decoration.color, isNot(Colors.transparent));
    });

    testWidgets('output_variant_kind는_body를_패널로_감싼다', (tester) async {
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

    testWidgets('frame_variant_kind는_body를_웹뷰_프레임으로_감싼다', (tester) async {
      await tester.pumpWidget(
        _harness(const Pin(placement: _placement, pinKind: UpegPinKind.embed)),
      );

      expect(pinKindChrome(UpegPinKind.embed).variant, PinChromeVariant.frame);
      expect(find.byKey(pinEmbedFrameKey), findsOneWidget);
      expect(find.byKey(pinOutputPanelKey), findsNothing);
    });

    testWidgets('frame_variant는_bodyOverride가_있어도_적용된다', (tester) async {
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

    testWidgets('live_variant_kind는_헤더에_갱신_인디케이터를_표시한다', (tester) async {
      await tester.pumpWidget(
        _harness(const Pin(placement: _placement, pinKind: UpegPinKind.live)),
      );

      expect(pinKindChrome(UpegPinKind.live).variant, PinChromeVariant.live);
      expect(find.byKey(pinLiveIndicatorKey), findsOneWidget);
    });

    testWidgets('live가_아닌_kind는_갱신_인디케이터가_없다', (tester) async {
      await tester.pumpWidget(
        _harness(const Pin(placement: _placement, pinKind: UpegPinKind.action)),
      );

      expect(find.byKey(pinLiveIndicatorKey), findsNothing);
    });

    testWidgets('button_variant_kind는_body_패널도_프레임도_없다', (tester) async {
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
