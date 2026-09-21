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
    test('action_returns_the_button_variant', () {
      expect(
        pinKindChrome(UpegPinKind.action).variant,
        PinChromeVariant.button,
      );
    });

    test('launcher_returns_the_button_variant', () {
      expect(
        pinKindChrome(UpegPinKind.launcher).variant,
        PinChromeVariant.button,
      );
    });

    test('chain_returns_the_button_variant', () {
      expect(pinKindChrome(UpegPinKind.chain).variant, PinChromeVariant.button);
    });

    test('llm_returns_the_button_variant', () {
      expect(pinKindChrome(UpegPinKind.llm).variant, PinChromeVariant.button);
    });

    test('inline_returns_the_output_variant', () {
      expect(
        pinKindChrome(UpegPinKind.inline).variant,
        PinChromeVariant.output,
      );
    });

    test('live_returns_the_live_variant', () {
      expect(pinKindChrome(UpegPinKind.live).variant, PinChromeVariant.live);
    });

    test('embed_returns_the_frame_variant', () {
      expect(pinKindChrome(UpegPinKind.embed).variant, PinChromeVariant.frame);
    });

    test('controlledembed_returns_the_frame_variant', () {
      expect(
        pinKindChrome(UpegPinKind.controlledEmbed).variant,
        PinChromeVariant.frame,
      );
    });

    test('all_eight_pinkinds_have_distinct_icons', () {
      final icons = {
        for (final kind in UpegPinKind.values) kind: pinKindChrome(kind).icon,
      };
      expect(icons.values.toSet(), hasLength(UpegPinKind.values.length));
    });

    test('all_eight_pinkinds_resolve_chrome_without_gaps', () {
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
