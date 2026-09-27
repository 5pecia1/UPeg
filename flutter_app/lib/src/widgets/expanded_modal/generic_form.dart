/// Phase 5 generic input form.
///
/// Walks `ToolDto.inputFields` and dispatches each `InputFieldType`
/// variant to a Flutter widget via pattern matching. The form holds typed
/// [FormValue]s keyed by `InputFieldDto.key`; the host page reads a
/// [ToolArgs] snapshot when the Run button is pressed.
///
/// Bespoke per-tool forms live next door under `bespoke_forms/`; the
/// page picks bespoke first, generic second.
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/platform/file_picker_bridge.dart'
    show filePickerBridgeProvider;
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/keyboard/keyboard_command_resolver.dart';
import 'package:upeg/src/rust/api/keyboard.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/file_input_policy.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/expanded_modal/form_validation.dart';
import 'package:upeg/src/widgets/expanded_modal/image_conversion_form_policy.dart';
import 'package:upeg/src/widgets/expanded_modal/form_value.dart';
import 'package:upeg/src/widgets/expanded_modal/file_input_field.dart';
import 'package:upeg/src/widgets/expanded_modal/file_selection_policy_adapter.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

const int _datePickerYearRange = 100;
const double _expandedFieldSpacing = 12;
const int _expandedMultilineMinLines = 3;
const int _compactMultilineMinLines = 2;
const int _helperMaxLines = 3;

/// Helper-text vocabulary for declared constraints. Kept next to the
/// widget (not in the i18n catalog) so it matches the terse, symbolic
/// register of the surrounding field chrome — and mirrors the TUI
/// form's constraint line word for word.
const String _minimumHintPrefix = 'min ';
const String _maximumHintPrefix = 'max ';
const String _hintSeparator = ' · ';

/// Called whenever the form's aggregate validity flips. Passing `true`
/// means every field is `FieldValidationOk`; the host (the expanded
/// modal page) wires this to the Run button's `enabled` state.
typedef OnValidationChanged = void Function(bool allOk);

/// Mutable form state shared between [GenericFormWidget] and its host.
///
/// Kept out of Riverpod on purpose: form state is short-lived (alive only
/// while the modal is open) and tightly coupled to the widget tree, so a
/// plain ChangeNotifier-shaped controller is the simpler fit.
///
/// State is typed via the [FormValue] sealed class — each FieldKind
/// stores its own concrete shape so dispatch sends typed JSON
/// (number → `num`, boolean → `bool`, multi-option → array) rather
/// than the old stringly-typed map.
class GenericFormController {
  GenericFormController({this.onChanged});

  /// Optional hook fired whenever a user edit stores or removes a field value.
  /// Lets an inline pin body re-run the tool on-change; the expanded modal
  /// leaves it null (it runs on an explicit button instead).
  final VoidCallback? onChanged;

  final Map<String, FormValue> _values = <String, FormValue>{};

  /// JSON-ready snapshot of the current field values. Lowers each
  /// [FormValue] through its `toJson()` so callers can pass a typed
  /// envelope to the FRB boundary — numbers stay numbers, booleans stay
  /// booleans, etc.
  ToolArgs snapshot() => formValuesToToolArgs(_values);

  /// Typed read for widgets that need the raw [FormValue] (e.g. a
  /// boolean field reading its current state, or a multi-option field
  /// driving its FilterChip selection set).
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

  /// Seed the controller from a deep-link `initialInput` JSON object.
  ///
  /// Values are decoded only for declared fields and only when their JSON
  /// shape matches the field schema. Invalid and unknown entries are ignored.
  void seed(ToolArgs initial, {required Iterable<InputFieldDto> fields}) {
    final input = initial.toJsonObject();
    _values.clear();
    for (final field in fields) {
      final value = formValueFromJson(field.fieldType, input[field.key]);
      if (value != null) _values[field.key] = value;
    }
  }

  /// Test-only escape hatch. Production code should use [set] / [value].
  @visibleForTesting
  Map<String, FormValue> debugRaw() => _values;
}

