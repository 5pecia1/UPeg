/// Custom popover in the upeg "ghost button" menu style.
///
/// Right-click menu contract: transparent background, single-pixel
/// border via [UpegTokens.line], rounded 4 corners (no Material
/// elevation), tight padding. Each menu item renders as a "ghost
/// button" — transparent background, hover → `surface2` tint, no
/// leading icon.
///
/// Why not [showMenu]?
/// `showMenu()` paints a `PopupMenuButton` with Material 3 elevation,
/// surfaceTint, item paddings (8/16/8/8) and a soft drop shadow that
/// breaks upeg's flat aesthetic. This widget implements the popover
/// surface against [UpegTokens] directly so it keeps the flat,
/// line-bordered look.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/keyboard/keyboard_command_resolver.dart';
import 'package:upeg/src/rust/api/keyboard.dart';
import 'package:upeg/src/theme/upeg_theme.dart';

const int _previousMenuItemOffset = -1;
const int _nextMenuItemOffset = 1;
const String _spaceText = ' ';

int _offsetForDirection(DirectionDto direction) {
  return switch (direction) {
    DirectionDto.left || DirectionDto.up => _previousMenuItemOffset,
    DirectionDto.right || DirectionDto.down => _nextMenuItemOffset,
  };
}

/// A single entry in [UpegPopover]. The selected value is returned
/// from [showUpegPopover].
@immutable
class UpegPopoverItem<T> {
  const UpegPopoverItem({required this.value, required this.label, this.key});

  final T value;
  final String label;
  final Key? key;
}

/// Show a popover anchored at [globalPosition] (typically the right-click
/// pointer position). Returns the selected value, or `null` if the user
/// dismissed by tapping outside.
Future<T?> showUpegPopover<T>({
  required BuildContext context,
  required Offset globalPosition,
  required List<UpegPopoverItem<T>> items,
}) {
  return Navigator.of(context).push<T>(
    _UpegPopoverRoute<T>(
      globalPosition: globalPosition,
      items: items,
      barrierLabel: MaterialLocalizations.of(context).modalBarrierDismissLabel,
    ),
  );
}

class _UpegPopoverRoute<T> extends PopupRoute<T> {
  _UpegPopoverRoute({
    required this.globalPosition,
    required this.items,
    required this.barrierLabel,
  });

  final Offset globalPosition;
  final List<UpegPopoverItem<T>> items;

  @override
  final String barrierLabel;

  @override
  Color? get barrierColor => null;

  @override
  bool get barrierDismissible => true;

  @override
  Duration get transitionDuration => Duration.zero;

  @override
  Duration get reverseTransitionDuration => Duration.zero;

  @override
  Widget buildPage(
    BuildContext context,
    Animation<double> animation,
    Animation<double> secondaryAnimation,
  ) {
    return _UpegPopoverSurface<T>(position: globalPosition, items: items);
  }

  @override
  Widget buildTransitions(
    BuildContext context,
    Animation<double> animation,
    Animation<double> secondaryAnimation,
    Widget child,
  ) {
    return child;
  }
}

class _UpegPopoverSurface<T> extends ConsumerStatefulWidget {
  const _UpegPopoverSurface({required this.position, required this.items});

  final Offset position;
  final List<UpegPopoverItem<T>> items;

  @override
  ConsumerState<_UpegPopoverSurface<T>> createState() =>
      _UpegPopoverSurfaceState<T>();
}

