part of '../../pages/expanded_modal_page.dart';

/// Render the modal shell; editing and dispatch state stay with the page.
Widget _buildModalFrame(_ExpandedModalPageState state, BuildContext context) {
  final ref = state.ref;
  final widget = state.widget;
  // Presentation actions resolve a statically declared target synchronously
  // on tap. Warm the cached catalogue while the form is visible so the
  // first action cannot observe the FutureProvider's loading frame.
  ref.watch(toolsProvider);
  final tokens = context.upeg;
  final tool = widget.tool;
  final toolId = ToolId.parse(tool.id);
  final bespoke = bespokeFor(toolId);
  final readinessAllowsRun = tool.invoker != InvokerDto.external_
      ? true
      : externalReadinessAllowsRun(
          ref.watch(
            externalReadinessProvider(
              readinessTargetFor(
                tool,
                boardKey: ref.watch(currentBoardKeyProvider),
              ),
            ),
          ),
        );
  final pin = state._editingPin;
  if (pin != null) {
    ref.listen(pinLastOutcomeProvider(pin), (_, next) {
      if (state._publishingOutcome || state._editingPin != pin) return;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (state._editingPin == pin) state._adoptOutcome(next);
      });
    });
  }
  final inputForm = Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      if (bespoke != null)
        KeyedSubtree(
          key: ValueKey(state._formRevision),
          child: bespoke(
            context,
            tool,
            state._handleBespokeSubmit,
            state._outcome?.primaryOutputText,
            state._bespokeInput,
            onInputChanged: state._handleBespokeInputChanged,
          ),
        )
      else ...[
        GenericFormWidget(
          key: ValueKey(state._formRevision),
          tool: tool,
          controller: state._controller,
          boundInputKeys: widget.collapseBoundInputs
              ? widget.presentationAction?.bindings
                        .map((binding) => binding.target)
                        .toSet() ??
                    const {}
              : const {},
          onValidationChanged: state._handleFormValidation,
        ),
        const SizedBox(height: 8),
        Align(
          alignment: Alignment.centerRight,
          child: ExpandedModalPrimaryButton(
            key: const Key('expanded-modal-run-btn'),
            label: t(ref, 'modal.action.run_short'),
            trailingHint: '[F1]',
            // Toolless tools (no required fields)
            // can always Run; otherwise gate on the
            // aggregate validation result. A request
            // in flight also disables Run so a slow
            // tool can't be double-fired.
            enabled:
                (tool.inputFields.isEmpty || state._formAllOk) &&
                readinessAllowsRun &&
                !state._phase.blocksRun &&
                !state._pinPending &&
                !state._draftLoadFailed,
            loading: state._phase.showsSpinner,
            onPressed: state._handleRun,
          ),
        ),
      ],
    ],
  );
  return Scaffold(
    backgroundColor: Colors.transparent,
    body: Shortcuts(
      shortcuts: const <ShortcutActivator, Intent>{
        SingleActivator(LogicalKeyboardKey.f1): _RunIntent(),
        SingleActivator(LogicalKeyboardKey.enter): _RunIntent(),
        SingleActivator(LogicalKeyboardKey.f2): _CopyIntent(),
        SingleActivator(LogicalKeyboardKey.escape): _CloseIntent(),
        SingleActivator(LogicalKeyboardKey.f3, includeRepeats: false):
            _PinIntent(),
      },
      child: Actions(
        actions: <Type, Action<Intent>>{
          _CopyIntent: CallbackAction<_CopyIntent>(
            onInvoke: (_) {
              state._handleCopy();
              return null;
            },
          ),
          _RunIntent: CallbackAction<_RunIntent>(
            onInvoke: (_) {
              if ((tool.inputFields.isEmpty || state._formAllOk) &&
                  readinessAllowsRun) {
                state._handleRun();
              }
              return null;
            },
          ),
          _CloseIntent: CallbackAction<_CloseIntent>(
            onInvoke: (_) {
              Navigator.of(context).pop();
              return null;
            },
          ),
          _PinIntent: CallbackAction<_PinIntent>(
            onInvoke: (_) {
              // The action returns synchronously; the placement write awaits
              // completion before selecting the new editing pin.
              unawaited(state._handlePin());
              return null;
            },
          ),
        },
        child: Focus(
          autofocus: true,
          onKeyEvent: (_, event) => state._handleAliasKey(event)
              ? KeyEventResult.handled
              : KeyEventResult.ignored,
          child: SafeArea(
            child: LayoutBuilder(
              builder: (context, constraints) {
                // A resolved collection needs enough room for its list and
                // selected detail. Plain forms retain the compact dialog.
                final presentationWidth = tool.presentation == null
                    ? 560.0
                    : 1000.0;
                final presentationHeight = tool.presentation == null
                    ? 640.0
                    : 720.0;
                return Center(
                  child: ConstrainedBox(
                    constraints: BoxConstraints(
                      maxWidth: presentationWidth,
                      maxHeight: presentationHeight,
                    ),
                    child: Container(
                      margin: const EdgeInsets.all(16),
                      decoration: BoxDecoration(
                        color: tokens.surface,
                        border: Border.all(color: tokens.line),
                        borderRadius: BorderRadius.circular(8),
                        boxShadow: const [
                          BoxShadow(
                            color: Color(0x66000000),
                            blurRadius: 60,
                            offset: Offset(0, 20),
                          ),
                        ],
                      ),
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          ExpandedModalHeader(tokens: tokens, tool: tool),
                          Flexible(
                            child: SingleChildScrollView(
                              padding: const EdgeInsets.fromLTRB(
                                16,
                                14,
                                16,
                                14,
                              ),
                              child: Column(
                                crossAxisAlignment: CrossAxisAlignment.stretch,
                                children: [
                                  if (tool.description.isNotEmpty &&
                                      !isImageConversionTool(tool.id))
                                    Padding(
                                      padding: const EdgeInsets.only(
                                        bottom: 12,
                                      ),
                                      child: Text(
                                        tool.description,
                                        style: TextStyle(
                                          color: tokens.fg3,
                                          fontSize: 11,
                                          height: 1.45,
                                        ),
                                      ),
                                    ),
                                  ExternalReadinessPanel(tool: tool),
                                  FocusScope(
                                    descendantsAreFocusable: !state._pinPending,
                                    child: AbsorbPointer(
                                      absorbing: state._pinPending,
                                      child: PinDraftEditor(
                                        pinKey: pin,
                                        child: inputForm,
                                      ),
                                    ),
                                  ),
                                  if (state._phase.showsSpinner) ...[
                                    const SizedBox(height: 12),
                                    _LiveRunBlock(
                                      tail: state._tail,
                                      tokens: tokens,
                                      onCancel: state._runId == null
                                          ? null
                                          : state._handleCancel,
                                      cancelRequested: state._cancelRequested,
                                    ),
                                  ],
                                  if (pin != null)
                                    PinSaveStatus(pinKey: pin, toolId: toolId),
                                  if (state._outcome != null) ...[
                                    const SizedBox(height: 14),
                                    OutcomeBlock(
                                      outcome: state._outcome!,
                                      stale: state._outcomeStale,
                                      inputUnknown: state._outcomeInputUnknown,
                                      restored: state._restoredOutcome,
                                      truncated: state._truncatedOutcome,
                                      outputFields: tool.outputFields,
                                      tool: tool,
                                      tokens: tokens,
                                      onRowAction:
                                          state._handlePresentationAction,
                                      onResultAction: (action) =>
                                          state._handlePresentationAction(
                                            action,
                                            null,
                                          ),
                                      onReadRowNavigate: (action, row) =>
                                          state._handlePresentationAction(
                                            action,
                                            row,
                                            singleClick: true,
                                          ),
                                    ),
                                    if (!state._outcome!.ok &&
                                        state._outcomeRunId != null)
                                      DiagnosticRunButton(
                                        runId: state._outcomeRunId!.value,
                                      ),
                                  ],
                                ],
                              ),
                            ),
                          ),
                          ExpandedModalFooter(
                            tokens: tokens,
                            tool: tool,
                            onPin: () => unawaited(state._handlePin()),
                            pinEnabled:
                                !state._phase.blocksRun && !state._pinPending,
                            editingPin: pin != null,
                          ),
                        ],
                      ),
                    ),
                  ),
                );
              },
            ),
          ),
        ),
      ),
    ),
  );
}

/// Project reset removes this modal even when Settings is stacked above it.
void _closeModalRoute(BuildContext context) {
  final route = ModalRoute.of(context);
  WidgetsBinding.instance.addPostFrameCallback((_) {
    if (context.mounted && route != null && route.isActive) {
      Navigator.of(context).removeRoute(route);
    }
  });
}
