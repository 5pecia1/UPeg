/// Tests for the [PinKindChrome] descriptor: every [UpegPinKind]
/// resolves to a [PinChromeVariant] + a distinct icon.
///
/// This used to cover a runtime `ToolId -> PinRenderer` registry (add/
/// lookup/remove). That registry started empty, stayed empty (no
/// production caller ever registered a renderer), and was replaced by
/// a compile-time exhaustive switch keyed on `PinKind` instead —
/// `pin.dart` consumes it directly to drive the header icon + body
/// chrome. These tests pin the new contract: correct variant per kind,
/// and all eight icons distinct so every kind reads differently at a
/// glance.
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/pin_renderers/registry.dart';

void main() {
  group('pinKindChrome', () {
    test('action은_button_variant를_반환한다', () {
      expect(
        pinKindChrome(UpegPinKind.action).variant,
        PinChromeVariant.button,
      );
    });

    test('launcher는_button_variant를_반환한다', () {
      expect(
        pinKindChrome(UpegPinKind.launcher).variant,
        PinChromeVariant.button,
      );
    });

    test('chain은_button_variant를_반환한다', () {
      expect(pinKindChrome(UpegPinKind.chain).variant, PinChromeVariant.button);
    });

    test('llm은_button_variant를_반환한다', () {
      expect(pinKindChrome(UpegPinKind.llm).variant, PinChromeVariant.button);
    });

    test('inline은_output_variant를_반환한다', () {
      expect(
        pinKindChrome(UpegPinKind.inline).variant,
        PinChromeVariant.output,
      );
    });

    test('live는_live_variant를_반환한다', () {
      expect(pinKindChrome(UpegPinKind.live).variant, PinChromeVariant.live);
    });

    test('embed는_frame_variant를_반환한다', () {
      expect(pinKindChrome(UpegPinKind.embed).variant, PinChromeVariant.frame);
    });

    test('controlledEmbed는_frame_variant를_반환한다', () {
      expect(
        pinKindChrome(UpegPinKind.controlledEmbed).variant,
        PinChromeVariant.frame,
      );
    });

    test('8종_PinKind_모두_서로_다른_아이콘을_가진다', () {
      final icons = {
        for (final kind in UpegPinKind.values) kind: pinKindChrome(kind).icon,
      };
      expect(icons.values.toSet(), hasLength(UpegPinKind.values.length));
    });

    test('8종_PinKind_모두_chrome_해석이_가능하다_누락_없음', () {
      // Exhaustive-switch smoke test: if a new UpegPinKind variant is
      // ever added without updating `pinKindChrome`, this file fails to
      // *compile* (not just fail this test) — see the switch in
      // registry.dart. Iterating all current values here still gives a
      // regression signal if that invariant is ever relaxed to a
      // default branch.
      for (final kind in UpegPinKind.values) {
        expect(pinKindChrome(kind), isNotNull);
      }
    });
  });
}
