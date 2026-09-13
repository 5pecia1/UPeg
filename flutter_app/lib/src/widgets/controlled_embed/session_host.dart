/// Keeps each browser in a real viewport while its debugger is closed.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/features/controlled_embed/providers.dart';
import 'package:upeg/src/features/controlled_embed/session_service.dart';

const kControlledEmbedSessionHostKey = Key('controlled-embed-session-host');
const _hiddenViewportGap = 16.0;

class ControlledEmbedSessionHost extends ConsumerWidget {
  const ControlledEmbedSessionHost({required this.child, super.key});
  final Widget child;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    if (!ref.watch(controlledEmbedNativeSupportedProvider)) return child;
    return ControlledEmbedSessionHostView(
      sessions: ref.watch(controlledEmbedSessionsProvider),
      child: child,
    );
  }
}

class ControlledEmbedSessionHostView extends StatelessWidget {
  const ControlledEmbedSessionHostView({
    required this.sessions,
    required this.child,
    super.key,
  });

  final ControlledEmbedSessionService sessions;
  final Widget child;

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: sessions,
    child: child,
    builder: (context, child) => Stack(
      key: kControlledEmbedSessionHostKey,
      clipBehavior: Clip.hardEdge,
      children: [
        for (final entry in sessions.entries)
          if (!entry.displayedInDebugger)
            Positioned(
              key: ValueKey(entry),
              left: -entry.browser.viewportSize.width - _hiddenViewportGap,
              top: -entry.browser.viewportSize.height - _hiddenViewportGap,
              width: entry.browser.viewportSize.width,
              height: entry.browser.viewportSize.height,
              child: IgnorePointer(
                child: ExcludeSemantics(
                  child: ExcludeFocus(
                    child: entry.browser.buildView(visible: false),
                  ),
                ),
              ),
            ),
        child!,
      ],
    ),
  );
}
