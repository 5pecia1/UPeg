/// Top board tab bar.
///
/// One underlined button per registered board, plus a `+ board`,
/// `+ pin`, `search ⌘K`, `edit`, and `settings` cluster on the
/// right. Switching writes to `currentBoardKeyProvider`.
///
/// The settings + edit + palette callbacks are owned by [BoardPage] so
/// the tab bar stays stateless; the underlying state lives in
/// providers, which means a settings flip from anywhere re-renders
/// this strip automatically.
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/keyboard/keyboard_command_resolver.dart';
import 'package:upeg/src/rust/api/keyboard.dart';
import 'package:upeg/src/rust/api/pegboard.dart' show BoardDto;
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/focused_pin_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/platform/keyboard_label.dart';
import 'package:upeg/src/widgets/no_transition_dialog.dart';
import 'package:upeg/src/widgets/board_details_dialog.dart';
import 'package:upeg/src/widgets/board_details_keys.dart';
import 'package:upeg/src/widgets/popover.dart';

/// Right-click context-menu actions on a board tab. Typed so the
/// menu builder and the result switch agree at compile time —
/// adding a new entry produces an exhaustiveness error in the
/// `switch` until it's handled.
enum BoardMenuAction { details, rename, delete }

const double _tabRowScrollBreakpoint = 720;

class BoardTabs extends ConsumerWidget {
  const BoardTabs({
    required this.onOpenPalette,
    required this.onOpenSettings,
    required this.onEditPinColor,
    super.key,
  });

  /// Opens the command palette overlay.
  final VoidCallback onOpenPalette;

  /// Opens the settings dialog/overlay.
  final VoidCallback onOpenSettings;

  /// Opens the pin color dialog for the currently focused pin.
  final VoidCallback onEditPinColor;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = context.upeg;
    final AsyncValue<List<BoardDto>> boards = ref.watch(boardsProvider);
    final selected = ref.watch(currentBoardKeyProvider);
    return Container(
      height: UpegSizing.tabBarHeight,
      padding: const EdgeInsets.symmetric(horizontal: 12),
      decoration: BoxDecoration(
        color: tokens.bg,
        border: Border(bottom: BorderSide(color: tokens.line)),
      ),
      child: boards.when(
        loading: () => const Align(
          alignment: Alignment.centerLeft,
          child: SizedBox(
            width: 120,
            child: LinearProgressIndicator(minHeight: 2),
          ),
        ),
        error: (err, _) => Text(
          t(ref, 'desktop.tab.load_failed', {'msg': '$err'}),
          style: TextStyle(color: tokens.warn, fontSize: 11),
        ),
        data: (list) => _TabRow(
          boards: list,
          selectedKey: selected?.value,
          onSelect: (key) {
            ref
                .read(currentBoardKeyProvider.notifier)
                .select(BoardKey.parse(key));
          },
          onAddBoard: () => promptCreateBoard(context, ref),
          onRenameBoard: (board) => promptRenameBoard(context, ref, board),
          onDeleteBoard: (board) => confirmDeleteBoard(context, ref, board),
          onOpenPalette: onOpenPalette,
          onOpenSettings: onOpenSettings,
          onEditPinColor: onEditPinColor,
        ),
      ),
    );
  }
}

class _TabRow extends ConsumerWidget {
  const _TabRow({
    required this.boards,
    required this.selectedKey,
    required this.onSelect,
    required this.onAddBoard,
    required this.onRenameBoard,
    required this.onDeleteBoard,
    required this.onOpenPalette,
    required this.onOpenSettings,
    required this.onEditPinColor,
  });

