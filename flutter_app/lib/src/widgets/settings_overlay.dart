/// Settings overlay.
///
/// A centered modal card with section headers (theme / layout /
/// language / backup) and segmented radio-style buttons mapped to
/// [TweaksDto] fields.
///
/// Wraps [TweaksForm] so widget tests can drive the form body in
/// isolation; the overlay just provides the modal shell + scaffolding.
library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/keyboard/keyboard_command_resolver.dart';
import 'package:upeg/src/rust/api/keyboard.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/state/diagnostics_provider.dart';
import 'package:upeg/src/widgets/no_transition_dialog.dart';
import 'package:upeg/src/widgets/tweaks_form.dart';

/// Helper used from the BoardPage settings button. Keeps the modal route
/// transition-free so close/open feels instant like the TUI.
Future<void> showSettingsOverlay(BuildContext context) {
  return showNoTransitionDialog<void>(
    context: context,
    barrierColor: Colors.black54,
    builder: (_) => const _SettingsModal(),
  );
}

class _SettingsModal extends ConsumerStatefulWidget {
  const _SettingsModal();

  @override
  ConsumerState<_SettingsModal> createState() => _SettingsModalState();
}

class _SettingsModalState extends ConsumerState<_SettingsModal> {
  final FocusNode _closeFocusNode = FocusNode(debugLabel: 'settings-close');

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) ref.invalidate(recentDiagnosticsProvider);
    });
  }

  @override
  void dispose() {
    _closeFocusNode.dispose();
    super.dispose();
  }

  KeyEventResult _handleSettingsKey(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
      return KeyEventResult.ignored;
    }
    // Let a settings text field finish every ordinary character before the
    // dialog's vim-like navigation resolver sees it. This includes `/`, a
    // normal absolute-path character that otherwise opens the board palette
    // while editing the project directory. The editable receives the text
    // input first; handling the bubbling key here only prevents an ancestor
    // shortcut. Explicit global chords remain available.
    if (primaryFocusIsEditableText() && !isGlobalShortcutKeyEvent(event)) {
      // Do not bubble into the settings/board shortcut handlers, while still
      // letting the platform text-input channel update EditableText. `handled`
      // also suppresses macOS text entry, which made absolute paths appear to
      // focus correctly but never receive their characters.
      return KeyEventResult.skipRemainingHandlers;
    }
    final cmd = resolveKeyboardCommand(
      ref,
      event,
      scope: KeyboardScopeDto.settings,
    );
    return switch (cmd) {
      KeyboardCommandDto_Close() => _close(),
      KeyboardCommandDto_FocusNext() => _focusNext(),
      KeyboardCommandDto_FocusPrevious() => _focusPrevious(),
      _ => KeyEventResult.ignored,
    };
  }

  KeyEventResult _handleCloseKey(FocusNode node, KeyEvent event) {
    if (isKeyboardActivationEvent(event)) {
      Navigator.of(context).pop();
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  KeyEventResult _close() {
    Navigator.of(context).pop();
    return KeyEventResult.handled;
  }

  KeyEventResult _focusNext() {
    FocusScope.of(context).nextFocus();
    return KeyEventResult.handled;
  }

  KeyEventResult _focusPrevious() {
    FocusScope.of(context).previousFocus();
    return KeyEventResult.handled;
  }

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    return FocusTraversalGroup(
      policy: WidgetOrderTraversalPolicy(),
      child: Focus(
        canRequestFocus: false,
        onKeyEvent: _handleSettingsKey,
        child: Dialog(
          backgroundColor: Colors.transparent,
          elevation: 0,
          insetPadding: const EdgeInsets.all(24),
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 360),
            child: Container(
              padding: const EdgeInsets.fromLTRB(18, 16, 18, 18),
              decoration: BoxDecoration(
                color: tokens.surface,
                border: Border.all(color: tokens.line),
                borderRadius: BorderRadius.circular(UpegSizing.radius3),
                boxShadow: const [
                  BoxShadow(
                    color: Color(0x66000000),
                    blurRadius: 64,
                    offset: Offset(0, 24),
                  ),
                ],
              ),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Row(
                    children: [
                      Text(
                        t(ref, 'settings.title').toUpperCase(),
                        style: TextStyle(
                          fontFamily: upegMonoFontFamily,
                          fontFamilyFallback: upegMonoFontFamilyFallback,
                          fontSize: 10,
                          letterSpacing: 0.6,
                          color: tokens.fg3,
                        ),
                      ),
                      const Spacer(),
                      Focus(
                        focusNode: _closeFocusNode,
                        autofocus: true,
                        descendantsAreFocusable: false,
                        descendantsAreTraversable: false,
                        onFocusChange: (_) => setState(() {}),
                        onKeyEvent: _handleCloseKey,
                        child: SizedBox(
                          key: const Key('settings-close-focus'),
                          child: TextButton(
                            key: const Key('settings-close-btn'),
                            style: TextButton.styleFrom(
                              foregroundColor: tokens.fg2,
                              padding: const EdgeInsets.symmetric(
                                horizontal: 8,
                                vertical: 4,
                              ),
                              shape: RoundedRectangleBorder(
                                side: BorderSide(
                                  color: _closeFocusNode.hasPrimaryFocus
                                      ? tokens.accent
                                      : tokens.line,
                                ),
                                borderRadius: BorderRadius.circular(
                                  UpegSizing.radius1,
                                ),
                              ),
                              textStyle: const TextStyle(
                                fontSize: 11,
                                fontWeight: FontWeight.w500,
                              ),
                            ),
                            onPressed: () => Navigator.of(context).pop(),
                            child: Text(t(ref, 'settings.close')),
                          ),
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(height: 14),
                  // Settings now hosts Theme / Layout / Language / Backup /
                  // Services — the combined height exceeds the modal's
                  // viewport on small displays, so wrap in a Flexible +
                  // SingleChildScrollView so the form scrolls instead of
                  // overflowing the Dialog.
                  Flexible(
                    child: SingleChildScrollView(child: const TweaksForm()),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
