library;

import 'dart:typed_data';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:xml/xml.dart';
import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/platform/file_picker_bridge.dart';
import 'package:upeg/src/theme/upeg_theme.dart';

abstract final class FileOutputKeys {
  static const summary = Key('structured-output-file-summary');
  static const save = Key('structured-output-file-save');
  static const preview = Key('structured-output-image-preview');
  static const status = Key('structured-output-file-status');
}

const double _previewExtent = 220;
const int _previewCacheWidth = 480;
const int _maxSvgPreviewBytes = 2 * 1024 * 1024;
const double _spacing = 8;
const Set<String> _previewMimes = {
  'image/png',
  'image/jpeg',
  'image/webp',
  'image/gif',
  'image/bmp',
};

// Generated SVGs consist solely of paths; reject external-resource markup.
bool _safeSvgPreview(Uint8List bytes) {
  if (bytes.length > _maxSvgPreviewBytes) return false;
  try {
    final document = XmlDocument.parse(utf8.decode(bytes));
    if (document.children.whereType<XmlDoctype>().isNotEmpty) return false;
    final root = document.rootElement;
    if (root.name.qualified != 'svg') return false;
    for (final element in root.descendants.whereType<XmlElement>()) {
      if (element.name.qualified != 'path') return false;
    }
    for (final element in [root, ...root.descendants.whereType<XmlElement>()]) {
      for (final attribute in element.attributes) {
        if (!{
          'xmlns',
          'width',
          'height',
          'viewBox',
          'd',
          'fill',
          'transform',
        }.contains(attribute.name.qualified)) {
          return false;
        }
        if (attribute.value.toLowerCase().contains('url(')) return false;
      }
    }
    return true;
  } catch (_) {
    return false;
  }
}

/// Shared File output UX, including a bounded preview and recoverable save errors.
class FileOutputCard extends ConsumerStatefulWidget {
  const FileOutputCard({
    required this.name,
    required this.summary,
    required this.bytes,
    required this.mime,
    required this.bridge,
    super.key,
  });

  final String name;
  final String summary;
  final Uint8List? bytes;
  final String? mime;
  final FilePickerBridge bridge;

  @override
  ConsumerState<FileOutputCard> createState() => _FileOutputCardState();
}

class _FileOutputCardState extends ConsumerState<FileOutputCard> {
  bool _saving = false;
  bool _saved = false;
  bool _failed = false;

  @override
  void didUpdateWidget(covariant FileOutputCard oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.bytes != widget.bytes || oldWidget.name != widget.name) {
      _saved = false;
      _failed = false;
    }
  }

  Future<void> _save() async {
    final bytes = widget.bytes;
    if (bytes == null || _saving) return;
    final dialogTitle = tRead(ref, 'media.file.save_title');
    setState(() {
      _saving = true;
      _saved = false;
      _failed = false;
    });
    try {
      final saved = await widget.bridge.saveBytes(
        name: widget.name,
        bytes: bytes,
        mime: widget.mime ?? 'application/octet-stream',
        dialogTitle: dialogTitle,
      );
      if (mounted && identical(widget.bytes, bytes)) {
        setState(() => _saved = saved);
      }
    } catch (_) {
      if (mounted) setState(() => _failed = true);
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final bytes = widget.bytes;
    final rasterPreview = bytes != null && _previewMimes.contains(widget.mime);
    final svgPreview =
        bytes != null &&
        widget.mime == 'image/svg+xml' &&
        _safeSvgPreview(bytes);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (rasterPreview || svgPreview)
          Padding(
            padding: const EdgeInsets.only(bottom: _spacing),
            child: Container(
              height: _previewExtent,
              decoration: BoxDecoration(
                color: Theme.of(context).colorScheme.surfaceContainerHighest,
                borderRadius: BorderRadius.circular(UpegSizing.radius2),
              ),
              child: svgPreview
                  ? SvgPicture.memory(
                      bytes,
                      key: FileOutputKeys.preview,
                      fit: BoxFit.contain,
                      semanticsLabel: t(ref, 'media.file.preview'),
                      errorBuilder: (_, _, _) => Center(
                        child: Text(t(ref, 'media.file.preview_unavailable')),
                      ),
                    )
                  : Image.memory(
                      bytes,
                      key: FileOutputKeys.preview,
                      fit: BoxFit.contain,
                      cacheWidth: _previewCacheWidth,
                      semanticLabel: t(ref, 'media.file.preview'),
                      errorBuilder: (_, _, _) => Center(
                        child: Text(t(ref, 'media.file.preview_unavailable')),
                      ),
                    ),
            ),
          ),
        if (bytes != null && widget.mime == 'image/svg+xml' && !svgPreview)
          Padding(
            padding: const EdgeInsets.only(bottom: _spacing),
            child: Text(t(ref, 'media.file.preview_unavailable')),
          ),
        Row(
          children: [
            Expanded(
              child: SelectableText(
                widget.summary,
                key: FileOutputKeys.summary,
              ),
            ),
            const SizedBox(width: _spacing),
            TextButton.icon(
              key: FileOutputKeys.save,
              icon: const Icon(Icons.save_alt),
              label: Text(
                t(ref, _saving ? 'media.file.saving' : 'media.file.save'),
              ),
              onPressed: bytes == null || _saving ? null : _save,
            ),
          ],
        ),
        if (_saved || _failed)
          Semantics(
            liveRegion: true,
            child: Text(
              t(
                ref,
                _failed
                    ? 'media.file.save_failed'
                    : (kIsWeb
                          ? 'media.file.download_started'
                          : 'media.file.saved'),
              ),
              key: FileOutputKeys.status,
              style: TextStyle(
                color: _failed ? context.upeg.warn : context.upeg.fg3,
              ),
            ),
          ),
      ],
    );
  }
}
