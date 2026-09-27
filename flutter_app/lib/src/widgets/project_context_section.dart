/// Settings section for selecting the active `.upeg` project directory.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/keyboard/keyboard_command_resolver.dart';
import 'package:upeg/src/platform/file_picker_bridge.dart';
import 'package:upeg/src/state/project_context_provider.dart';
import 'package:upeg/src/state/running_tools_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';

const Key projectBrowseButtonKey = Key('project-context-browse');
const Key projectActivateButtonKey = Key('project-context-activate');
const Key projectCloseButtonKey = Key('project-context-close');
const Key projectRootInputFocusKey = Key('project-context-root-input-focus');

class ProjectContextSection extends ConsumerStatefulWidget {
  const ProjectContextSection({super.key});

  @override
  ConsumerState<ProjectContextSection> createState() =>
      _ProjectContextSectionState();
}

class _ProjectContextSectionState extends ConsumerState<ProjectContextSection> {
  final TextEditingController _root = TextEditingController();
  bool _busy = false;
  String? _error;

  @override
  void dispose() {
    _root.dispose();
    super.dispose();
  }

  Future<void> _browse() async {
    final picked = await ref
        .read(filePickerBridgeProvider)
        .pickDirectoryPath(dialogTitle: tRead(ref, 'project.browse_title'));
    if (picked != null && mounted) setState(() => _root.text = picked);
  }

  Future<void> _activate() async {
    final root = _root.text.trim();
    if (root.isEmpty) {
      setState(() => _error = tRead(ref, 'project.choose_first'));
      return;
    }
    if (_runsAreActive()) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      final definition = await ref
          .read(projectContextProvider.notifier)
          .validateRoot(root);
      if (!mounted) return;
      final choices = await _chooseConflicts(definition);
      if (choices == null || !mounted) return;
      for (final entry in choices.entries) {
        await ref
            .read(projectContextProvider.notifier)
            .setToolChoice(root: root, toolId: entry.key, choice: entry.value);
      }
      await ref.read(projectContextProvider.notifier).activate(root);
    } on Object catch (error) {
      if (mounted) {
        setState(
          () => _error = tRead(ref, 'project.operation_failed', {
            'msg': '$error',
          }),
        );
      }
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _close() async {
    if (_runsAreActive()) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      await ref.read(projectContextProvider.notifier).close();
      if (mounted) _root.clear();
    } on Object catch (error) {
      if (mounted) {
        setState(
          () => _error = tRead(ref, 'project.operation_failed', {
            'msg': '$error',
          }),
        );
      }
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  bool _runsAreActive() {
    if (ref.read(runningToolsProvider).isEmpty) return false;
    setState(() {
      _error = tRead(ref, 'project.running_tools');
    });
    return true;
  }

  /// The settings dialog also owns keyboard navigation. A path character such
  /// as `/` must finish at this editable field instead of bubbling into that
  /// command resolver and opening the palette. Text editing still receives
  /// its platform text-input update first; this only consumes the later key
  /// event bubble. Explicit global chords remain available.
  KeyEventResult _handleRootInputKey(FocusNode node, KeyEvent event) {
    if (!isKeyboardPress(event) || isGlobalShortcutKeyEvent(event)) {
      return KeyEventResult.ignored;
    }
    return KeyEventResult.skipRemainingHandlers;
  }

  Future<Map<String, ProjectToolChoice>?> _chooseConflicts(
    ProjectDefinition definition,
  ) async {
    final unresolved = definition.conflicts.where((item) => item.needsChoice);
    if (unresolved.isEmpty) return const {};
    return showDialog<Map<String, ProjectToolChoice>>(
      context: context,
      builder: (_) => _ToolChoiceDialog(conflicts: unresolved.toList()),
    );
  }

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    final project = switch (ref.watch(projectContextProvider)) {
      AsyncData(:final value) => value,
      _ => null,
    };
    final activeRoot = project?.root;
    if (_root.text.isEmpty && activeRoot != null) _root.text = activeRoot;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          activeRoot == null
              ? t(ref, 'project.global_context')
              : t(ref, 'project.active_root', {'root': activeRoot}),
          key: const Key('project-context-active-root'),
          style: TextStyle(color: tokens.fg2, fontSize: 11),
        ),
        if (project != null) ...[
          const SizedBox(height: 4),
          Text(
            t(ref, 'project.summary', {
              'name': project.name,
              'boards': '${project.boardCount}',
              'toolkits': '${project.toolkits.length}',
            }),
            style: TextStyle(color: tokens.fg3, fontSize: 10),
          ),
        ],
        const SizedBox(height: 8),
        Focus(
          key: projectRootInputFocusKey,
          onKeyEvent: _handleRootInputKey,
          child: TextField(
            key: const Key('project-context-root-input'),
            controller: _root,
            enabled: !_busy,
            decoration: InputDecoration(
              labelText: t(ref, 'project.folder_label'),
              border: const OutlineInputBorder(),
            ),
          ),
        ),
        const SizedBox(height: 6),
        Wrap(
          spacing: 6,
          children: [
            OutlinedButton.icon(
              key: projectBrowseButtonKey,
              onPressed: _busy ? null : _browse,
              icon: const Icon(Icons.folder_open, size: 16),
              label: Text(t(ref, 'project.browse')),
            ),
            FilledButton(
              key: projectActivateButtonKey,
              onPressed: _busy ? null : _activate,
              child: Text(
                t(ref, activeRoot == null ? 'project.open' : 'project.switch'),
              ),
            ),
            if (activeRoot != null)
              OutlinedButton(
                key: projectCloseButtonKey,
                onPressed: _busy ? null : _close,
                child: Text(t(ref, 'project.close')),
              ),
          ],
        ),
        if (_error case final error?) ...[
          const SizedBox(height: 6),
          Text(
            error,
            key: const Key('project-context-error'),
            style: TextStyle(color: tokens.warn, fontSize: 10),
          ),
        ],
      ],
    );
  }
}

