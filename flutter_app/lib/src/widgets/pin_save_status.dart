/// A persistent recovery affordance for a draft/result that was not saved.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/state/inline_draft_provider.dart';
import 'package:upeg/src/state/last_outcome_provider.dart';
import 'package:upeg/src/state/pin_provider.dart';

/// Do not let a temporarily unreadable saved draft look like a fresh, empty
/// form that is safe to overwrite. Retry restores editing after loading it.
class PinDraftEditor extends ConsumerWidget {
  const PinDraftEditor({required this.pinKey, required this.child, super.key});

  final PinKey? pinKey;
  final Widget child;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final pin = pinKey;
    if (pin == null) return child;
    final drafts = ref.watch(inlineDraftProvider);
    return ListenableBuilder(
      listenable: drafts,
      builder: (context, _) {
        final blocked = drafts.hasLoadFailure(pin);
        return ExcludeFocus(
          excluding: blocked,
          child: IgnorePointer(ignoring: blocked, child: child),
        );
      },
    );
  }
}

class PinSaveStatus extends ConsumerWidget {
  const PinSaveStatus({
    required this.pinKey,
    required this.toolId,
    this.compact = false,
    super.key,
  });

  final PinKey pinKey;
  final ToolId toolId;
  final bool compact;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final drafts = ref.watch(inlineDraftProvider);
    final outcome = ref.watch(pinLastOutcomeProvider(pinKey));
    // Observe map updates as well as this pin's result: failed hydration has
    // no result value to publish, but still needs a visible retry action.
    ref.watch(lastOutcomeProvider);
    final outcomes = ref.read(lastOutcomeProvider.notifier);
    return ListenableBuilder(
      listenable: drafts,
      builder: (context, _) {
        final resultFailed = outcome is FreshOutcome && !outcome.persisted;
        final loadFailed =
            drafts.hasLoadFailure(pinKey) || outcomes.hasLoadFailure(pinKey.$1);
        if (!drafts.hasSaveFailure(pinKey) && !loadFailed && !resultFailed) {
          return const SizedBox.shrink();
        }
        final message = t(
          ref,
          loadFailed ? 'pin.state.load_failed' : 'pin.state.save_failed',
        );
        void retry() {
          drafts.retry(pinKey);
          if (outcomes.hasLoadFailure(pinKey.$1)) {
            outcomes.hydrateForBoard(pinKey.$1);
          }
          if (resultFailed) outcomes.retry(pinKey, toolId);
        }

        if (compact) {
          return Semantics(
            liveRegion: true,
            label: message,
            child: Tooltip(
              message: message,
              child: TextButton.icon(
                key: const Key('pin-state-retry'),
                style: TextButton.styleFrom(
                  foregroundColor: Theme.of(context).colorScheme.error,
                  padding: EdgeInsets.zero,
                  minimumSize: const Size(0, 32),
                  tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                ),
                onPressed: retry,
                icon: const Icon(Icons.warning_amber, size: 16),
                label: Text(
                  t(ref, 'pin.state.retry'),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ),
          );
        }
        return Semantics(
          liveRegion: true,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(
                message,
                key: const Key('pin-state-save-failed'),
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
              TextButton(
                key: const Key('pin-state-retry'),
                onPressed: retry,
                child: Text(t(ref, 'pin.state.retry')),
              ),
            ],
          ),
        );
      },
    );
  }
}
