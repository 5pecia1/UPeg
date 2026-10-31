part of '../../pages/expanded_modal_page.dart';

/// Batch O2 (I13 F3): modal-wide shortcut intent for "+ pin". Routed
/// through `Shortcuts`/`Actions` so bespoke forms (which bind F1+F2
/// only) let F3 bubble up to the modal shell.
class _PinIntent extends Intent {
  const _PinIntent();
}

class _CopyIntent extends Intent {
  const _CopyIntent();
}

class _RunIntent extends Intent {
  const _RunIntent();
}

class _CloseIntent extends Intent {
  const _CloseIntent();
}

/// F2 copies the visible result text; diagnostic reports have their own
/// named Copy action in the report dialog.
Future<void> _copyModalOutcome(_ExpandedModalPageState state) async {
  final ref = state.ref;
  final context = state.context;
  final writer = ref.read(clipboardWriterProvider);
  final outcome = state._outcome;
  final copyText = outcome?.ok == false
      ? outcome?.errorMessage
      : copyableOutcomeText(outcome);
  if (!context.mounted) return;
  if (copyText == null) {
    final messenger = ScaffoldMessenger.of(context);
    messenger.clearSnackBars();
    messenger.removeCurrentSnackBar();
    messenger.showSnackBar(
      SnackBar(content: Text(tRead(ref, 'clipboard.no_text_result'))),
    );
    return;
  }
  await copyWithFeedback(
    context: context,
    ref: ref,
    write: () => writer.write(copyText),
    successKey: copySuccessKeyForRelation(
      state._outcome == outcome
          ? state._outcomeInputRelation
          : ResultInputRelation.unknown,
    ),
  );
}

extension _ModalPinAction on _ExpandedModalPageState {
  Future<void> _handlePin() async {
    if (_projectChanged || _phase.blocksRun || _pinPending) return;
    FocusScope.of(context).requestFocus();
    _updatePinState(() => _pinPending = true);
    try {
      final copiedFromExistingPin = _editingPin != null;
      final args = _currentInput;
      final result = _outcome;
      final resultArgs = _outcomeArgs;
      final recorder = ref.read(lastOutcomeProvider.notifier);
      final toolId = ToolId.parse(widget.tool.id);
      final pin = await pinToolFromModal(
        context: context,
        ref: ref,
        tool: widget.tool,
        notifySuccess: false,
      );
      if (pin == null || _projectChanged) return;
      _drafts.set(pin, args, toolId: toolId);
      if (result != null) {
        recorder.record(pin, toolId, result, args: resultArgs);
      }
      if (!mounted) return;
      _updatePinState(() => _editingPin = pin);
      ref.read(focusedPinProvider.notifier).focus(pin.$2);
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            tRead(
              ref,
              copiedFromExistingPin
                  ? result == null
                        ? 'modal.pin.input_copied_and_editing'
                        : 'modal.pin.copied_and_editing'
                  : 'modal.pin.added_and_editing',
            ),
          ),
        ),
      );
    } finally {
      _updatePinState(() => _pinPending = false);
    }
  }
}