  final List<BoardDto> boards;
  final String? selectedKey;
  final ValueChanged<String> onSelect;
  final VoidCallback onAddBoard;
  final ValueChanged<BoardDto> onRenameBoard;
  final ValueChanged<BoardDto> onDeleteBoard;
  final VoidCallback onOpenPalette;
  final VoidCallback onOpenSettings;
  final VoidCallback onEditPinColor;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final compact = constraints.maxWidth < _tabRowScrollBreakpoint;
        if (compact) {
          return SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: ConstrainedBox(
              constraints: BoxConstraints(minWidth: constraints.maxWidth),
              child: Row(children: _compactChildren(context, ref)),
            ),
          );
        }

        return Row(
          children: [
            Expanded(
              child: SingleChildScrollView(
                scrollDirection: Axis.horizontal,
                child: Row(children: _boardChildren(context, ref)),
              ),
            ),
            const SizedBox(width: 12),
            ..._actionChildren(context, ref),
          ],
        );
      },
    );
  }

  List<Widget> _compactChildren(BuildContext context, WidgetRef ref) {
    return [
      ..._boardChildren(context, ref),
      const SizedBox(width: 12),
      ..._actionChildren(context, ref),
    ];
  }

  List<Widget> _boardChildren(BuildContext context, WidgetRef ref) {
    return [
      for (final board in boards)
        _BoardTabButton(
          key: ValueKey('board-tab-${board.key}'),
          board: board,
          active: board.key == selectedKey,
          onTap: () => onSelect(board.key),
          onRename: () => onRenameBoard(board),
          onDelete: () => onDeleteBoard(board),
        ),
      const SizedBox(width: 4),
      _GhostButton(
        key: const Key('add-board-btn'),
        label: t(ref, 'desktop.tab.add_board'),
        onPressed: onAddBoard,
      ),
    ];
  }

  List<Widget> _actionChildren(BuildContext context, WidgetRef ref) {
    final tokens = context.upeg;
    final hasFocusedPin = ref.watch(focusedPinProvider) != null;
    return [
      _GhostButton(
        key: BoardDetailsKeys.open,
        icon: const Icon(Icons.info_outline, size: 11),
        label: t(ref, 'board.details.open'),
        onPressed: selectedKey == null
            ? null
            : () => unawaited(showBoardDetailsDialog(context, selectedKey!)),
      ),
      const SizedBox(width: 4),
      _GhostButton(
        key: const Key('edit-pin-color-btn'),
        icon: const Icon(Icons.palette_outlined, size: 11),
        label: t(ref, 'desktop.tab.pin_color'),
        tooltip: hasFocusedPin
            ? t(ref, 'desktop.tab.pin_color')
            : t(ref, 'desktop.tab.pin_color_disabled'),
        onPressed: hasFocusedPin ? onEditPinColor : null,
      ),
      const SizedBox(width: 4),
      _GhostButton(
        key: const Key('open-palette-btn'),
        label: t(ref, 'desktop.tab.add_tool'),
        accent: tokens.accent,
        accentFill: true,
        onPressed: onOpenPalette,
      ),
      const SizedBox(width: 4),
      _GhostButton(
        icon: const Icon(Icons.search, size: 11),
        label: t(ref, 'desktop.tab.search').trim(),
        trailing: _Kbd(
          text: shortcutLabel(ref.watch(keyboardPlatformProvider), 'K'),
        ),
        onPressed: onOpenPalette,
      ),
      const SizedBox(width: 4),
      _GhostButton(
        key: const Key('open-settings-btn'),
        icon: const Icon(Icons.settings_outlined, size: 11),
        label: t(ref, 'desktop.tab.settings'),
        onPressed: onOpenSettings,
      ),
    ];
  }
}

class _BoardTabButton extends ConsumerWidget {
  const _BoardTabButton({
    required this.board,
    required this.active,
    required this.onTap,
    required this.onRename,
    required this.onDelete,
    super.key,
  });

  final BoardDto board;
  final bool active;
  final VoidCallback onTap;
  final VoidCallback onRename;
  final VoidCallback onDelete;

