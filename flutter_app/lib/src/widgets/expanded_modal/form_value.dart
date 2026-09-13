/// Typed value union for the generic modal form.
///
/// `GenericFormController` holds a `Map<String, FormValue>` instead of
/// a stringly-typed `Map<String, dynamic>` so each [InputFieldType]
/// branch can read/write its own concrete shape. The Run button feeds
/// the map through [formValuesToJsonMap] which lowers each variant to
/// the JSON value the dispatcher expects (number → `num`, boolean →
/// `bool`, multi-option → `List<String>`, date-time → ISO-8601 string).
///
/// Living in its own file keeps the controller and the encoder
/// importable without dragging in the whole `Material` form widget
/// tree — pure tests (e.g. `form_value_test.dart`) can exercise the
/// encoding without booting a `WidgetTester`.
library;

import 'package:flutter/foundation.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/rust/canonical_file_value_codec.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

final RegExp _isoDateTimePattern = RegExp(
  r'^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})'
  r'(?::(\d{2})(?:[.,](\d+))?)?'
  r'(?:Z|([+-])(\d{2}):?(\d{2}))?$',
  caseSensitive: false,
);

const int _monthsInYear = 12;
const int _hoursPerDay = 24;
const int _minutesPerHour = 60;
const int _secondsPerMinute = 60;
const int _maximumIsoOffsetHours = 14;
const int _leapYearCycle = 4;
const int _centuryYears = 100;
const int _leapCenturyYears = 400;
const List<int> _daysByMonth = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

/// Largest magnitude a `double` represents exactly as an integer
/// (2^53). Beyond it `toInt()` silently loses precision.
const double _maximumExactIntegerDouble = 9007199254740992;

/// Sealed union of every value a generic-form field may hold.
///
/// Each subclass is `final` + `const`-constructible so a field
/// observer can compare instances cheaply and tests can build
/// expected snapshots inline.
@immutable
sealed class FormValue {
  const FormValue();

  /// Lower this value to a JSON-compatible primitive — the shape the
  /// dispatcher expects on the wire. Numbers and booleans stay
  /// typed; multi-options become arrays; dates serialise to ISO-8601
  /// (UTC for consistency across machines and tests).
  Object? toJson();
}

final class TextValue extends FormValue {
  const TextValue(this.value);
  final String value;

  @override
  Object toJson() => value;

  @override
  bool operator ==(Object other) =>
      identical(this, other) || other is TextValue && other.value == value;

  @override
  int get hashCode => value.hashCode;
}

final class NumberValue extends FormValue {
  const NumberValue(this.value);
  final num value;

  @override
  Object toJson() => value;

  @override
  bool operator ==(Object other) =>
      identical(this, other) || other is NumberValue && other.value == value;

  @override
  int get hashCode => value.hashCode;
}

final class BooleanValue extends FormValue {
  const BooleanValue(this.value);
  final bool value;

  @override
  Object toJson() => value;

  @override
  bool operator ==(Object other) =>
      identical(this, other) || other is BooleanValue && other.value == value;

  @override
  int get hashCode => value.hashCode;
}

final class OptionValue extends FormValue {
  const OptionValue(this.key);
  final String key;

  @override
  Object toJson() => key;

  @override
  bool operator ==(Object other) =>
      identical(this, other) || other is OptionValue && other.key == key;

  @override
  int get hashCode => key.hashCode;
}

final class MultiOptionValue extends FormValue {
  const MultiOptionValue(this.keys);
  final List<String> keys;

  @override
  List<String> toJson() => List<String>.unmodifiable(keys);

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is MultiOptionValue && listEquals(other.keys, keys);

  @override
  int get hashCode => Object.hashAll(keys);
}

final class DateTimeValue extends FormValue {
  const DateTimeValue(this.value);
  final DateTime value;

  @override
  Object toJson() => value.toUtc().toIso8601String();

  @override
  bool operator ==(Object other) =>
      identical(this, other) || other is DateTimeValue && other.value == value;

  @override
  int get hashCode => value.hashCode;
}

final class FileFormValue extends FormValue {
  const FileFormValue(this.value);
  final CanonicalFileValue value;

  @override
  Map<String, Object?> toJson() => canonicalFileValueToJson(value);

  @override
  bool operator ==(Object other) =>
      identical(this, other) || other is FileFormValue && other.value == value;

  @override
  int get hashCode => value.hashCode;
}

/// Convert one JSON value according to its declared input schema.
///
/// A mismatched value is rejected instead of being coerced to text. This
/// keeps deep-link prefill on the same typed contract as interactive input.
FormValue? formValueFromJson(InputFieldType fieldType, Object? value) {
  return switch (fieldType) {
    InputFieldType_Text() ||
    InputFieldType_Multiline() ||
    InputFieldType_Markdown() ||
    InputFieldType_FilePath() ||
    InputFieldType_Url() => value is String ? TextValue(value) : null,
    InputFieldType_File() => _fileFormValueFromJson(value),
    InputFieldType_Number() =>
      value is num && value.isFinite ? NumberValue(value) : null,
    InputFieldType_Integer() => value is int ? NumberValue(value) : null,
    InputFieldType_Boolean() => value is bool ? BooleanValue(value) : null,
    InputFieldType_Select(:final options) =>
      value is String && choiceValues(options).contains(value)
          ? OptionValue(value)
          : null,
    InputFieldType_MultiOptions(:final options) => _multiOptionValueFromJson(
      value,
      choiceValues(options),
    ),
    InputFieldType_DateTime() => _dateTimeValueFromJson(value),
  };
}

