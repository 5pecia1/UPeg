/// Pin color editor dialog.
///
/// Mirrors the TUI pin color editor contract: palette swatch selection,
/// custom HEX input with validation, Save/Reset/Cancel actions.
library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/theme/upeg_theme.dart';

const String _pinColorDialogKey = 'pin-color-dialog';
const String _pinColorSaveKey = 'pin-color-save';
const String _pinColorResetKey = 'pin-color-reset';
const String _pinColorCancelKey = 'pin-color-cancel';
const _hexPrefix = '#';
const _hexDigits = 6;
const _hexLen = 1 + _hexDigits;

/// Predefined colour swatches for quick selection.
/// Each entry is `(uppercase hex without '#', display Color)`.
const _swatches = <({String hexRgb, Color color})>[
  (hexRgb: 'FF5733', color: Color(0xFFFF5733)),
  (hexRgb: 'FFC107', color: Color(0xFFFFC107)),
  (hexRgb: '4CAF50', color: Color(0xFF4CAF50)),
  (hexRgb: '2196F3', color: Color(0xFF2196F3)),
  (hexRgb: '9C27B0', color: Color(0xFF9C27B0)),
  (hexRgb: 'E91E63', color: Color(0xFFE91E63)),
  (hexRgb: '00BCD4', color: Color(0xFF00BCD4)),
  (hexRgb: 'FF9800', color: Color(0xFFFF9800)),
  (hexRgb: '607D8B', color: Color(0xFF607D8B)),
  (hexRgb: 'FFFFFF', color: Color(0xFFFFFFFF)),
  (hexRgb: '000000', color: Color(0xFF000000)),
];

class PinColorDialog extends ConsumerStatefulWidget {
  const PinColorDialog({
    required this.initialColor,
    required this.onSave,
    required this.onReset,
    required this.onCancel,
    super.key,
  });

  final String? initialColor;
  final void Function(String? colorHex) onSave;
  final void Function(String? previousColor) onReset;
  final VoidCallback onCancel;

  @override
  ConsumerState<PinColorDialog> createState() => _PinColorDialogState();
}

class _PinColorDialogState extends ConsumerState<PinColorDialog> {
  late final TextEditingController _hexController;
  late final FocusNode _focusNode;
  late String _displayHex;

  @override
  void initState() {
    super.initState();
    _displayHex = widget.initialColor?.toUpperCase() ?? '';
    _hexController = TextEditingController(text: _displayHex);
    _focusNode = FocusNode();
  }

  @override
  void dispose() {
    _focusNode.dispose();
    _hexController.dispose();
    super.dispose();
  }

  bool get _isValid => _validateHex(_hexController.text);

  void _onSwatchTap(String hexRgb) {
    final hex = '$_hexPrefix$hexRgb';
    _hexController.text = hex;
    setState(() {
      _displayHex = hex;
    });
  }

  void _onTextChanged(String value) {
    final normalized = _normalizeHex(value);
    setState(() {
      _displayHex = normalized;
    });
  }

  void _onSave() {
    if (_isValid) {
      widget.onSave(_normalizeHex(_hexController.text));
    }
  }

  void _onReset() {
    widget.onReset(widget.initialColor);
  }

  void _onCancel() {
    widget.onCancel();
  }

