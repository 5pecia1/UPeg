/// The quit confirmation the desktop shell routes every exit through.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/keyboard/keyboard_command_resolver.dart';
import 'package:upeg/src/rust/api/keyboard.dart';

/// Quit-confirm dialog — contract row `q` in
/// `docs/ui-ux-surface-contract.md`: quitting always routes through an
/// explicit confirmation, mirroring the TUI's `ConfirmQuit` view. Key
/// handling reuses the shared confirm scope (the TUI maps ConfirmQuit to
/// `KeyboardScope::ConfirmDelete` too), so F1 / Enter / `y` confirm and
/// Esc / `n` / `q` cancel — the binding policy stays canonical in
/// `upeg-core/src/keyboard.rs`. Styling mirrors the board delete-confirm
/// dialog in `board_tabs.dart`.
class QuitConfirmDialog extends ConsumerWidget {
  const QuitConfirmDialog({super.key});

  /// Stable widget-test contract keys.
  static const Key dialogKey = Key('quit-confirm-dialog');
  static const Key confirmButtonKey = Key('quit-confirm-submit');

  KeyEventResult _onKeyEvent(
    BuildContext context,
    WidgetRef ref,
    KeyEvent event,
  ) {
    final cmd = resolveKeyboardCommand(
      ref,
      event,
      scope: KeyboardScopeDto.confirmDelete,
    );
    return switch (cmd) {
      KeyboardCommandDto_Confirm() => _close(context, confirmed: true),
      KeyboardCommandDto_Cancel() => _close(context, confirmed: false),
      _ => KeyEventResult.ignored,
    };
  }

  KeyEventResult _close(BuildContext context, {required bool confirmed}) {
    Navigator.of(context).pop(confirmed);
    return KeyEventResult.handled;
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Focus(
      autofocus: true,
      onKeyEvent: (_, event) => _onKeyEvent(context, ref, event),
      child: AlertDialog(
        key: dialogKey,
        title: const Text('Quit upeg?'),
        content: const Text(
          'This closes the desktop app and releases its instance lock.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: const Text('Cancel'),
          ),
          TextButton(
            key: confirmButtonKey,
            onPressed: () => Navigator.of(context).pop(true),
            child: const Text('Quit'),
          ),
        ],
      ),
    );
  }
}
