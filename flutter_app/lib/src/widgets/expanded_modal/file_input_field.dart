// File input field — pick and drop converge on one assembly step.
//
//   * The `Choose file` button opens the platform dialog; the whole
//     field is the drop zone — a drag highlights border and background,
//     leaving restores them. Both paths feed
//     `file_selection_assembler.dart`, where policy (allowed extensions,
//     count, per-file and total size) is enforced, so a drop can never
//     be looser than the picker. Reads stream through a bounded window
//     and size is checked before opening.
//   * The empty state explains both paths together ("choose a file or
//     drop it here"). A `File` value travels as bytes, not a filesystem
//     path — the canonical `FileValue` JSON (`upeg_core`'s
//     `input::file_value`) — so this works unchanged in the browser
//     build too.
//   * `max_count > 1` sends the picked files as `directory` entries
//     under a single name; the shape is decided by policy, not the
//     picked count — with `max_count == 1` a single file becomes `bytes`
//     directly, with `max_count > 1` even one file is wrapped in a
//     directory.
//   * Honest gaps: real directory selection is refused — the
//     `directory` above is a synthetic multi-file container and dropping
//     a folder is rejected. `FilePath` is a different kind (a path
//     string plus a folder button, no drop zone). The Chrome extension
//     has no drop path — only the native `<input type="file">` (policy
//     checks still apply there). The field does not pre-explain policy;
//     a violation surfaces as an error after selection.
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/platform/file_picker_bridge.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/expanded_modal/file_drop_adapter.dart';
import 'package:upeg/src/widgets/expanded_modal/file_picker_selection_adapter.dart';
import 'package:upeg/src/widgets/expanded_modal/file_selection_assembler.dart';
import 'package:upeg/src/widgets/expanded_modal/form_value.dart';

part 'file_input_actions.dart';

const Duration _highlightDuration = Duration(milliseconds: 120);
const double _borderWidth = 2;
const double _dragHighlightOpacity = 0.12;

/// Catalog keys (upeg-pegboard-ui/src/i18n.rs) — copy renders through
/// `t()`/`tRead()` so the field follows the active locale.
const String _emptyPromptKey = 'modal.file.empty_prompt';
const String _selectedCountKey = 'modal.file.selected_count';
const String _pickFailureKey = 'modal.file.pick_failed';

abstract final class FileInputKeys {
  static const dropTarget = Key('file-input-drop-target');
  static const pickFocusRing = Key('file-input-pick-focus-ring');
  static const clearFocusRing = Key('file-input-clear-focus-ring');
  static const pickButton = Key('file-input-pick-button');
  static const clearButton = Key('file-input-clear-button');
}

final class FileInputField extends ConsumerStatefulWidget {
  const FileInputField({
    required this.label,
    required this.value,
    required this.policy,
    required this.pickerBridge,
    required this.onChanged,
    required this.onCleared,
    this.description,
    this.dropAdapter = const DesktopFileDropAdapter(),
    this.compact = false,
    super.key,
  });

  final String label;
  final String? description;
  final FileFormValue? value;
  final FileSelectionPolicy policy;
  final FilePickerBridge pickerBridge;
  final ValueChanged<FileFormValue> onChanged;
  final VoidCallback onCleared;
  final FileDropAdapter dropAdapter;
  final bool compact;

  @override
  ConsumerState<FileInputField> createState() => _FileInputFieldState();
}

final class _FileInputFieldState extends ConsumerState<FileInputField> {
  bool _isDragging = false;
  bool _isBusy = false;
  String? _errorMessage;

  List<String> get _selectedNames {
    final value = widget.value?.value;
    if (value == null) return const [];
    return switch (value.content) {
      CanonicalFileContent_Bytes() => [value.name],
      CanonicalFileContent_Directory(:final entries) =>
        entries.map((entry) => entry.name).toList(growable: false),
    };
  }

  Future<void> _pickFiles() async {
    if (_isBusy) return;
    _beginSelection();
    try {
      final picked = await widget.pickerBridge.pickOpenFiles(
        allowedExtensions: widget.policy.extensions.isEmpty
            ? null
            : widget.policy.extensions,
        allowMultiple: widget.policy.maxCount > 1,
        maxCount: widget.policy.maxCount,
        maxFileBytes: widget.policy.maxFileBytes,
        maxTotalBytes: widget.policy.maxTotalBytes,
      );
      if (picked == null) return;
      await _assembleAndCommit(fileSelectionCandidatesFromPicked(picked));
    } on FilePickerReadFailure catch (failure) {
      _showFailure(
        FileSelectionFailure(
          FileSelectionErrorCode.readFailed,
          fileName: failure.fileName,
        ),
      );
    } on FilePickerSelectionFailure catch (failure) {
      _showFailure(fileSelectionFailureFromPicker(failure));
    } on FileSelectionFailure catch (failure) {
      _showFailure(failure);
    } on Exception {
      _showError(tRead(ref, _pickFailureKey));
    } finally {
      _finishSelection();
    }
  }