  Future<void> _showContextMenu(
    BuildContext context,
    WidgetRef ref,
    Offset globalPosition,
  ) {
    return showUpegPopover<BoardMenuAction>(
      context: context,
      globalPosition: globalPosition,
      items: [
        UpegPopoverItem<BoardMenuAction>(
          key: Key('board-menu-rename-${board.key}'),
          value: BoardMenuAction.rename,
          label: tRead(ref, 'desktop.tab.menu_rename'),
        ),
        UpegPopoverItem<BoardMenuAction>(
          key: Key('board-menu-delete-${board.key}'),
          value: BoardMenuAction.delete,
          label: tRead(ref, 'desktop.tab.menu_delete'),
        ),
        UpegPopoverItem<BoardMenuAction>(
          value: BoardMenuAction.details,
          label: tRead(ref, 'board.details.open'),
        ),
      ],
    ).then((value) {
      switch (value) {
        case BoardMenuAction.details:
          if (context.mounted) {
            unawaited(showBoardDetailsDialog(context, board.key));
          }
        case BoardMenuAction.rename:
          onRename();
        case BoardMenuAction.delete:
          onDelete();
        case null:
          break;
      }
    });
  }

  Future<void> _showKeyboardContextMenu(BuildContext context, WidgetRef ref) {
    final renderObject = context.findRenderObject();
    final box = renderObject is RenderBox ? renderObject : null;
    final position = box == null
        ? Offset.zero
        : box.localToGlobal(Offset(0, box.size.height));
    return _showContextMenu(context, ref, position);
  }

