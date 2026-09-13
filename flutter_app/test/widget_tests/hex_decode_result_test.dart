/// Pure unit tests for the `decodeHex` parser + `HexDecodeResult`
/// sealed class (Batch K1).
///
/// `decodeHex` is the SoC boundary between the `num.hex_to_decimal` bespoke
/// form widget (which owns input state) and the pure decoding logic
/// (which knows nothing about widgets). The widget calls it on every
/// keystroke and renders the typed result — empty / ok / error.
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/widgets/expanded_modal/bespoke_forms/hex_decode_result.dart';

void main() {
  group('decodeHex', () {
    test('decodeHex는_올바른_hex를_decimal로_반환한다', () {
      expect(
        decodeHex('ff'),
        const HexDecodeResult.ok(decimal: 255, hexNormalized: 'ff'),
      );
    });

    test('decodeHex는_0x_접두사를_허용한다', () {
      expect(
        decodeHex('0xFF'),
        const HexDecodeResult.ok(decimal: 255, hexNormalized: 'ff'),
      );
    });

    test('decodeHex는_앞뒤_공백을_벗긴다', () {
      expect(
        decodeHex('  ff  '),
        const HexDecodeResult.ok(decimal: 255, hexNormalized: 'ff'),
      );
    });

    test('decodeHex는_대문자를_소문자로_정규화한다', () {
      expect(
        decodeHex('CAFE'),
        const HexDecodeResult.ok(decimal: 0xcafe, hexNormalized: 'cafe'),
      );
    });

    test('decodeHex는_빈_문자열을_empty로_반환한다', () {
      expect(decodeHex(''), const HexDecodeResult.empty());
    });

    test('decodeHex는_공백만_있는_문자열을_empty로_반환한다', () {
      expect(decodeHex('   '), const HexDecodeResult.empty());
    });

    test('decodeHex는_잘못된_문자열을_error로_반환한다', () {
      final r = decodeHex('xyz');
      expect(r, isA<HexDecodeError>());
      expect((r as HexDecodeError).message, isNotEmpty);
    });

    test('decodeHex는_0x만_있을때_error로_반환한다', () {
      expect(decodeHex('0x'), isA<HexDecodeError>());
    });
  });
}
