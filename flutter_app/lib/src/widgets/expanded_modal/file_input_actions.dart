part of 'file_input_field.dart';

final class _FileInputActions extends ConsumerStatefulWidget {
  const _FileInputActions({
    required this.isBusy,
    required this.hasSelection,
    required this.onPick,
    required this.onClear,
  });

  final bool isBusy;
  final bool hasSelection;
  final VoidCallback onPick;
  final VoidCallback onClear;

  @override
  ConsumerState<_FileInputActions> createState() => _FileInputActionsState();
}

final class _FileInputActionsState extends ConsumerState<_FileInputActions> {
  final FocusNode _pickFocusNode = FocusNode();
  final FocusNode _clearFocusNode = FocusNode();
  bool _isPickFocused = false;
  bool _isClearFocused = false;

  @override
  void initState() {
    super.initState();
    _pickFocusNode.addListener(_handlePickFocusChanged);
    _clearFocusNode.addListener(_handleClearFocusChanged);
  }

  void _handlePickFocusChanged() {
    final value = _pickFocusNode.hasFocus;
    if (_isPickFocused == value) return;
    setState(() => _isPickFocused = value);
  }

  void _handleClearFocusChanged() {
    final value = _clearFocusNode.hasFocus;
    if (_isClearFocused == value) return;
    setState(() => _isClearFocused = value);
  }

  @override
  void dispose() {
    _pickFocusNode.removeListener(_handlePickFocusChanged);
    _clearFocusNode.removeListener(_handleClearFocusChanged);
    _pickFocusNode.dispose();
    _clearFocusNode.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final tokens = context.upeg;
    final pickLabel = t(ref, 'modal.generic.file_pick');
    return Wrap(
      spacing: UpegSizing.radius1,
      runSpacing: UpegSizing.radius1,
      children: [
        Semantics(
          button: true,
          label: pickLabel,
          child: DecoratedBox(
            key: FileInputKeys.pickFocusRing,
            decoration: BoxDecoration(
              border: Border.all(
                color: _isPickFocused ? tokens.focusRing : Colors.transparent,
                width: _borderWidth,
              ),
              borderRadius: BorderRadius.circular(UpegSizing.radius3),
            ),
            child: Padding(
              padding: const EdgeInsets.all(UpegSizing.radius1 / 2),
              child: FilledButton.icon(
                key: FileInputKeys.pickButton,
                focusNode: _pickFocusNode,
                onPressed: widget.isBusy ? null : widget.onPick,
                style: FilledButton.styleFrom(
                  backgroundColor: tokens.accent,
                  foregroundColor: tokens.onAccent,
                  overlayColor: upegHighContrastForeground(tokens.onAccent),
                  disabledBackgroundColor: tokens.lineSoft,
                  disabledForegroundColor: tokens.fg2,
                  minimumSize: const Size(0, kMinInteractiveDimension),
                  tapTargetSize: MaterialTapTargetSize.padded,
                  padding: const EdgeInsets.symmetric(
                    horizontal: UpegSizing.radius2 * 2,
                  ),
                  textStyle: theme.textTheme.labelSmall,
                ),
                icon: widget.isBusy
                    ? const SizedBox.square(
                        dimension: 16,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(Icons.attach_file),
                label: Text(pickLabel),
              ),
            ),
          ),
        ),
        if (widget.hasSelection)
          DecoratedBox(
            key: FileInputKeys.clearFocusRing,
            decoration: BoxDecoration(
              border: Border.all(
                color: _isClearFocused ? tokens.focusRing : Colors.transparent,
                width: _borderWidth,
              ),
              shape: BoxShape.circle,
            ),
            child: Padding(
              padding: const EdgeInsets.all(UpegSizing.radius1 / 2),
              child: IconButton(
                key: FileInputKeys.clearButton,
                focusNode: _clearFocusNode,
                tooltip: t(ref, 'modal.generic.file_clear'),
                onPressed: widget.isBusy ? null : widget.onClear,
                color: tokens.fg2,
                icon: const Icon(Icons.clear),
              ),
            ),
          ),
      ],
    );
  }
}