class GenericFormWidget extends ConsumerStatefulWidget {
  const GenericFormWidget({
    required this.tool,
    required this.controller,
    this.onValidationChanged,
    this.compact = false,
    this.boundInputKeys = const <String>{},
    super.key,
  });

  final ToolDto tool;
  final GenericFormController controller;

  /// Dense layout for the inline pin body: underline (not outlined) fields,
  /// label as a placeholder, smaller type and tighter spacing so the form
  /// fits a tile. The expanded modal leaves this false for the full layout.
  final bool compact;
  final Set<String> boundInputKeys;

  /// Optional gate hook: fires whenever the aggregate validity flips.
  /// `true` ⇒ every required field is filled and every field validator
  /// passes. The expanded modal page wires this to the Run button.
  final OnValidationChanged? onValidationChanged;

  @override
  ConsumerState<GenericFormWidget> createState() => _GenericFormWidgetState();
}

class _GenericFormWidgetState extends ConsumerState<GenericFormWidget> {
  /// Per-field validation outcome — read by the [TextFormField]'s
  /// `errorText` and by the aggregate `allOk` gate.
  final Map<String, FieldValidation> _validations = <String, FieldValidation>{};

  final Map<String, TextEditingController> _textControllers =
      <String, TextEditingController>{};

  bool _allOk = false;

  /// Required-field errors are noisy on a newly opened form, but the Run
  /// gate must still start closed. This flag belongs to the widget instance
  /// so a user edit in one tool never exposes errors in another.
  bool _showRequiredErrors = false;

