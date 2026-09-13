library;

import 'dart:async';
import 'dart:typed_data';

import 'package:desktop_drop/desktop_drop.dart';
import 'package:flutter/widgets.dart';
import 'package:upeg/src/platform/bounded_file_reader.dart';
import 'package:upeg/src/widgets/expanded_modal/file_selection_assembler.dart';

@immutable
final class FileDropCallbacks {
  const FileDropCallbacks({
    required this.onEntered,
    required this.onExited,
    required this.onDropped,
  });

  final VoidCallback onEntered;
  final VoidCallback onExited;
  final Future<void> Function(List<FileSelectionCandidate>) onDropped;
}

abstract interface class FileDropAdapter {
  Widget wrap({required Widget child, required FileDropCallbacks callbacks});
}

final class DesktopFileDropAdapter implements FileDropAdapter {
  const DesktopFileDropAdapter();

  @override
  Widget wrap({required Widget child, required FileDropCallbacks callbacks}) {
    return DropTarget(
      onDragEntered: (_) => callbacks.onEntered(),
      onDragExited: (_) => callbacks.onExited(),
      onDragDone: (details) {
        unawaited(
          callbacks.onDropped(
            details.files.map(candidateFromItem).toList(growable: false),
          ),
        );
      },
      child: child,
    );
  }

  FileSelectionCandidate candidateFromItem(DropItem item) {
    if (item is DropItemDirectory) {
      return FileSelectionCandidate.directory(name: item.name);
    }
    return FileSelectionCandidate.file(
      name: item.name,
      mime: item.mimeType,
      readBytes: (maximumBytes) => _readDropItem(item, maximumBytes),
    );
  }
}

Future<Uint8List> _readDropItem(DropItem item, int maximumBytes) {
  return _withAppleBookmark(item, () async {
    final metadataLength = await item.length();
    if (metadataLength > maximumBytes) {
      throw FileReadLimitExceeded(maximumBytes);
    }
    return readBoundedByteStream(
      item.openRead(0, maximumBytes + 1),
      maximumBytes: maximumBytes,
    );
  });
}

Future<T> _withAppleBookmark<T>(
  DropItem item,
  Future<T> Function() operation,
) async {
  final bookmark = item.extraAppleBookmark;
  if (bookmark == null || bookmark.isEmpty) return operation();

  final accessStarted = await DesktopDrop.instance
      .startAccessingSecurityScopedResource(bookmark: bookmark);
  try {
    return await operation();
  } finally {
    if (accessStarted) {
      await DesktopDrop.instance.stopAccessingSecurityScopedResource(
        bookmark: bookmark,
      );
    }
  }
}
