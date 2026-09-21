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
    test('decodehex_returns_valid_hex_as_decimal', () {
      expect(
        decodeHex('ff'),
        const HexDecodeResult.ok(decimal: 255, hexNormalized: 'ff'),
      );
    });

    test('decodehex_accepts_a_0x_prefix', () {
      expect(
        decodeHex('0xFF'),
        const HexDecodeResult.ok(decimal: 255, hexNormalized: 'ff'),
      );
    });

    test('decodehex_strips_surrounding_whitespace', () {
      expect(
        decodeHex('  ff  '),
        const HexDecodeResult.ok(decimal: 255, hexNormalized: 'ff'),
      );
    });

    test('decodehex_normalizes_uppercase_to_lowercase', () {
      expect(
        decodeHex('CAFE'),
        const HexDecodeResult.ok(decimal: 0xcafe, hexNormalized: 'cafe'),
      );
    });

    test('decodehex_returns_empty_for_an_empty_string', () {
      expect(decodeHex(''), const HexDecodeResult.empty());
    });

    test('decodehex_returns_empty_for_a_whitespace_only_string', () {
      expect(decodeHex('   '), const HexDecodeResult.empty());
    });

    test('decodehex_returns_error_for_an_invalid_string', () {
      final r = decodeHex('xyz');
      expect(r, isA<HexDecodeError>());
      expect((r as HexDecodeError).message, isNotEmpty);
    });

    test('decodehex_returns_error_for_a_bare_0x', () {
      expect(decodeHex('0x'), isA<HexDecodeError>());
    });
  });
}
