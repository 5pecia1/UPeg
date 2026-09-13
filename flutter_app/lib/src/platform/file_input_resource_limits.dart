library;

import 'dart:convert';

// `maxFileInputCount`, `maxFileInputNodes`, and `maxFileInputMetadataBytes`
// restate the canonical `File` input budget declared once in Rust at
// `upeg-core/src/input/file_budget.rs`. There is no shared build-time source
// between Dart and Rust, so these numbers are pinned — not merely mirrored —
// by `upeg-core/tests/file_budget_cross_language_pin.rs`, which reads this
// file by path and fails the build if it drifts from Rust.
const int maxFileInputCount = 100;
const int maxFileInputNodes = 128;
const int maxFileInputMetadataBytes = 16 * 1024;
const String fileInputSelectionRootName = 'files';

typedef FileInputNodeMetadata = ({String name, String? mime});

int flatFileInputNodeCount(int fileCount, {required bool createsDirectory}) {
  return fileCount + (createsDirectory ? 1 : 0);
}

int fileInputMetadataByteCount(
  Iterable<FileInputNodeMetadata> nodes, {
  required bool includesSelectionRoot,
}) {
  var total = includesSelectionRoot
      ? utf8.encode(fileInputSelectionRootName).length
      : 0;
  for (final node in nodes) {
    total += utf8.encode(node.name).length;
    final mime = node.mime;
    if (mime != null) total += utf8.encode(mime).length;
  }
  return total;
}