  KeyEventResult _handleKey(
    BuildContext context,
    WidgetRef ref,
    KeyEvent event,
  ) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
      return KeyEventResult.ignored;
    }
    if (isKeyboardContextMenuEvent(event)) {
      unawaited(_showKeyboardContextMenu(context, ref));
      return KeyEventResult.handled;
    }
    if (isKeyboardActivationEvent(event)) {
      onTap();
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = context.upeg;
    final color = active ? tokens.fg : tokens.fg3;
    final underline = active ? tokens.accent : Colors.transparent;
    return Focus(
      onKeyEvent: (_, event) => _handleKey(context, ref, event),
      child: GestureDetector(
        onSecondaryTapDown: (details) =>
            unawaited(_showContextMenu(context, ref, details.globalPosition)),
        onLongPressStart: (details) =>
            unawaited(_showContextMenu(context, ref, details.globalPosition)),
        child: InkWell(
          onTap: onTap,
          child: Container(
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
            decoration: BoxDecoration(
              border: Border(bottom: BorderSide(color: underline, width: 2)),
            ),
            child: Text(
              board.title,
              style: TextStyle(
                color: color,
                fontSize: 12,
                fontFamily: upegMonoFontFamily,
                fontFamilyFallback: upegMonoFontFamilyFallback,
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// Ghost outlined chip with optional accent fill.
class _GhostButton extends StatelessWidget {
  const _GhostButton({
    this.label,
    this.icon,
    this.trailing,
    this.onPressed,
    this.accent,
    this.accentFill = false,
    this.tooltip,
    super.key,
  });

  final String? label;
  final Widget? icon;
  final Widget? trailing;
  final VoidCallback? onPressed;

  /// When set, paints the border (and, with [accentFill], the
  /// background) in this colour. Mirrors the `.gh.primary` modifier
  /// where the accent flood-fills.
  final Color? accent;
  final bool accentFill;

  /// Tooltip text shown on hover. When null, no tooltip is displayed.
  final String? tooltip;

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    final borderColor = accent ?? tokens.line;
    final bgColor = accentFill ? (accent ?? tokens.accent) : Colors.transparent;
    final fgColor = accentFill
        ? UpegTokens.onFill(bgColor)
        : (accent ?? tokens.fg2);
    final disabled = onPressed == null;
    final child = Semantics(
      button: true,
      enabled: !disabled,
      label: label,
      child: InkWell(
        onTap: onPressed,
        borderRadius: BorderRadius.circular(UpegSizing.radius1),
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
          decoration: BoxDecoration(
            color: bgColor,
            border: Border.all(color: borderColor),
            borderRadius: BorderRadius.circular(UpegSizing.radius1),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              if (icon != null) ...[
                IconTheme.merge(
                  data: IconThemeData(color: fgColor, size: 11),
                  child: icon!,
                ),
                const SizedBox(width: 5),
              ],
              if (label != null)
                Text(
                  label!,
                  style: TextStyle(
                    fontFamily: upegMonoFontFamily,
                    fontFamilyFallback: upegMonoFontFamilyFallback,
                    fontSize: 11,
                    fontWeight: FontWeight.w500,
                    color: fgColor,
                    height: 1.0,
                  ),
                ),
              if (trailing != null) ...[
                const SizedBox(width: 5),
                DefaultTextStyle.merge(
                  style: TextStyle(color: fgColor),
                  child: trailing!,
                ),
              ],
            ],
          ),
        ),
      ),
    );
    if (disabled) {
      return Opacity(
        opacity: 0.45,
        child: tooltip != null
            ? Tooltip(message: tooltip!, child: child)
            : child,
      );
    }
    if (tooltip != null) {
      return Tooltip(message: tooltip!, child: child);
    }
    return child;
  }
}

/// Show the "+ board" text dialog. On submit it calls `createBoard`
/// and switches the current-board selection to the new key so the
/// user lands on their fresh tab immediately.
Future<void> promptCreateBoard(BuildContext context, WidgetRef ref) async {
  final title = await showNoTransitionDialog<String>(
    context: context,
    builder: (_) => const _BoardNameDialog(
      dialogKey: Key('create-board-dialog'),
      fieldKey: Key('create-board-field'),
      submitKey: Key('create-board-submit'),
      titleKey: 'desktop.tab.create_prompt',
      hintTextKey: 'desktop.tab.new_placeholder',
      submitLabelKey: 'desktop.tab.create_confirm',
    ),
  );
  final raw = title?.trim();
  if (raw == null || raw.isEmpty) return;
  final String key;
  try {
    key = ref.read(boardCreatorProvider)(raw);
  } on Object catch (err, stack) {
    debugPrint('createBoard failed: $err');
    debugPrint(stack.toString());
    return;
  }
  ref.invalidate(boardsProvider);
  ref.read(currentBoardKeyProvider.notifier).select(BoardKey.parse(key));
}

Future<void> promptRenameBoard(
  BuildContext context,
  WidgetRef ref,
  BoardDto board,
) async {
  final next = await showNoTransitionDialog<String>(
    context: context,
    builder: (_) => _BoardNameDialog(
      dialogKey: Key('rename-board-dialog-${board.key}'),
      fieldKey: const Key('rename-board-field'),
      submitKey: const Key('rename-board-submit'),
      titleKey: 'desktop.tab.rename_prompt_titled',
      titleArgs: {'title': board.title},
      initialValue: board.title,
      submitLabelKey: 'desktop.tab.rename_confirm_title',
    ),
  );
  final raw = next?.trim();
  if (raw == null || raw.isEmpty || raw == board.title) return;
  try {
    ref.read(boardRenamerProvider)(board.key, raw);
  } on Object catch (err, stack) {
    debugPrint('renameBoard failed: $err');
    debugPrint(stack.toString());
    return;
  }
  ref.invalidate(boardsProvider);
}

Future<void> confirmDeleteBoard(
  BuildContext context,
  WidgetRef ref,
  BoardDto board,
) async {
  final ok = await showNoTransitionDialog<bool>(
    context: context,
    builder: (_) => _ConfirmDeleteBoardDialog(board: board),
  );
  if (ok != true) return;
  try {
    ref.read(boardDeleterProvider)(board.key);
  } on Object catch (err, stack) {
    debugPrint('deleteBoard failed: $err');
    debugPrint(stack.toString());
    return;
  }
  // If the user deleted the active board, drop the selection so the
  // BoardPage never sits on a null body while other boards remain.
  if (ref.read(currentBoardKeyProvider)?.value == board.key) {
    final fallback = _firstRemainingBoardKey(
      ref.read(boardsProvider).value ?? const <BoardDto>[],
      deletedKey: board.key,
    );
    if (fallback == null) {
      ref.read(currentBoardKeyProvider.notifier).clear();
    } else {
      ref.read(currentBoardKeyProvider.notifier).select(fallback);
    }
  }
  ref.invalidate(boardsProvider);
}

BoardKey? _firstRemainingBoardKey(
  List<BoardDto> boards, {
  required String deletedKey,
}) {
  for (final board in boards) {
    if (board.key != deletedKey) {
      return BoardKey.parse(board.key);
    }
  }
  return null;
}

class _BoardNameDialog extends ConsumerStatefulWidget {
  const _BoardNameDialog({
    required this.dialogKey,
    required this.fieldKey,
    required this.submitKey,
    required this.titleKey,
    required this.submitLabelKey,
    this.titleArgs,
    this.initialValue = '',
    this.hintTextKey,
  });

  final Key dialogKey;
  final Key fieldKey;
  final Key submitKey;

  /// Catalog key (+ optional `{placeholder}` args) for the dialog title.
  final String titleKey;
  final Map<String, String>? titleArgs;

  /// Catalog key for the submit button label.
  final String submitLabelKey;
  final String initialValue;

  /// Catalog key for the text-field hint (`null` = no hint).
  final String? hintTextKey;

  @override
  ConsumerState<_BoardNameDialog> createState() => _BoardNameDialogState();
}

class _BoardNameDialogState extends ConsumerState<_BoardNameDialog> {
  late final TextEditingController _controller;

  @override
  void initState() {
    super.initState();
    _controller = TextEditingController(text: widget.initialValue);
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  KeyEventResult _onKeyEvent(FocusNode node, KeyEvent event) {
    final cmd = resolveKeyboardCommand(
      ref,
      event,
      scope: KeyboardScopeDto.boardEditor,
    );
    return switch (cmd) {
      KeyboardCommandDto_Commit() => _commit(),
      KeyboardCommandDto_Cancel() => _cancel(),
      _ => KeyEventResult.ignored,
    };
  }

  KeyEventResult _commit() {
    Navigator.of(context).pop(_controller.text);
    return KeyEventResult.handled;
  }

  KeyEventResult _cancel() {
    Navigator.of(context).pop();
    return KeyEventResult.handled;
  }

  @override
  Widget build(BuildContext context) {
    return Focus(
      canRequestFocus: false,
      onKeyEvent: _onKeyEvent,
      child: AlertDialog(
        key: widget.dialogKey,
        title: Text(t(ref, widget.titleKey, widget.titleArgs)),
        content: TextField(
          key: widget.fieldKey,
          controller: _controller,
          autofocus: true,
          decoration: InputDecoration(
            hintText: widget.hintTextKey == null
                ? null
                : t(ref, widget.hintTextKey!),
          ),
          onSubmitted: (value) => Navigator.of(context).pop(value),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: Text(t(ref, 'desktop.tab.remove_cancel')),
          ),
          TextButton(
            key: widget.submitKey,
            onPressed: () => Navigator.of(context).pop(_controller.text),
            child: Text(t(ref, widget.submitLabelKey)),
          ),
        ],
      ),
    );
  }
}

class _ConfirmDeleteBoardDialog extends ConsumerWidget {
  const _ConfirmDeleteBoardDialog({required this.board});

  final BoardDto board;

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
        key: Key('delete-board-dialog-${board.key}'),
        title: Text(
          t(ref, 'desktop.tab.remove_confirm', {'title': board.title}),
        ),
        content: Text(t(ref, 'desktop.tab.remove_confirm_body')),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: Text(t(ref, 'desktop.tab.remove_cancel')),
          ),
          TextButton(
            key: const Key('delete-board-submit'),
            onPressed: () => Navigator.of(context).pop(true),
            child: Text(t(ref, 'desktop.tab.remove_confirm_ok')),
          ),
        ],
      ),
    );
  }
}

/// Inline keyboard-shortcut chip used in the search button.
class _Kbd extends StatelessWidget {
  const _Kbd({required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 2),
      decoration: BoxDecoration(
        color: tokens.surface2,
        border: Border.all(color: tokens.line),
        borderRadius: BorderRadius.circular(3),
      ),
      child: Text(
        text,
        style: TextStyle(
          fontFamily: upegMonoFontFamily,
          fontFamilyFallback: upegMonoFontFamilyFallback,
          fontSize: 11,
          color: tokens.fg2,
          height: 1.0,
        ),
      ),
    );
  }
}
