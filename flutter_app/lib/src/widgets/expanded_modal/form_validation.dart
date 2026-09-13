/// Pure validators for the generic modal form.
///
/// Each validator returns a [FieldValidation] sealed result so the
/// caller can pattern-match rather than juggle `String?` error
/// messages. Validators are pure functions (no widgets, no I/O) so
/// they live next to [FormValue] and unit tests cover them without a
/// `WidgetTester`.
///
/// The form widget aggregates per-field results and drives the Run
/// button via `validations.values.every((v) => v.isOk)`.
library;

import 'package:flutter/foundation.dart';
import 'package:upeg/src/widgets/expanded_modal/form_value.dart';

const String _requiredMessage = 'required';
const String _invalidValueMessage = 'invalid value';
const String _notNumberMessage = 'not a number';
const String _notIntegerMessage = 'not an integer';
const String _invalidFormatMessage = 'invalid format';
const String _minimumMessagePrefix = 'min ';
const String _maximumMessagePrefix = 'max ';

/// Sealed result of a single field validation pass.
@immutable
sealed class FieldValidation {
  const FieldValidation();

  bool get isOk => this is FieldValidationOk;
}

final class FieldValidationOk extends FieldValidation {
  const FieldValidationOk();
}

final class FieldValidationError extends FieldValidation {
  const FieldValidationError(this.message);
  final String message;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is FieldValidationError && other.message == message;

  @override
  int get hashCode => message.hashCode;
}

/// Required-empty check shared by every kind. Returning early keeps
/// the per-kind validators short.
FieldValidation? _requiredCheck(String raw, {required bool required}) {
  if (raw.isEmpty && required) {
    return const FieldValidationError(_requiredMessage);
  }
  if (raw.isEmpty) {
    return const FieldValidationOk();
  }
  return null;
}

/// Text validator. [pattern] mirrors the field's declared
/// `String(regex=…)` constraint: Rust re-checks it before dispatch, so
/// this only spares the user an obvious round-trip. An uncompilable
/// pattern is treated as "cannot pre-check here" and deferred to Rust.
FieldValidation validateText(
  String raw, {
  required bool required,
  String? pattern,
}) {
  final r = _requiredCheck(raw, required: required);
  if (r != null) return r;
  return _patternCheck(raw, pattern) ?? const FieldValidationOk();
}

FieldValidation validateNumber(
  String raw, {
  required bool required,
  double? min,
  double? max,
}) {
  final r = _requiredCheck(raw, required: required);
  if (r != null) return r;
  final parsed = num.tryParse(raw);
  if (parsed == null || !parsed.isFinite) {
    return const FieldValidationError(_notNumberMessage);
  }
  return _rangeCheck(parsed.toDouble(), min, max) ?? const FieldValidationOk();
}

/// Integer validator. Rejects a fractional literal (`1.5`) that
/// [validateNumber] would accept — the declared `Integer` kind is a
/// different wire type, not a rendering hint.
FieldValidation validateInteger(
  String raw, {
  required bool required,
  double? min,
  double? max,
}) {
  final r = _requiredCheck(raw, required: required);
  if (r != null) return r;
  final parsed = int.tryParse(raw.trim());
  if (parsed == null) {
    return const FieldValidationError(_notIntegerMessage);
  }
  return _rangeCheck(parsed.toDouble(), min, max) ?? const FieldValidationOk();
}

FieldValidation? _rangeCheck(double value, double? min, double? max) {
  if (min != null && value < min) {
    return FieldValidationError('$_minimumMessagePrefix${formatBound(min)}');
  }
  if (max != null && value > max) {
    return FieldValidationError('$_maximumMessagePrefix${formatBound(max)}');
  }
  return null;
}

FieldValidation? _patternCheck(String raw, String? pattern) {
  if (pattern == null || pattern.isEmpty) return null;
  final RegExp compiled;
  try {
    compiled = RegExp(pattern);
  } on FormatException {
    // Dart and Rust regex dialects are close but not identical; a
    // pattern this engine cannot compile is left for Rust to enforce.
    return null;
  }
  return compiled.hasMatch(raw)
      ? null
      : const FieldValidationError(_invalidFormatMessage);
}

/// Render a constraint bound for a message / helper text: a whole
/// number prints as `128`, not `128.0`.
String formatBound(double value) {
  if (!value.isFinite) return '$value';
  final rounded = value.roundToDouble();
  return rounded == value ? '${rounded.toInt()}' : '$value';
}

FieldValidation validateUrl(String raw, {required bool required}) {
  final r = _requiredCheck(raw, required: required);
  if (r != null) return r;
  final uri = Uri.tryParse(raw);
  if (uri == null || !uri.hasScheme || !uri.hasAuthority) {
    return const FieldValidationError('not a url');
  }
  return const FieldValidationOk();
}

FieldValidation validateBooleanValue(
  FormValue? value, {
  required bool required,
}) {
  return _validateTypedValue(
    value,
    required: required,
    isValid: value is BooleanValue,
  );
}

FieldValidation validateOptionValue(
  FormValue? value, {
  required bool required,
  required List<String> options,
}) {
  return _validateTypedValue(
    value,
    required: required,
    isValid: value is OptionValue && options.contains(value.key),
  );
}

FieldValidation validateMultiOptionValue(
  FormValue? value, {
  required bool required,
  required List<String> options,
}) {
  final isEmpty = value is MultiOptionValue && value.keys.isEmpty;
  if (isEmpty) {
    return required
        ? const FieldValidationError(_requiredMessage)
        : const FieldValidationOk();
  }
  return _validateTypedValue(
    value,
    required: required,
    isValid:
        value is MultiOptionValue &&
        value.keys.every((key) => options.contains(key)),
  );
}

FieldValidation validateDateTimeValue(
  FormValue? value, {
  required bool required,
}) {
  return _validateTypedValue(
    value,
    required: required,
    isValid: value is DateTimeValue,
  );
}

FieldValidation validateFileValue(FormValue? value, {required bool required}) {
  return _validateTypedValue(
    value,
    required: required,
    isValid: value is FileFormValue,
  );
}

FieldValidation _validateTypedValue(
  FormValue? value, {
  required bool required,
  required bool isValid,
}) {
  if (value == null) {
    return required
        ? const FieldValidationError(_requiredMessage)
        : const FieldValidationOk();
  }
  return isValid
      ? const FieldValidationOk()
      : const FieldValidationError(_invalidValueMessage);
}
