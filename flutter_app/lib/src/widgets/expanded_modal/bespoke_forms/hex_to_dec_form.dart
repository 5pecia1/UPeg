/// Bespoke form widget for the `num.hex_to_decimal` tool (Batch K3 / I03).
///
/// The value field is a single TextField that decodes the hex input
/// live on every keystroke, the decimal preview sits underneath, and
/// F1/F2 shortcuts re-run / copy the result. F1 also dispatches the
/// real tool via the existing `onSubmit` seam so the modal can render
/// the canonical output block + embed.
///
/// Type-system notes:
/// - The local `_result` field is a [HexDecodeResult] sealed variant —
///   the UI dispatches on it via a switch (no stringly-typed flags).
/// - `onSubmit` hands a [ToolArgs] envelope back to the modal host. The
///   JSON dictionary still contains only `{'value': '<raw input>'}`, but
///   `Map<String, dynamic>` no longer leaks through the widget API.
library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';
import 'package:upeg/src/widgets/expanded_modal/bespoke_forms/hex_decode_result.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

/// Argument key submitted to `dispatchTool`. Must match the tool's input
/// field declared on the Rust side (`#[tool(inputs = [required input: …])]`,
/// read via `read_str(args, "input")`). Submitting `value` here dispatched
/// num.hex_to_decimal on an empty input because the runtime never read that key.
const String hexToDecInputArgKey = 'input';

class HexToDecForm extends ConsumerStatefulWidget {
  const HexToDecForm({
    required this.tool,
    required this.onSubmit,
    this.initialInput,
    this.clipboardWriter = const FlutterClipboardWriter(),
    super.key,
  });

  final ToolDto tool;
  final void Function(ToolArgs args) onSubmit;

  /// Deep-link / pin pre-fill. Carries a single value (the raw hex string
  /// under the tool's input-field key); the form seeds its input with it so
  /// a `upeg://open?...&input=0x2a` link opens with the value already typed.
  final ToolArgs? initialInput;
  final ClipboardWriter clipboardWriter;

  @override
  ConsumerState<HexToDecForm> createState() => _HexToDecFormState();
}

class _RunIntent extends Intent {
  const _RunIntent();
}

class _CopyIntent extends Intent {
  const _CopyIntent();
}

class _HexToDecFormState extends ConsumerState<HexToDecForm> {
  final TextEditingController _input = TextEditingController();
  HexDecodeResult _result = const HexDecodeResult.empty();

  @override
  void initState() {
    super.initState();
    // Seed the pre-fill before wiring the listener so we set `_result`
    // directly here rather than through `_onInputChanged`'s `setState`,
    // which is illegal during initState.
    final String? seed = _seedFromInitialInput();
    if (seed != null) {
      _input.text = seed;
      _result = decodeHex(seed);
    }
    _input.addListener(_onInputChanged);
  }

  /// The single pre-fill value, independent of its arg-key. `initialInput`
  /// carries exactly one entry (the raw input under the tool's field key),
  /// so reading the first non-empty string value avoids hard-coding the key.
  String? _seedFromInitialInput() {
    final values = widget.initialInput?.toJsonObject().values;
    if (values == null || values.isEmpty) return null;
    final Object? first = values.first;
    return first is String && first.isNotEmpty ? first : null;
  }

  @override
  void dispose() {
    _input.removeListener(_onInputChanged);
    _input.dispose();
    super.dispose();
  }

  void _onInputChanged() {
    setState(() {
      _result = decodeHex(_input.text);
    });
  }

  void _handleRun() {
    if (_result is! HexDecodeOk) return;
    widget.onSubmit(
      ToolArgs.fromJsonObject(<String, Object?>{
        hexToDecInputArgKey: _input.text.trim(),
      }),
    );
  }

  void _handleCopy() {
    final r = _result;
    if (r is! HexDecodeOk) return;
    widget.clipboardWriter.write(r.decimal.toString());
  }

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    return Shortcuts(
      shortcuts: const <ShortcutActivator, Intent>{
        SingleActivator(LogicalKeyboardKey.f1): _RunIntent(),
        SingleActivator(LogicalKeyboardKey.f2): _CopyIntent(),
      },
      child: Actions(
        actions: <Type, Action<Intent>>{
          _RunIntent: CallbackAction<_RunIntent>(
            onInvoke: (_) {
              _handleRun();
              return null;
            },
          ),
          _CopyIntent: CallbackAction<_CopyIntent>(
            onInvoke: (_) {
              _handleCopy();
              return null;
            },
          ),
        },
        child: Focus(
          autofocus: true,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              TextField(
                key: const Key('hex-to-dec-input'),
                controller: _input,
                autofocus: true,
                style: TextStyle(
                  fontFamily: upegMonoFontFamily,
                  fontFamilyFallback: upegMonoFontFamilyFallback,
                  fontSize: 12,
                  color: tokens.fg,
                ),
                decoration: InputDecoration(
                  labelText: 'hex',
                  hintText: t(ref, 'modal.hex.hint'),
                  border: OutlineInputBorder(
                    borderRadius: BorderRadius.circular(UpegSizing.radius1),
                  ),
                ),
              ),
              const SizedBox(height: 10),
              _PreviewBlock(result: _result, tokens: tokens),
              const SizedBox(height: 10),
              Row(
                children: [
                  Text(
                    t(ref, 'modal.hex.shortcut_hints'),
                    style: TextStyle(
                      fontFamily: upegMonoFontFamily,
                      fontFamilyFallback: upegMonoFontFamilyFallback,
                      fontSize: 10,
                      color: tokens.fg4,
                    ),
                  ),
                  const Spacer(),
                  TextButton(
                    key: const Key('hex-to-dec-run-btn'),
                    onPressed: _result is HexDecodeOk ? _handleRun : null,
                    child: Text(t(ref, 'modal.action.run_short')),
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _PreviewBlock extends ConsumerWidget {
  const _PreviewBlock({required this.result, required this.tokens});

  final HexDecodeResult result;
  final UpegTokens tokens;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final r = result;
    switch (r) {
      case HexDecodeEmpty():
        return Text(
          t(ref, 'modal.hex.empty'),
          style: TextStyle(
            color: tokens.fg4,
            fontStyle: FontStyle.italic,
            fontSize: 11,
          ),
        );
      case HexDecodeOk():
        return Container(
          padding: const EdgeInsets.all(10),
          decoration: BoxDecoration(
            color: tokens.bg2,
            border: Border.all(color: tokens.line),
            borderRadius: BorderRadius.circular(UpegSizing.radius1),
          ),
          child: Row(
            children: [
              Text(
                'decimal',
                style: TextStyle(
                  fontFamily: upegMonoFontFamily,
                  fontFamilyFallback: upegMonoFontFamilyFallback,
                  fontSize: 10,
                  color: tokens.fg4,
                ),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: SelectableText(
                  r.decimal.toString(),
                  style: TextStyle(
                    fontFamily: upegMonoFontFamily,
                    fontFamilyFallback: upegMonoFontFamilyFallback,
                    fontSize: 13,
                    color: tokens.fg,
                  ),
                ),
              ),
            ],
          ),
        );
      case HexDecodeError():
        return Text(
          t(ref, r.messageKey),
          style: TextStyle(color: tokens.warn, fontSize: 11),
        );
    }
  }
}
