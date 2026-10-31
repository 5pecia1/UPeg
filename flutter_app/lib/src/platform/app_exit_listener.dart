/// Handles native app-exit requests that bypass the window-close and keyboard
/// quit handlers, notably Quit from the macOS application menu.
library;

import 'dart:ui' show AppExitResponse;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/state/inline_draft_provider.dart';

class AppExitListener extends ConsumerStatefulWidget {
  const AppExitListener({required this.child, super.key});

  final Widget child;

  @override
  ConsumerState<AppExitListener> createState() => _AppExitListenerState();
}

class _AppExitListenerState extends ConsumerState<AppExitListener> {
  late final AppLifecycleListener _listener;

  @override
  void initState() {
    super.initState();
    _listener = AppLifecycleListener(onExitRequested: _onExitRequested);
  }

  Future<AppExitResponse> _onExitRequested() async {
    // Another observer may still cancel the OS request, so keep the instance
    // lock until process exit instead of running the explicit quit pipeline.
    ref.read(inlineDraftProvider).flushAll();
    return AppExitResponse.exit;
  }

  @override
  void dispose() {
    _listener.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => widget.child;
}