  @override
  Widget build(BuildContext context) {
    final tokens = Theme.of(context).extension<UpegTokens>()!;
    return KeyboardListener(
      focusNode: _focusNode,
      autofocus: true,
      onKeyEvent: (event) {
        if (event is KeyDownEvent &&
            event.logicalKey == LogicalKeyboardKey.escape) {
          _onCancel();
        }
      },
      child: Container(
        key: const Key(_pinColorDialogKey),
        padding: const EdgeInsets.all(16),
        decoration: BoxDecoration(
          color: tokens.surface,
          borderRadius: BorderRadius.circular(12),
          border: Border.all(color: tokens.line),
        ),
        constraints: const BoxConstraints(minWidth: 300, maxWidth: 360),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(
              t(ref, 'pin_color.title'),
              style: TextStyle(
                fontFamily: upegMonoFontFamily,
                fontFamilyFallback: upegMonoFontFamilyFallback,
                fontSize: 14,
                fontWeight: FontWeight.w500,
                color: tokens.fg,
              ),
            ),
            const SizedBox(height: 16),
            _buildSwatches(tokens),
            const SizedBox(height: 16),
            _buildHexInput(tokens),
            const SizedBox(height: 16),
            _buildActions(tokens),
          ],
        ),
      ),
    );
  }

  Widget _buildSwatches(UpegTokens tokens) {
    return Wrap(
      spacing: 8,
      runSpacing: 8,
      children: [
        for (final swatch in _swatches)
          GestureDetector(
            onTap: () => _onSwatchTap(swatch.hexRgb),
            child: Container(
              key: Key('color-swatch-${swatch.hexRgb.toLowerCase()}'),
              width: 32,
              height: 32,
              decoration: BoxDecoration(
                color: swatch.color,
                shape: BoxShape.circle,
                border: Border.all(color: tokens.line, width: 1),
              ),
            ),
          ),
      ],
    );
  }

  Widget _buildHexInput(UpegTokens tokens) {
    final bool isError = _hexController.text.isNotEmpty && !_isValid;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        TextField(
          controller: _hexController,
          onChanged: _onTextChanged,
          decoration: InputDecoration(
            hintText: '#RRGGBB',
            hintStyle: TextStyle(color: tokens.fg4, fontSize: 14),
            isDense: true,
            contentPadding: const EdgeInsets.symmetric(
              horizontal: 12,
              vertical: 8,
            ),
            border: OutlineInputBorder(
              borderRadius: BorderRadius.circular(8),
              borderSide: BorderSide(color: tokens.line),
            ),
            enabledBorder: OutlineInputBorder(
              borderRadius: BorderRadius.circular(8),
              borderSide: BorderSide(color: tokens.line),
            ),
            focusedBorder: OutlineInputBorder(
              borderRadius: BorderRadius.circular(8),
              borderSide: BorderSide(color: tokens.accent),
            ),
            errorBorder: OutlineInputBorder(
              borderRadius: BorderRadius.circular(8),
              borderSide: BorderSide(color: tokens.warn),
            ),
            errorText: isError ? t(ref, 'pin_color.invalid_hex') : null,
          ),
          style: TextStyle(
            fontFamily: upegMonoFontFamily,
            fontFamilyFallback: upegMonoFontFamilyFallback,
            fontSize: 14,
            color: tokens.fg,
          ),
          inputFormatters: [
            FilteringTextInputFormatter.allow(RegExp(r'^#?[0-9a-fA-F]{0,6}')),
          ],
          maxLength: _hexLen,
          buildCounter:
              (_, {required currentLength, required isFocused, maxLength}) =>
                  null,
        ),
      ],
    );
  }

  Widget _buildActions(UpegTokens tokens) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.end,
      children: [
        TextButton(
          key: const Key(_pinColorCancelKey),
          onPressed: _onCancel,
          style: TextButton.styleFrom(foregroundColor: tokens.fg3),
          child: Text(t(ref, 'desktop.tab.remove_cancel')),
        ),
        const SizedBox(width: 8),
        TextButton(
          key: const Key(_pinColorResetKey),
          onPressed: _onReset,
          style: TextButton.styleFrom(foregroundColor: tokens.fg3),
          child: Text(t(ref, 'pin_color.reset')),
        ),
        const SizedBox(width: 8),
        ElevatedButton(
          key: const Key(_pinColorSaveKey),
          onPressed: _isValid ? _onSave : null,
          style: ElevatedButton.styleFrom(
            backgroundColor: tokens.accent,
            foregroundColor: tokens.bg,
          ),
          child: Text(t(ref, 'pin_color.save')),
        ),
      ],
    );
  }
}

bool _validateHex(String value) {
  if (value.isEmpty) return false;
  final normalized = _normalizeHex(value);
  if (normalized.length != _hexLen) return false;
  if (!normalized.startsWith(_hexPrefix)) return false;
  final hexPart = normalized.substring(1);
  return hexPart.length == _hexDigits &&
      RegExp(r'^[0-9A-F]{6}$').hasMatch(hexPart);
}

String _normalizeHex(String value) {
  if (value.isEmpty) return '';
  var hex = value.trim();
  if (!hex.startsWith(_hexPrefix)) {
    hex = '$_hexPrefix$hex';
  }
  return hex.substring(0, 1) + hex.substring(1).toUpperCase();
}