  void _startPicking() {
    unawaited(
      _pickFiles().catchError((Object error, StackTrace stackTrace) {
        if (error is! Error) {
          Error.throwWithStackTrace(error, stackTrace);
        }
        // Material button callbacks cannot await a Future. Forward programmer
        // Errors unchanged to Flutter's error boundary instead of user feedback.
        FlutterError.reportError(
          FlutterErrorDetails(
            exception: error,
            stack: stackTrace,
            library: 'upeg file input',
            context: ErrorDescription('while handling a file pick'),
          ),
        );
      }),
    );
  }

  Future<void> _dropFiles(List<FileSelectionCandidate> candidates) async {
    if (_isBusy) return;
    _beginSelection();
    try {
      await _assembleAndCommit(candidates);
    } on FileSelectionFailure catch (failure) {
      _showFailure(failure);
    } on Exception {
      _showError(tRead(ref, _pickFailureKey));
    } finally {
      _finishSelection();
    }
  }

  Future<void> _assembleAndCommit(
    List<FileSelectionCandidate> candidates,
  ) async {
    final assembled = await FileSelectionAssembler(
      widget.policy,
    ).assemble(candidates);
    if (!mounted) return;
    widget.onChanged(FileFormValue(assembled));
  }

  void _beginSelection() {
    setState(() {
      _isBusy = true;
      _isDragging = false;
      _errorMessage = null;
    });
  }

  void _finishSelection() {
    if (!mounted) return;
    setState(() => _isBusy = false);
  }

  void _showError(String message) {
    if (!mounted) return;
    setState(() => _errorMessage = message);
  }

  /// Localized presentation of a typed [FileSelectionFailure] — the
  /// failure carries the code + structured fields, this resolves them
  /// against the active locale.
  void _showFailure(FileSelectionFailure failure) {
    _showError(tRead(ref, failure.messageKey, failure.messageArgs));
  }

  void _setDragging(bool value) {
    if (_isBusy || _isDragging == value) return;
    setState(() => _isDragging = value);
  }

  void _clear() {
    if (_isBusy) return;
    setState(() => _errorMessage = null);
    widget.onCleared();
  }

  @override
  Widget build(BuildContext context) {
    final names = _selectedNames;
    final theme = Theme.of(context);
    final tokens = context.upeg;
    final borderColor = _isDragging ? tokens.accent : tokens.line;
    final content = AnimatedContainer(
      key: FileInputKeys.dropTarget,
      duration: _highlightDuration,
      padding: widget.compact
          ? const EdgeInsets.all(UpegSizing.pinGap)
          : UpegSizing.pinBodyPadding,
      decoration: BoxDecoration(
        color: _isDragging
            ? tokens.accent.withValues(alpha: _dragHighlightOpacity)
            : tokens.surface2,
        border: Border.all(color: borderColor, width: _borderWidth),
        borderRadius: BorderRadius.circular(UpegSizing.radius2),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(widget.label, style: theme.textTheme.labelMedium),
          // A drop target has no InputDecoration, so the declared
          // description gets its own line under the label instead of
          // the helperText slot every other field kind uses. A compact
          // tile has no room for it (mirrors `_multiOptionsField`).
          if (widget.description case final description?
              when !widget.compact && description.isNotEmpty) ...[
            const SizedBox(height: UpegSizing.radius1),
            Text(
              description,
              style: theme.textTheme.labelSmall?.copyWith(color: tokens.fg3),
            ),
          ],
          const SizedBox(height: UpegSizing.radius2),
          Text(
            names.isEmpty
                ? t(ref, _emptyPromptKey)
                : t(ref, _selectedCountKey, {'count': '${names.length}'}),
          ),
          if (names.isNotEmpty) ...[
            const SizedBox(height: UpegSizing.radius1),
            for (final name in names)
              Text(
                name,
                maxLines: 1,
                softWrap: false,
                overflow: TextOverflow.ellipsis,
              ),
          ],
          const SizedBox(height: UpegSizing.radius2),
          _FileInputActions(
            isBusy: _isBusy,
            hasSelection: names.isNotEmpty,
            onPick: _startPicking,
            onClear: _clear,
          ),
          if (_errorMessage case final message?) ...[
            const SizedBox(height: UpegSizing.radius1),
            Semantics(
              container: true,
              liveRegion: true,
              label: message,
              child: ExcludeSemantics(
                child: DecoratedBox(
                  decoration: BoxDecoration(
                    color: tokens.warn,
                    borderRadius: BorderRadius.circular(UpegSizing.radius1),
                  ),
                  child: Padding(
                    padding: const EdgeInsets.symmetric(
                      horizontal: UpegSizing.radius2,
                      vertical: UpegSizing.radius1,
                    ),
                    child: Text(
                      message,
                      style: theme.textTheme.labelSmall?.copyWith(
                        color: tokens.onWarn,
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ],
        ],
      ),
    );
    return widget.dropAdapter.wrap(
      child: content,
      callbacks: FileDropCallbacks(
        onEntered: () => _setDragging(true),
        onExited: () => _setDragging(false),
        onDropped: _dropFiles,
      ),
    );
  }
}
