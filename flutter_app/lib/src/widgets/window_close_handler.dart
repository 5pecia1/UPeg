import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/pages/board_page/quit_confirm_dialog.dart';
import 'package:upeg/src/platform/app_lifecycle.dart';
import 'package:upeg/src/widgets/no_transition_dialog.dart';
import 'package:window_manager/window_manager.dart';

/// Keeps the native close listener alive across popup and board modes.
class WindowCloseHandler extends ConsumerStatefulWidget {
  const WindowCloseHandler({required this.builder, super.key});

  final Widget Function(Future<void> Function() requestQuit) builder;

  @override
  ConsumerState<WindowCloseHandler> createState() => _WindowCloseHandlerState();
}

class _WindowCloseHandlerState extends ConsumerState<WindowCloseHandler>
    with WindowListener {
  bool _quitInProgress = false;

  @override
  void initState() {
    super.initState();
    windowManager.addListener(this);
    unawaited(installWindowCloseIntercept());
  }

  @override
  void dispose() {
    windowManager.removeListener(this);
    super.dispose();
  }

  @override
  void onWindowClose() => unawaited(_requestQuit());

  Future<void> _requestQuit() async {
    if (_quitInProgress) return;
    _quitInProgress = true;
    try {
      await runQuitRequest(
        confirm: () async =>
            await showNoTransitionDialog<bool>(
              context: context,
              builder: (_) => const QuitConfirmDialog(),
            ) ??
            false,
        quit: ref.read(quitAppProvider),
      );
    } finally {
      _quitInProgress = false;
    }
  }

  @override
  Widget build(BuildContext context) => widget.builder(_requestQuit);
}