class _ToolChoiceDialog extends ConsumerStatefulWidget {
  const _ToolChoiceDialog({required this.conflicts});
  final List<ProjectToolConflict> conflicts;

  @override
  ConsumerState<_ToolChoiceDialog> createState() => _ToolChoiceDialogState();
}

class _ToolChoiceDialogState extends ConsumerState<_ToolChoiceDialog> {
  late final Map<String, ProjectToolChoice> _choices = {
    for (final conflict in widget.conflicts)
      conflict.toolId: ProjectToolChoice.global,
  };

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: Text(t(ref, 'project.duplicate_title')),
      content: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 420),
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(t(ref, 'project.duplicate_help')),
              for (final conflict in widget.conflicts)
                RadioGroup<ProjectToolChoice>(
                  groupValue: _choices[conflict.toolId],
                  onChanged: (value) {
                    if (value != null) {
                      setState(() => _choices[conflict.toolId] = value);
                    }
                  },
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      const SizedBox(height: 12),
                      Text(conflict.toolId),
                      RadioListTile<ProjectToolChoice>(
                        value: ProjectToolChoice.global,
                        title: Text(
                          t(ref, 'project.global_source', {
                            'source': conflict.globalSource,
                          }),
                        ),
                      ),
                      RadioListTile<ProjectToolChoice>(
                        value: ProjectToolChoice.project,
                        title: Text(
                          t(ref, 'project.project_source', {
                            'source': conflict.projectSource,
                          }),
                        ),
                      ),
                    ],
                  ),
                ),
            ],
          ),
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(t(ref, 'project.cancel')),
        ),
        FilledButton(
          onPressed: () => Navigator.of(context).pop(_choices),
          child: Text(t(ref, 'project.continue')),
        ),
      ],
    );
  }
}