/// Canonical wire values of a choice list. Labels and descriptions are
/// presentation-only — everything that crosses the dispatch boundary or
/// gets validated compares against these.
List<String> choiceValues(List<ChoiceOptionDto> options) => <String>[
  for (final option in options) option.value,
];

/// Seed value for a freshly opened form field, derived from the field's
/// declared constraints.
///
/// Returning a [FormValue] here (rather than pre-filling text inside the
/// widget) keeps "what does this field start as" a pure, testable
/// function of the DTO — the widget just stores whatever comes back.
/// `null` means "leave the field empty".
FormValue? defaultFormValueFor(InputFieldDto field) {
  final constraints = field.constraints;
  return switch (field.fieldType) {
    // A boolean is never "unset" on the wire: an untouched switch reads
    // as false, so the form starts it there explicitly.
    InputFieldType_Boolean() => const BooleanValue(false),
    InputFieldType_Integer() => _integerDefault(constraints?.number?.default_),
    InputFieldType_Number() => switch (constraints?.number?.default_) {
      final double value when value.isFinite => NumberValue(value),
      _ => null,
    },
    InputFieldType_Text() ||
    InputFieldType_Multiline() ||
    InputFieldType_Markdown() ||
    InputFieldType_FilePath() ||
    InputFieldType_Url() => switch (constraints?.string?.default_) {
      final String value => TextValue(value),
      _ => null,
    },
    InputFieldType_File() ||
    InputFieldType_Select() ||
    InputFieldType_MultiOptions() ||
    InputFieldType_DateTime() => null,
  };
}

/// An `Integer` field must dispatch a JSON integer, so a `20.0`
/// constraint default becomes `20` — `20.0` would fail Rust's
/// integer validation.
NumberValue? _integerDefault(double? value) {
  if (value == null || !value.isFinite) return null;
  final rounded = value.roundToDouble();
  if (rounded != value || rounded.abs() > _maximumExactIntegerDouble) {
    return null;
  }
  return NumberValue(rounded.toInt());
}

FileFormValue? _fileFormValueFromJson(Object? value) {
  try {
    return switch (canonicalFileValueFromJson(value)) {
      final file? => FileFormValue(file),
      null => null,
    };
  } on CanonicalFileValueCodecException {
    return null;
  }
}

MultiOptionValue? _multiOptionValueFromJson(
  Object? value,
  List<String> options,
) {
  if (value is! List<Object?> || value.any((item) => item is! String)) {
    return null;
  }
  final keys = value.cast<String>();
  if (keys.any((key) => !options.contains(key))) return null;
  return MultiOptionValue(List<String>.unmodifiable(keys));
}

DateTimeValue? _dateTimeValueFromJson(Object? value) {
  if (value is! String) return null;
  final match = _isoDateTimePattern.firstMatch(value);
  if (match == null) return null;

  final year = int.parse(match.group(1)!);
  final month = int.parse(match.group(2)!);
  final day = int.parse(match.group(3)!);
  final hour = int.parse(match.group(4)!);
  final minute = int.parse(match.group(5)!);
  final second = switch (match.group(6)) {
    final raw? => int.parse(raw),
    null => 0,
  };
  if (month < 1 || month > _monthsInYear) return null;
  if (day < 1 || day > _daysInMonth(year, month)) return null;
  if (hour < 0 || hour >= _hoursPerDay) return null;
  if (minute < 0 || minute >= _minutesPerHour) return null;
  if (second < 0 || second >= _secondsPerMinute) return null;

  final offsetHourRaw = match.group(9);
  final offsetMinuteRaw = match.group(10);
  if (offsetHourRaw != null && offsetMinuteRaw != null) {
    final offsetHour = int.parse(offsetHourRaw);
    final offsetMinute = int.parse(offsetMinuteRaw);
    if (offsetHour > _maximumIsoOffsetHours) return null;
    if (offsetMinute >= _minutesPerHour) return null;
    if (offsetHour == _maximumIsoOffsetHours && offsetMinute != 0) return null;
  }

  final parsed = DateTime.tryParse(value);
  return parsed == null ? null : DateTimeValue(parsed);
}

int _daysInMonth(int year, int month) {
  final february = 2;
  final baseDays = _daysByMonth[month - 1];
  if (month != february || !_isLeapYear(year)) return baseDays;
  return baseDays + 1;
}

bool _isLeapYear(int year) {
  return year % _leapCenturyYears == 0 ||
      year % _centuryYears != 0 && year % _leapYearCycle == 0;
}

/// Encode a map of [FormValue]s into the JSON object shape used by
/// [ToolArgs].
Map<String, Object?> formValuesToJsonMap(Map<String, FormValue> values) {
  final out = <String, Object?>{};
  for (final entry in values.entries) {
    out[entry.key] = entry.value.toJson();
  }
  return out;
}

ToolArgs formValuesToToolArgs(Map<String, FormValue> values) {
  return ToolArgs.fromJsonObject(formValuesToJsonMap(values));
}
