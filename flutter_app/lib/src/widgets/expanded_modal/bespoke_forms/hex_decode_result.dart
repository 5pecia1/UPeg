/// Pure parser for hex → decimal decoding, separated from any widget
/// concerns so it is unit-testable without `pumpWidget` and so the
/// `HexToDecForm` stays a thin display wrapper.
///
/// Accepts `0x`-prefixed and bare hex, trims whitespace, normalises
/// to lower-case. Returns a typed `HexDecodeResult` sealed family so
/// the widget never has to dispatch on stringly-typed flags.
library;

/// Common locale-independent diagnostic for a non-hex input. Centralised
/// so tests can pin it; the UI renders [HexDecodeError.messageKey]
/// through the catalog instead of showing this literal.
const String hexDecodeErrorMessage = 'not a valid hex value';

/// Sealed result of [decodeHex]. The three variants map 1:1 to the
/// three UI states of the bespoke form: nothing typed yet, decoded
/// successfully, or input is malformed.
sealed class HexDecodeResult {
  const HexDecodeResult();

  const factory HexDecodeResult.empty() = HexDecodeEmpty;

  const factory HexDecodeResult.ok({
    required int decimal,
    required String hexNormalized,
  }) = HexDecodeOk;

  const factory HexDecodeResult.error({required String message}) =
      HexDecodeError;
}

final class HexDecodeEmpty extends HexDecodeResult {
  const HexDecodeEmpty();

  @override
  bool operator ==(Object other) => other is HexDecodeEmpty;

  @override
  int get hashCode => (HexDecodeEmpty).hashCode;
}

final class HexDecodeOk extends HexDecodeResult {
  const HexDecodeOk({required this.decimal, required this.hexNormalized});

  final int decimal;
  final String hexNormalized;

  @override
  bool operator ==(Object other) =>
      other is HexDecodeOk &&
      other.decimal == decimal &&
      other.hexNormalized == hexNormalized;

  @override
  int get hashCode => Object.hash(decimal, hexNormalized);
}

final class HexDecodeError extends HexDecodeResult {
  const HexDecodeError({required this.message});

  /// Locale-independent diagnostic for tests and logs — user-facing
  /// copy comes from [messageKey], never from this field.
  final String message;

  /// Catalog key (upeg-pegboard-ui/src/i18n.rs) for the localized
  /// presentation of this failure — render via `t(ref, key)`.
  String get messageKey => 'modal.hex.invalid';

  @override
  bool operator ==(Object other) =>
      other is HexDecodeError && other.message == message;

  @override
  int get hashCode => message.hashCode;
}

/// Parse [raw] as a hexadecimal integer. Accepts an optional `0x`
/// prefix and arbitrary surrounding whitespace. Returns
/// [HexDecodeResult.empty] when the trimmed input is empty,
/// [HexDecodeResult.ok] on success, [HexDecodeResult.error] otherwise.
HexDecodeResult decodeHex(String raw) {
  final trimmed = raw.trim().toLowerCase();
  if (trimmed.isEmpty) {
    return const HexDecodeResult.empty();
  }
  final stripped = trimmed.startsWith('0x') ? trimmed.substring(2) : trimmed;
  if (stripped.isEmpty) {
    return const HexDecodeResult.error(message: hexDecodeErrorMessage);
  }
  final value = int.tryParse(stripped, radix: 16);
  if (value == null) {
    return const HexDecodeResult.error(message: hexDecodeErrorMessage);
  }
  return HexDecodeResult.ok(decimal: value, hexNormalized: stripped);
}
