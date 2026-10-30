/// Mutable state shared by a generic input form and its host.
part of 'generic_form.dart';

/// Form values keyed by each input field's declared key.
///
/// State is kept out of Riverpod because it lives only while the form is
/// mounted. The host reads a typed [ToolArgs] snapshot when it runs the tool.
class GenericFormController {
  GenericFormController({this.onChanged});

  /// Optional hook fired whenever a user edit stores or removes a field value.
  /// Inline pins use it for input-triggered runs and saving drafts. Expanded
  /// forms use it for saving drafts while keeping execution explicit.
  final VoidCallback? onChanged;

  final Map<String, FormValue> _values = <String, FormValue>{};

  /// JSON-ready snapshot of the current typed field values.
  ToolArgs snapshot() => formValuesToToolArgs(_values);

  /// Typed read for fields such as switches and multi-option chips.
  FormValue? value(String key) => _values[key];

  void set(String key, FormValue value) {
    _values[key] = value;
    onChanged?.call();
  }

  /// Remove a field value in response to a user edit.
  void remove(String key) {
    _values.remove(key);
    onChanged?.call();
  }

  /// Store a widget default without presenting it as a user edit.
  void setDefault(String key, FormValue value) {
    _values.putIfAbsent(key, () => value);
  }

  /// Seed declared fields from deep-link input or a saved editing draft.
  ///
  /// Values are decoded against their field schema. Invalid and unknown
  /// entries are ignored unless [preserveInvalidText] retains malformed
  /// numeric/date editor text from an in-progress draft.
  void seed(
    ToolArgs initial, {
    required Iterable<InputFieldDto> fields,
    bool preserveInvalidText = false,
  }) {
    final input = initial.toJsonObject();
    _values.clear();
    for (final field in fields) {
      final raw = input[field.key];
      final value = formValueFromJson(field.fieldType, raw);
      if (value != null) {
        _values[field.key] = value;
        continue;
      }
      // File inputs retain their canonical structured codec. Only textual
      // number/date editor states may fall back to their raw string.
      if (preserveInvalidText && raw is String) {
        switch (field.fieldType) {
          case InputFieldType_Number():
          case InputFieldType_Integer():
          case InputFieldType_DateTime():
            _values[field.key] = TextValue(raw);
          default:
            break;
        }
      }
    }
  }

  /// Test-only escape hatch. Production code should use [set] / [value].
  @visibleForTesting
  Map<String, FormValue> debugRaw() => _values;
}
