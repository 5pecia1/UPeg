import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/rust/api/board_details.dart';
import 'package:upeg/src/rust/api/boot.dart'
    show FrbError_ProjectManifestChanged;
import 'package:upeg/src/state/board_details_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/board_details_keys.dart';
import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';

const double _dialogWidth = 680;
const double _sectionGap = 20;

Future<void> showBoardDetailsDialog(BuildContext context, String boardKey) {
  return showDialog<void>(
    context: context,
    builder: (_) => BoardDetailsDialog(boardKey: boardKey),
  );
}

class BoardDetailsDialog extends ConsumerWidget {
  const BoardDetailsDialog({required this.boardKey, super.key});

  final String boardKey;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final details = ref.watch(boardDetailsProvider(boardKey));
    return AlertDialog(
      key: BoardDetailsKeys.dialog,
      title: Text(t(ref, 'board.details.title')),
      content: SizedBox(
        width: _dialogWidth,
        child: details.when(
          loading: () => const Center(child: CircularProgressIndicator()),
          error: (error, _) => Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(switch (error) {
                FrbError_ProjectManifestChanged(:final path) => t(
                  ref,
                  'board.details.project_changed',
                  {'path': path},
                ),
                _ => t(ref, 'board.details.load_failed', {'message': '$error'}),
              }),
              TextButton(
                onPressed: () => ref.invalidate(boardDetailsProvider(boardKey)),
                child: Text(t(ref, 'board.details.retry')),
              ),
            ],
          ),
          data: (data) => _BoardDetailsBody(details: data),
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(t(ref, 'board.details.close')),
        ),
      ],
    );
  }
}

class _BoardDetailsBody extends ConsumerStatefulWidget {
  const _BoardDetailsBody({required this.details});

  final BoardDetailsDto details;

  @override
  ConsumerState<_BoardDetailsBody> createState() => _BoardDetailsBodyState();
}

class _BoardDetailsBodyState extends ConsumerState<_BoardDetailsBody> {
  late final TextEditingController _description;
  late final TextEditingController _instructions;
  bool _saving = false;
  bool _saved = false;
  Object? _saveError;
  bool _connectionExpanded = false;

  @override
  void initState() {
    super.initState();
    _description = TextEditingController(text: widget.details.description);
    _instructions = TextEditingController(text: widget.details.instructions);
  }

  @override
  void dispose() {
    _description.dispose();
    _instructions.dispose();
    super.dispose();
  }

