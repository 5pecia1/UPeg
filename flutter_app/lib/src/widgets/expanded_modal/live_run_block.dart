part of '../../pages/expanded_modal_page.dart';

/// The cancellable progress strip used only while the modal dispatches.
class _LiveRunBlock extends ConsumerWidget {
  const _LiveRunBlock({
    required this.tail,
    required this.tokens,
    required this.onCancel,
  });

  final LiveTail tail;
  final UpegTokens tokens;
  final VoidCallback onCancel;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Container(
      key: modalLiveOutputTailKey,
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: tokens.bg2,
        border: Border.all(color: tokens.line),
        borderRadius: BorderRadius.circular(UpegSizing.radius2),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          Row(
            children: [
              Flexible(
                child: Text(
                  t(ref, 'modal.pill.live_output'),
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    fontFamily: upegMonoFontFamily,
                    fontFamilyFallback: upegMonoFontFamilyFallback,
                    fontSize: 10,
                    letterSpacing: 0.4,
                    color: tokens.fg3,
                  ),
                ),
              ),
              const Spacer(),
              TextButton(
                key: const Key('expanded-modal-cancel-btn'),
                style: TextButton.styleFrom(
                  foregroundColor: tokens.warn,
                  padding: const EdgeInsets.symmetric(
                    horizontal: 8,
                    vertical: 4,
                  ),
                  minimumSize: const Size(0, 24),
                  textStyle: const TextStyle(
                    fontSize: 11,
                    fontWeight: FontWeight.w500,
                  ),
                ),
                onPressed: onCancel,
                child: Text(t(ref, 'modal.action.cancel_run')),
              ),
            ],
          ),
          if (tail.isNotEmpty) ...[
            const SizedBox(height: 6),
            LiveOutputTailView(tail: tail, fontSize: 11),
          ],
        ],
      ),
    );
  }
}