class _UpegPopoverSurfaceState<T>
    extends ConsumerState<_UpegPopoverSurface<T>> {
  final FocusNode _focusNode = FocusNode(debugLabel: 'upeg-popover');
  int _selectedIndex = 0;

  @override
  void dispose() {
    _focusNode.dispose();
    super.dispose();
  }

  KeyEventResult _handleKey(FocusNode node, KeyEvent event) {
    final menuCommand = resolveKeyboardCommand(
      ref,
      event,
      scope: KeyboardScopeDto.settings,
    );
    final menuResult = switch (menuCommand) {
      KeyboardCommandDto_Close() => _close(),
      KeyboardCommandDto_FocusNext() => _moveSelection(_nextMenuItemOffset),
      KeyboardCommandDto_FocusPrevious() => _moveSelection(
        _previousMenuItemOffset,
      ),
      KeyboardCommandDto_Move(:final direction) => _moveSelection(
        _offsetForDirection(direction),
      ),
      _ => KeyEventResult.ignored,
    };
    if (menuResult == KeyEventResult.handled) return menuResult;

    final activationCommand = resolveKeyboardCommand(
      ref,
      event,
      scope: KeyboardScopeDto.form,
    );
    return switch (activationCommand) {
      KeyboardCommandDto_Run() => _activateSelection(),
      KeyboardCommandDto_Text(:final ch) when ch == _spaceText =>
        _activateSelection(),
      KeyboardCommandDto_Close() => _close(),
      _ => KeyEventResult.ignored,
    };
  }

  KeyEventResult _close() {
    Navigator.of(context).pop<T>();
    return KeyEventResult.handled;
  }

  KeyEventResult _moveSelection(int delta) {
    if (widget.items.isEmpty) return KeyEventResult.ignored;
    setState(() {
      _selectedIndex =
          (_selectedIndex + delta + widget.items.length) % widget.items.length;
    });
    return KeyEventResult.handled;
  }

  KeyEventResult _activateSelection() {
    if (widget.items.isEmpty) return KeyEventResult.ignored;
    Navigator.of(context).pop<T>(widget.items[_selectedIndex].value);
    return KeyEventResult.handled;
  }

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    return Focus(
      focusNode: _focusNode,
      autofocus: true,
      onKeyEvent: _handleKey,
      child: Stack(
        children: [
          Positioned(
            left: widget.position.dx,
            top: widget.position.dy,
            child: Material(
              color: Colors.transparent,
              elevation: 0,
              child: ConstrainedBox(
                constraints: const BoxConstraints(minWidth: 132, maxWidth: 240),
                child: Container(
                  decoration: BoxDecoration(
                    color: tokens.surface,
                    border: Border.all(color: tokens.line),
                    borderRadius: BorderRadius.circular(UpegSizing.radius1),
                  ),
                  padding: const EdgeInsets.symmetric(vertical: 4),
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      for (final entry in widget.items.indexed)
                        _GhostMenuItem<T>(
                          item: entry.$2,
                          selected: entry.$1 == _selectedIndex,
                          onTap: () =>
                              Navigator.of(context).pop<T>(entry.$2.value),
                        ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _GhostMenuItem<T> extends StatefulWidget {
  const _GhostMenuItem({
    required this.item,
    required this.selected,
    required this.onTap,
  });

  final UpegPopoverItem<T> item;
  final bool selected;
  final VoidCallback onTap;

  @override
  State<_GhostMenuItem<T>> createState() => _GhostMenuItemState<T>();
}

class _GhostMenuItemState<T> extends State<_GhostMenuItem<T>> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    final active = _hover || widget.selected;
    final bg = active ? tokens.surface2 : Colors.transparent;
    final fg = active ? tokens.fg : tokens.fg2;
    return Semantics(
      button: true,
      selected: widget.selected,
      child: MouseRegion(
        onEnter: (_) => setState(() => _hover = true),
        onExit: (_) => setState(() => _hover = false),
        cursor: SystemMouseCursors.click,
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: widget.onTap,
          child: Container(
            key: widget.item.key,
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
            color: bg,
            child: Text(
              widget.item.label,
              style: TextStyle(
                fontFamily: upegMonoFontFamily,
                fontFamilyFallback: upegMonoFontFamilyFallback,
                fontSize: 12,
                color: fg,
                height: 1.0,
              ),
            ),
          ),
        ),
      ),
    );
  }
}