  Future<void> _save() async {
    setState(() {
      _saving = true;
      _saved = false;
      _saveError = null;
    });
    try {
      await ref.read(boardGuidanceSaverProvider)(
        widget.details.boardKey,
        _description.text,
        _instructions.text,
      );
      if (!mounted) return;
      ref.invalidate(boardConnectionProvider(widget.details.boardKey));
      setState(() => _saved = true);
    } catch (error) {
      if (mounted) setState(() => _saveError = error);
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  void _markEdited(String _) {
    if (_saved || _saveError != null) {
      setState(() {
        _saved = false;
        _saveError = null;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final details = widget.details;
    final projectManifest = details.projectManifestPath;
    final readOnly = projectManifest != null;
    return SingleChildScrollView(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(details.title, style: Theme.of(context).textTheme.titleMedium),
          SelectableText(details.boardKey),
          const SizedBox(height: _sectionGap),
          Text(t(ref, 'board.details.prepare')),
          const SizedBox(height: _sectionGap),
          if (projectManifest != null) ...[
            Text(t(ref, 'board.details.project_owned')),
            SelectableText(
              projectManifest,
              key: BoardDetailsKeys.projectSource,
            ),
            const SizedBox(height: _sectionGap),
          ],
          TextField(
            key: BoardDetailsKeys.description,
            controller: _description,
            readOnly: readOnly || _saving,
            minLines: 1,
            maxLines: 3,
            onChanged: _markEdited,
            decoration: InputDecoration(
              labelText: t(ref, 'board.details.description'),
            ),
          ),
          const SizedBox(height: _sectionGap),
          TextField(
            key: BoardDetailsKeys.instructions,
            controller: _instructions,
            readOnly: readOnly || _saving,
            minLines: 5,
            maxLines: 14,
            onChanged: _markEdited,
            decoration: InputDecoration(
              labelText: t(ref, 'board.details.instructions'),
              alignLabelWithHint: true,
            ),
          ),
          const SizedBox(height: 8),
          Text(t(ref, 'board.details.optional')),
          if (!readOnly) ...[
            const SizedBox(height: 12),
            Align(
              alignment: Alignment.centerLeft,
              child: FilledButton(
                key: BoardDetailsKeys.save,
                onPressed: _saving ? null : _save,
                child: Text(
                  t(
                    ref,
                    _saving ? 'board.details.saving' : 'board.details.save',
                  ),
                ),
              ),
            ),
          ],
          if (_saved)
            Text(
              t(
                ref,
                details.nativeConnectionSupported
                    ? 'board.details.saved'
                    : 'board.details.saved_browser',
              ),
              key: BoardDetailsKeys.saved,
            ),
          if (_saveError != null)
            Text(
              t(ref, 'board.details.save_failed', {'message': '$_saveError'}),
              key: BoardDetailsKeys.saveError,
              style: TextStyle(color: context.upeg.warn),
            ),
          const SizedBox(height: _sectionGap),
          if (!details.nativeConnectionSupported)
            Text(
              t(ref, 'board.connection.web_unavailable'),
              key: BoardDetailsKeys.webNotice,
            )
          else ...[
            const Divider(),
            Text(t(ref, 'board.connection.directory')),
            SelectableText(details.executionDirectory ?? ''),
            const SizedBox(height: 8),
            Text(t(ref, 'board.connection.directory_help')),
            const SizedBox(height: 12),
            OutlinedButton.icon(
              key: BoardDetailsKeys.connection,
              icon: Icon(_connectionExpanded ? Icons.expand_less : Icons.link),
              onPressed: () =>
                  setState(() => _connectionExpanded = !_connectionExpanded),
              label: Text(t(ref, 'board.connection.show')),
            ),
            if (_connectionExpanded)
              _BoardConnectionPanel(boardKey: details.boardKey),
          ],
        ],
      ),
    );
  }
}

class _BoardConnectionPanel extends ConsumerStatefulWidget {
  const _BoardConnectionPanel({required this.boardKey});
  final String boardKey;

  @override
  ConsumerState<_BoardConnectionPanel> createState() =>
      _BoardConnectionPanelState();
}

class _BoardConnectionPanelState extends ConsumerState<_BoardConnectionPanel> {
  bool _copied = false;
  Object? _copyError;

  Future<void> _copy(String config) async {
    try {
      await ref.read(clipboardWriterProvider).write(config);
      if (mounted) {
        setState(() {
          _copied = true;
          _copyError = null;
        });
      }
    } catch (error) {
      if (mounted) {
        setState(() {
          _copied = false;
          _copyError = error;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final preview = ref.watch(boardConnectionProvider(widget.boardKey));
    return preview.when(
      loading: () => const LinearProgressIndicator(),
      error: (error, _) => Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(t(ref, 'board.connection.load_failed', {'message': '$error'})),
          TextButton(
            onPressed: () =>
                ref.invalidate(boardConnectionProvider(widget.boardKey)),
            child: Text(t(ref, 'board.details.retry')),
          ),
        ],
      ),
      data: (data) => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          const SizedBox(height: 12),
          Text(t(ref, 'board.connection.steps')),
          if (data.readiness == BoardToolReadinessDto.unavailable)
            Text(
              t(ref, 'board.connection.cli_unavailable'),
              style: TextStyle(color: context.upeg.warn),
            ),
          for (final reason in data.readinessReasons) Text(reason),
          const SizedBox(height: 12),
          if (data.projectManifestPath case final manifest?) ...[
            Text(t(ref, 'board.connection.project')),
            SelectableText(manifest),
          ],
          Text(
            t(ref, 'board.connection.tools', {'count': '${data.tools.length}'}),
          ),
          if (data.tools.isEmpty)
            Text(
              t(
                ref,
                data.readiness == BoardToolReadinessDto.ready
                    ? 'board.connection.no_tools'
                    : 'board.connection.incomplete_tools',
              ),
            ),
          for (final tool in data.tools) _ToolPreview(tool: tool),
          const SizedBox(height: 12),
          Text(t(ref, 'board.connection.config')),
          SelectableText(
            data.configJson,
            style: const TextStyle(
              fontFamily: upegMonoFontFamily,
              fontSize: 12,
            ),
          ),
          const SizedBox(height: 8),
          OutlinedButton.icon(
            key: BoardDetailsKeys.copy,
            icon: const Icon(Icons.copy_outlined),
            onPressed: () => _copy(data.configJson),
            label: Text(t(ref, 'board.connection.copy')),
          ),
          if (_copied)
            Text(
              t(ref, 'board.connection.copied'),
              key: BoardDetailsKeys.copied,
            ),
          if (_copyError != null)
            Text(
              t(ref, 'board.connection.copy_failed', {
                'message': '$_copyError',
              }),
            ),
          const SizedBox(height: 12),
          Text(t(ref, 'board.connection.reconnect')),
        ],
      ),
    );
  }
}

class _ToolPreview extends ConsumerWidget {
  const _ToolPreview({required this.tool});
  final BoardConnectionToolDto tool;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final readinessKey = switch (tool.readiness) {
      BoardToolReadinessDto.ready => 'board.connection.ready',
      BoardToolReadinessDto.unavailable => 'board.connection.unavailable',
      BoardToolReadinessDto.unchecked => 'board.connection.unchecked',
    };
    return ExpansionTile(
      tilePadding: EdgeInsets.zero,
      title: Text(tool.toolId),
      subtitle: Text(t(ref, readinessKey)),
      expandedCrossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(tool.description),
        for (final reason in tool.readinessReasons) Text(reason),
        if (tool.executionDirectory case final directory?) ...[
          Text(t(ref, 'board.connection.directory')),
          SelectableText(directory),
        ],
        Text(t(ref, 'board.connection.defaults')),
        SelectableText(tool.defaultsJson),
        Text(t(ref, 'board.connection.schema')),
        SelectableText(tool.inputSchemaJson),
      ],
    );
  }
}
