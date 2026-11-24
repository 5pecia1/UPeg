/// Application-wide desktop commands, including commands from the macOS menu.
library;

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:upeg/src/widgets/notification_results_dialog.dart';
import 'package:upeg/src/widgets/settings_overlay.dart';
import 'package:upeg/src/widgets/cheatsheet_overlay.dart';

class DesktopAppCommands extends StatefulWidget {
  const DesktopAppCommands({
    required this.child,
    this.enabled = true,
    super.key,
  });
  final bool enabled;
  final Widget child;

  @override
  State<DesktopAppCommands> createState() => _DesktopAppCommandsState();
}

class _DesktopAppCommandsState extends State<DesktopAppCommands> {
  static const _channel = MethodChannel('upeg/app_commands');

  @override
  void initState() {
    super.initState();
    HardwareKeyboard.instance.addHandler(_handleKey);
    _channel.setMethodCallHandler(_handleNativeCommand);
  }

  @override
  void dispose() {
    HardwareKeyboard.instance.removeHandler(_handleKey);
    _channel.setMethodCallHandler(null);
    super.dispose();
  }

  Future<void> _handleNativeCommand(MethodCall call) async {
    if (call.method == 'settings') _openSettings();
    if (call.method == 'help' && widget.enabled) {
      final context = notificationResultsNavigatorKey.currentContext;
      if (context != null) showCheatsheetOverlay(context);
    }
  }

  bool _handleKey(KeyEvent event) {
    if (event is! KeyDownEvent ||
        event.logicalKey != LogicalKeyboardKey.comma) {
      return false;
    }
    final keyboard = HardwareKeyboard.instance;
    final mac = !kIsWeb && defaultTargetPlatform == TargetPlatform.macOS;
    final desktop =
        !kIsWeb &&
        (mac ||
            defaultTargetPlatform == TargetPlatform.windows ||
            defaultTargetPlatform == TargetPlatform.linux);
    if (!widget.enabled ||
        !desktop ||
        keyboard.isAltPressed ||
        keyboard.isShiftPressed ||
        (mac
            ? !keyboard.isMetaPressed || keyboard.isControlPressed
            : !keyboard.isControlPressed || keyboard.isMetaPressed)) {
      return false;
    }
    _openSettings();
    return true;
  }

  void _openSettings() {
    final context = notificationResultsNavigatorKey.currentContext;
    if (widget.enabled && context != null) showSettingsOverlay(context);
  }

  @override
  Widget build(BuildContext context) => widget.child;
}