  @override
  void initState() {
    super.initState();
    // Compute initial validity against the starting values: a declared
    // default, a deep-link `initialInput` pre-fill, or nothing. A
    // required field with neither leaves the Run button disabled.
    for (final field in widget.tool.inputFields) {
      // Declared defaults (`Integer(default=20)`, `String(default=…)`,
      // and a boolean's implicit false) seed the controller before the
      // first validation pass, so the Run gate sees the same values the
      // user sees.
      final seed = defaultFormValueFor(field);
      if (seed != null) widget.controller.setDefault(field.key, seed);
      _validations[field.key] = _validateField(field);
    }
    _normalizeImageOptions();
    _allOk = _computeAllOk();
    // Defer the callback to after the first frame; running it inside
    // `initState` would call the host's setState before its own frame
    // finishes building.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      widget.onValidationChanged?.call(_allOk);
    });
  }

  @override
  void dispose() {
    for (final controller in _textControllers.values) {
      controller.dispose();
    }
    super.dispose();
  }

  bool _fieldVisible(InputFieldDto field) => imageConversionFieldVisible(
    widget.tool.id,
    field.key,
    widget.controller.value,
  );

  void _normalizeImageOptions() {
    if (!isImageConversionTool(widget.tool.id)) return;
    widget.controller.setDefault(
      ImageConversionFields.format,
      const OptionValue('png'),
    );
    for (final field in widget.tool.inputFields) {
      if (!_fieldVisible(field)) {
        widget.controller._values.remove(field.key);
        _textControllers[field.key]?.clear();
      } else {
        final seed = defaultFormValueFor(field);
        if (seed != null && widget.controller.value(field.key) == null) {
          widget.controller.setDefault(field.key, seed);
          _textControllers[field.key]?.text = _validationText(seed);
        }
      }
      _validations[field.key] = _validateField(field);
    }
  }

  bool _computeAllOk() => widget.tool.inputFields
      .where(_fieldVisible)
      .every((field) => _validations[field.key]?.isOk ?? false);

  FieldValidation _validateField(InputFieldDto field) {
    final value = widget.controller.value(field.key);
    final number = field.constraints?.number;
    return switch (field.fieldType) {
      InputFieldType_Number() => validateNumber(
        _validationText(value),
        required: field.required_,
        min: number?.min,
        max: number?.max,
      ),
      InputFieldType_Integer() => validateInteger(
        _validationText(value),
        required: field.required_,
        min: number?.min,
        max: number?.max,
      ),
      InputFieldType_Url() => validateUrl(
        _validationText(value),
        required: field.required_,
      ),
      InputFieldType_Text() ||
      InputFieldType_Multiline() ||
      InputFieldType_Markdown() ||
      InputFieldType_FilePath() => validateText(
        _validationText(value),
        required: field.required_,
        pattern: field.constraints?.string?.regex,
      ),
      InputFieldType_File() => validateFileValue(
        value,
        required: field.required_,
      ),
      InputFieldType_Boolean() => validateBooleanValue(
        value,
        required: field.required_,
      ),
      InputFieldType_Select(:final options) => validateOptionValue(
        value,
        required: field.required_,
        options: choiceValues(options),
      ),
      InputFieldType_MultiOptions(:final options) => validateMultiOptionValue(
        value,
        required: field.required_,
        options: choiceValues(options),
      ),
      InputFieldType_DateTime() => validateDateTimeValue(
        value,
        required: field.required_,
      ),
    };
  }

  String _validationText(FormValue? value) {
    return switch (value) {
      TextValue(:final value) => value,
      NumberValue(:final value) => '$value',
      _ => '',
    };
  }

  /// Revalidate a typed field value. Setting state pushes any `errorText`
  /// into the field decoration, and the host learns about an aggregate
  /// validity flip through `onValidationChanged`.
  void _onValueChanged(InputFieldDto field) {
    final next = _validateField(field);
    final prevAllOk = _allOk;
    setState(() {
      _showRequiredErrors = true;
      _validations[field.key] = next;
      _normalizeImageOptions();
      _allOk = _computeAllOk();
    });
    if (_allOk != prevAllOk) {
      widget.onValidationChanged?.call(_allOk);
    }
  }

  String? _errorTextFor(InputFieldDto field) {
    final v = _validations[field.key];
    if (v is FieldValidationError) {
      if (!_showRequiredErrors && v.messageKey == 'modal.validation.required') {
        return null;
      }
      return t(ref, v.messageKey, v.messageArgs);
    }
    return null;
  }

  bool get _compact => widget.compact;

  /// Standard field decoration, compact-aware. In compact mode the label
  /// becomes a placeholder (no vertical floating-label space), the border is
  /// a thin underline, and padding/type shrink to fit a tile.
  InputDecoration _decoration(
    InputFieldDto field, {
    String? decoratedLabel,
    Widget? suffixIcon,
  }) {
    final label = decoratedLabel ?? _decoratedLabel(field);
    if (!_compact) {
      return InputDecoration(
        labelText: label,
        // The declared placeholder is a sample value, so it belongs in
        // the hint slot; the description is guidance, so it belongs
        // under the field.
        hintText: field.constraints?.string?.placeholder,
        helperText: _helperTextFor(field),
        helperMaxLines: _helperMaxLines,
        border: const OutlineInputBorder(),
        errorText: _errorTextFor(field),
        suffixIcon: suffixIcon,
      );
    }
    final textTheme = Theme.of(context).textTheme;
    return InputDecoration(
      isDense: true,
      hintText: label,
      hintStyle: textTheme.labelMedium,
      border: const UnderlineInputBorder(),
      contentPadding: const EdgeInsets.symmetric(vertical: UpegSizing.radius1),
      errorText: _errorTextFor(field),
      errorStyle: textTheme.labelSmall?.copyWith(color: context.upeg.warn),
      suffixIcon: suffixIcon,
    );
  }

  /// Compact field text style (null keeps the modal's default sizing).
  TextStyle? get _fieldStyle =>
      _compact ? Theme.of(context).textTheme.bodyMedium : null;

  @override
  Widget build(BuildContext context) {
    final fields = widget.tool.inputFields;
    if (fields.isEmpty) {
      return Padding(
        key: const Key('generic-form-empty'),
        padding: const EdgeInsets.symmetric(vertical: 12),
        child: Text(t(ref, 'modal.generic.no_inputs')),
      );
    }
    final visible = fields
        .where(
          (field) =>
              _fieldVisible(field) &&
              (!isImageConversionTool(widget.tool.id) ||
                  field.key != ImageConversionFields.outputLimit),
        )
        .toList();
    final bound = visible
        .where(
          (field) =>
              widget.boundInputKeys.contains(field.key) &&
              field.fieldType is InputFieldType_Text &&
              _validationText(widget.controller.value(field.key)).isNotEmpty,
        )
        .toList();
    return FocusTraversalGroup(
      policy: WidgetOrderTraversalPolicy(),
      child: Focus(
        canRequestFocus: false,
        onKeyEvent: _onFormKeyEvent,
        child: Column(
          key: const Key('generic-form'),
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            if (isImageConversionTool(widget.tool.id) && !_compact)
              Padding(
                padding: const EdgeInsets.only(bottom: _expandedFieldSpacing),
                child: Text(t(ref, 'media.convert.guidance')),
              ),
            for (final field in visible.where(
              (field) => !bound.contains(field),
            ))
              Padding(
                padding: EdgeInsets.only(
                  bottom: _compact ? UpegSizing.radius2 : _expandedFieldSpacing,
                ),
                child: _buildField(field),
              ),
            if (bound.isNotEmpty)
              Material(
                type: MaterialType.transparency,
                child: ExpansionTile(
                  key: const Key('generic-form-bound-inputs'),
                  title: Text(t(ref, 'modal.generic.bound_inputs')),
                  initiallyExpanded: bound.any(
                    (field) => !_validateField(field).isOk,
                  ),
                  children: [
                    for (final field in bound)
                      Padding(
                        padding: const EdgeInsets.only(
                          bottom: _expandedFieldSpacing,
                        ),
                        child: _buildField(field),
                      ),
                  ],
                ),
              ),
            if (isImageConversionTool(widget.tool.id)) ...[
              if (widget.controller.value(ImageConversionFields.format) ==
                  const OptionValue('ico'))
                Padding(
                  padding: const EdgeInsets.only(bottom: _expandedFieldSpacing),
                  child: Text(t(ref, 'media.convert.ico')),
                ),
              for (final field in fields.where(
                (field) => field.key == ImageConversionFields.outputLimit,
              ))
                ExpansionTile(
                  title: Text(t(ref, 'media.convert.advanced')),
                  initiallyExpanded: !_validateField(field).isOk,
                  children: [
                    Padding(
                      padding: const EdgeInsets.only(
                        bottom: _expandedFieldSpacing,
                      ),
                      child: _buildField(field),
                    ),
                  ],
                ),
            ],
          ],
        ),
      ),
    );
  }

  KeyEventResult _onFormKeyEvent(FocusNode node, KeyEvent event) {
    final cmd = resolveKeyboardCommand(
      ref,
      event,
      scope: KeyboardScopeDto.form,
    );
    return switch (cmd) {
      KeyboardCommandDto_FocusNext() => _focusNext(),
      KeyboardCommandDto_FocusPrevious() => _focusPrevious(),
      _ => KeyEventResult.ignored,
    };
  }

  KeyEventResult _focusNext() {
    FocusScope.of(context).nextFocus();
    return KeyEventResult.handled;
  }

  KeyEventResult _focusPrevious() {
    FocusScope.of(context).previousFocus();
    return KeyEventResult.handled;
  }

  Widget _buildField(InputFieldDto field) {
    return switch (field.fieldType) {
      InputFieldType_Text() => _textField(field, multiline: false),
      InputFieldType_Multiline() => _textField(field, multiline: true),
      InputFieldType_Number() => _numberField(field),
      InputFieldType_Integer() => _integerField(field),
      InputFieldType_Boolean() => _booleanField(field),
      InputFieldType_File(:final policy) => _fileField(field, policy),
      InputFieldType_Select(:final options) => _selectField(field, options),
      InputFieldType_MultiOptions(:final options) => _multiOptionsField(
        field,
        options,
      ),
      InputFieldType_DateTime() => _dateTimeField(field),
      InputFieldType_Markdown() => _markdownField(field),
      InputFieldType_FilePath() => _filePathField(field),
      InputFieldType_Url() => _urlField(field),
    };
  }

  String _seedTextFor(InputFieldDto field) {
    return switch (widget.controller.value(field.key)) {
      TextValue(:final value) => value,
      NumberValue(:final value) => '$value',
      _ => '',
    };
  }

  TextEditingController _textControllerFor(String key, String text) {
    final controller = _textControllers.putIfAbsent(
      key,
      () => TextEditingController(text: text),
    );
    if (controller.text != text) {
      controller.value = TextEditingValue(
        text: text,
        selection: TextSelection.collapsed(offset: text.length),
      );
    }
    return controller;
  }

  Widget _textField(InputFieldDto field, {required bool multiline}) {
    return TextFormField(
      key: Key('field-${field.key}'),
      initialValue: _seedTextFor(field),
      decoration: _decoration(field),
      style: _fieldStyle,
      maxLines: multiline ? null : 1,
      minLines: multiline
          ? (_compact ? _compactMultilineMinLines : _expandedMultilineMinLines)
          : 1,
      onChanged: (value) => _onTextChanged(field, value),
    );
  }

  /// Store or clear a text-shaped field in response to a user edit.
  ///
  /// Mirrors `_numericField`: an optional field's empty text removes the
  /// key instead of storing an empty [TextValue], so the dispatched args
  /// omit it rather than sending `""` (which the Rust side would then
  /// reject on a pattern/type mismatch). A required field's empty text is
  /// still stored so [_requiredCheck] can flag it as missing.
  void _onTextChanged(InputFieldDto field, String value) {
    if (!field.required_ && value.isEmpty) {
      widget.controller.remove(field.key);
      _onValueChanged(field);
      return;
    }
    widget.controller.set(field.key, TextValue(value));
    _onValueChanged(field);
  }

  Widget _numberField(InputFieldDto field) => _numericField(
    field,
    decimal: true,
    // Parse to a typed [num]; the dispatcher then sees a JSON number
    // (not a string). Malformed and non-finite input lands as a
    // [TextValue] so JSON encoding can never receive NaN or infinity.
    parse: (raw) => switch (num.tryParse(raw)) {
      final parsed? when parsed.isFinite => NumberValue(parsed),
      _ => null,
    },
  );

  /// `Integer` fields are not `Number` fields with a nicer label: the
  /// wire type is a JSON integer, so `1.5` is rejected here rather than
  /// dispatched and bounced back by Rust.
  Widget _integerField(InputFieldDto field) => _numericField(
    field,
    decimal: false,
    parse: (raw) => switch (int.tryParse(raw.trim())) {
      final parsed? => NumberValue(parsed),
      _ => null,
    },
  );

  Widget _numericField(
    InputFieldDto field, {
    required bool decimal,
    required FormValue? Function(String raw) parse,
  }) {
    return TextFormField(
      key: Key('field-${field.key}'),
      initialValue: _seedTextFor(field),
      decoration: _decoration(field),
      style: _fieldStyle,
      keyboardType: TextInputType.numberWithOptions(
        decimal: decimal,
        signed: true,
      ),
      onChanged: (value) {
        if (!field.required_ && value.trim().isEmpty) {
          widget.controller.remove(field.key);
          _onValueChanged(field);
          return;
        }
        // Unparseable input is kept verbatim so the validator can name
        // the problem instead of the field silently swallowing it.
        widget.controller.set(field.key, parse(value) ?? TextValue(value));
        _onValueChanged(field);
      },
    );
  }

  Widget _booleanField(InputFieldDto field) {
    final current = switch (widget.controller.value(field.key)) {
      BooleanValue(:final value) => value,
      _ => false,
    };
    void onChanged(bool value) {
      widget.controller.set(field.key, BooleanValue(value));
      _onValueChanged(field);
    }

    if (!_compact) {
      return SwitchListTile(
        key: Key('field-${field.key}'),
        title: Text(_decoratedLabel(field)),
        // A switch has no InputDecoration, so the declared description
        // rides the list tile's own subtitle slot.
        subtitle: switch (_nonEmpty(field.description)) {
          final description? => Text(
            description,
            style: Theme.of(
              context,
            ).textTheme.labelSmall?.copyWith(color: context.upeg.fg3),
          ),
          null => null,
        },
        value: current,
        onChanged: onChanged,
      );
    }
    return Semantics(
      key: Key('field-${field.key}'),
      container: true,
      excludeSemantics: true,
      label: _decoratedLabel(field),
      toggled: current,
      enabled: true,
      focusable: true,
      onTap: () => onChanged(!current),
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        excludeFromSemantics: true,
        onTap: () => onChanged(!current),
        child: Row(
          children: [
            Expanded(
              child: Text(
                _decoratedLabel(field),
                style: Theme.of(context).textTheme.labelMedium,
              ),
            ),
            Switch(value: current, onChanged: onChanged),
          ],
        ),
      ),
    );
  }

  Widget _fileField(InputFieldDto field, FileInputPolicyDto policy) {
    final current = switch (widget.controller.value(field.key)) {
      final FileFormValue value => value,
      _ => null,
    };
    return FileInputField(
      key: Key('field-${field.key}'),
      label: _decoratedLabel(field),
      description: field.description,
      value: current,
      policy: fileSelectionPolicyFromDto(policy),
      pickerBridge: ref.read(filePickerBridgeProvider),
      compact: _compact,
      onChanged: (value) {
        widget.controller.set(field.key, value);
        _onValueChanged(field);
      },
      onCleared: () {
        widget.controller.remove(field.key);
        _onValueChanged(field);
      },
    );
  }

  Widget _selectField(InputFieldDto field, List<ChoiceOptionDto> options) {
    final current = switch (widget.controller.value(field.key)) {
      OptionValue(:final key) => key,
      _ => null,
    };
    // A two-line menu entry needs content-sized rows; the fixed default
    // row height would clip the description.
    final describes =
        !_compact && options.any((opt) => opt.description != null);
    return DropdownButtonFormField<String>(
      key: Key('field-${field.key}'),
      decoration: _decoration(field),
      initialValue: current,
      isExpanded: _compact,
      itemHeight: describes ? null : kMinInteractiveDimension,
      // The closed button shows the label only — the description belongs
      // to the choosing moment, not to the chosen value.
      selectedItemBuilder: (context) => [
        for (final opt in options)
          Align(
            alignment: AlignmentDirectional.centerStart,
            child: Text(opt.label, style: _fieldStyle),
          ),
      ],
      items: [
        for (final opt in options)
          DropdownMenuItem<String>(
            value: opt.value,
            child: _choiceEntry(opt, describe: describes),
          ),
      ],
      onChanged: (value) {
        if (value == null) return;
        widget.controller.set(field.key, OptionValue(value));
        _onValueChanged(field);
      },
    );
  }

  Widget _choiceEntry(ChoiceOptionDto option, {required bool describe}) {
    final description = describe ? option.description : null;
    if (description == null || description.isEmpty) {
      return Text(option.label, style: _fieldStyle);
    }
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(option.label, style: _fieldStyle),
        Text(
          description,
          style: Theme.of(
            context,
          ).textTheme.labelSmall?.copyWith(color: context.upeg.fg3),
        ),
      ],
    );
  }

  Widget _markdownField(InputFieldDto field) {
    // Markdown render is a separate row (tool description) — the form
    // field itself is just a multiline text input with a monospace
    // family to telegraph "this expects markup".
    return TextFormField(
      key: Key('field-${field.key}'),
      initialValue: _seedTextFor(field),
      decoration: _decoration(field),
      maxLines: null,
      minLines: _compact ? 2 : 4,
      style: Theme.of(context).textTheme.bodyMedium?.copyWith(
        fontFamily: upegMonoFontFamily,
        fontFamilyFallback: upegMonoFontFamilyFallback,
      ),
      onChanged: (value) => _onTextChanged(field, value),
    );
  }

  Widget _filePathField(InputFieldDto field) {
    // Read the current path off the controller for the displayed text.
    final currentPath = switch (widget.controller.value(field.key)) {
      TextValue(:final value) => value,
      _ => '',
    };
    return TextFormField(
      key: Key('field-${field.key}'),
      controller: _textControllerFor(field.key, currentPath),
      style: _fieldStyle,
      decoration: _decoration(
        field,
        suffixIcon: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            IconButton(
              key: Key('field-${field.key}-pick-file'),
              tooltip: t(ref, 'modal.generic.file_path_pick_file'),
              icon: const Icon(Icons.insert_drive_file_outlined),
              onPressed: () => _pickPath(field, directory: false),
            ),
            IconButton(
              key: Key('field-${field.key}-pick-directory'),
              tooltip: t(ref, 'modal.generic.file_path_pick_folder'),
              icon: const Icon(Icons.folder_open),
              onPressed: () => _pickPath(field, directory: true),
            ),
          ],
        ),
      ),
      onChanged: (value) => _onTextChanged(field, value),
    );
  }

  Future<void> _pickPath(InputFieldDto field, {required bool directory}) async {
    final bridge = ref.read(filePickerBridgeProvider);
    final picked = directory
        ? await bridge.pickDirectoryPath()
        : await bridge.pickOpenPath();
    if (picked == null || !mounted) return;
    widget.controller.set(field.key, TextValue(picked));
    _onValueChanged(field);
  }

  Widget _urlField(InputFieldDto field) {
    return TextFormField(
      key: Key('field-${field.key}'),
      initialValue: _seedTextFor(field),
      decoration: _decoration(field),
      style: _fieldStyle,
      keyboardType: TextInputType.url,
      onChanged: (value) => _onTextChanged(field, value),
    );
  }

  Widget _dateTimeField(InputFieldDto field) {
    // Read the current selection through the typed value union — the
    // text shows ISO-8601 so the picker round-trips losslessly.
    final selected = switch (widget.controller.value(field.key)) {
      DateTimeValue(:final value) => value,
      _ => null,
    };
    final display = selected == null ? '' : selected.toUtc().toIso8601String();
    return Focus(
      onKeyEvent: (_, event) {
        if (isKeyboardActivationEvent(event)) {
          unawaited(_pickDateTime(field, selected));
          return KeyEventResult.handled;
        }
        return KeyEventResult.ignored;
      },
      child: TextFormField(
        key: Key('field-${field.key}'),
        readOnly: true,
        controller: _textControllerFor(field.key, display),
        style: _fieldStyle,
        decoration: _decoration(
          field,
          suffixIcon: IconButton(
            icon: const Icon(Icons.calendar_today),
            onPressed: () => unawaited(_pickDateTime(field, selected)),
          ),
        ),
        onTap: () => unawaited(_pickDateTime(field, selected)),
      ),
    );
  }

  Future<void> _pickDateTime(InputFieldDto field, DateTime? selected) async {
    final now = DateTime.now();
    final picked = await showDatePicker(
      context: context,
      initialDate: selected ?? now,
      firstDate: DateTime(now.year - _datePickerYearRange),
      lastDate: DateTime(now.year + _datePickerYearRange),
    );
    if (picked == null) return;
    if (!mounted) return;
    widget.controller.set(field.key, DateTimeValue(picked));
    _onValueChanged(field);
  }

  Widget _multiOptionsField(
    InputFieldDto field,
    List<ChoiceOptionDto> options,
  ) {
    // Read the current selection set off the controller. An absent /
    // wrong-typed entry yields an empty set so the first tap kicks the
    // value into a [MultiOptionValue].
    final selected = switch (widget.controller.value(field.key)) {
      MultiOptionValue(:final keys) => keys.toSet(),
      _ => <String>{},
    };
    final errorText = _errorTextFor(field);
    final colorScheme = Theme.of(context).colorScheme;
    final compactLabelStyle = Theme.of(context).textTheme.labelSmall;
    return Padding(
      key: Key('field-${field.key}'),
      padding: EdgeInsets.zero,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding: const EdgeInsets.only(bottom: UpegSizing.radius2),
            child: Text(
              _decoratedLabel(field),
              style: _compact ? Theme.of(context).textTheme.labelMedium : null,
            ),
          ),
          // Chips carry no InputDecoration either; the description gets
          // its own line above them (full layout only — a tile has no
          // room for it).
          if (_compact ? null : _nonEmpty(field.description)
              case final description?)
            Padding(
              padding: const EdgeInsets.only(bottom: UpegSizing.radius2),
              child: Text(
                description,
                style: Theme.of(
                  context,
                ).textTheme.labelSmall?.copyWith(color: context.upeg.fg3),
              ),
            ),
          Wrap(
            spacing: UpegSizing.radius2,
            runSpacing: UpegSizing.radius1,
            children: [
              for (final opt in options)
                FilterChip(
                  key: Key('field-${field.key}-chip-${opt.value}'),
                  // A chip has no room for a second line, so the
                  // declared description rides the tooltip instead of
                  // being dropped.
                  tooltip: _compact ? null : opt.description,
                  label: Text(
                    opt.label,
                    style: _compact
                        ? compactLabelStyle?.copyWith(
                            color: selected.contains(opt.value)
                                ? colorScheme.onSecondary
                                : context.upeg.fg3,
                          )
                        : null,
                  ),
                  selected: selected.contains(opt.value),
                  selectedColor: _compact ? colorScheme.secondary : null,
                  visualDensity: _compact ? VisualDensity.compact : null,
                  onSelected: (next) {
                    final updated = selected.toSet();
                    if (next) {
                      updated.add(opt.value);
                    } else {
                      updated.remove(opt.value);
                    }
                    widget.controller.set(
                      field.key,
                      MultiOptionValue(updated.toList()..sort()),
                    );
                    _onValueChanged(field);
                  },
                ),
            ],
          ),
          if (errorText != null)
            Padding(
              padding: const EdgeInsets.only(top: UpegSizing.radius1),
              child: Semantics(
                container: true,
                liveRegion: true,
                label: errorText,
                child: ExcludeSemantics(
                  child: Text(
                    errorText,
                    style: Theme.of(
                      context,
                    ).textTheme.labelSmall?.copyWith(color: context.upeg.warn),
                  ),
                ),
              ),
            ),
        ],
      ),
    );
  }

  String _decoratedLabel(InputFieldDto field) {
    final key = isImageConversionTool(widget.tool.id)
        ? imageConversionLabelKey(field.key)
        : null;
    final label = key == null ? field.label : t(ref, key);
    return field.required_ ? '$label *' : label;
  }

  /// Under-field guidance: the declared description, plus the numeric
  /// range when one is declared. `null` keeps the old zero-height
  /// layout for fields that declare neither.
  String? _helperTextFor(InputFieldDto field) {
    final parts = <String>[
      ?_nonEmpty(field.description),
      ?_rangeHint(field.constraints?.number),
    ];
    return parts.isEmpty ? null : parts.join(_hintSeparator);
  }

  static String? _nonEmpty(String? value) =>
      value == null || value.isEmpty ? null : value;

  static String? _rangeHint(NumberConstraintsDto? constraints) {
    final parts = <String>[
      if (constraints?.min case final double min)
        '$_minimumHintPrefix${formatBound(min)}',
      if (constraints?.max case final double max)
        '$_maximumHintPrefix${formatBound(max)}',
    ];
    return parts.isEmpty ? null : parts.join(_hintSeparator);
  }
}
