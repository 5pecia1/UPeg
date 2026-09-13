library;

import 'dart:typed_data';

// `maxFileInputTransportBytes` restates the canonical `File` input raw-byte
// budget (`MAX_FILE_INPUT_RAW_BYTES`) declared once in Rust at
// `upeg-core/src/input/file_budget.rs`. There is no shared build-time source
// between Dart and Rust, so this number is pinned — not merely mirrored — by
// `upeg-core/tests/file_budget_cross_language_pin.rs`, which reads this file
// by path and fails the build if it drifts from Rust.
const int maxFileInputTransportBytes = 50 * 1024 * 1024;
const int maxFileInputAggregateTransportBytes = maxFileInputTransportBytes;

int effectiveFileInputAggregateBytes(int? policyMaximumBytes) {
  if (policyMaximumBytes == null) {
    return maxFileInputAggregateTransportBytes;
  }
  return policyMaximumBytes < maxFileInputAggregateTransportBytes
      ? policyMaximumBytes
      : maxFileInputAggregateTransportBytes;
}

final class FileReadLimitExceeded implements Exception {
  const FileReadLimitExceeded(this.maximumBytes);

  final int maximumBytes;
}

Future<Uint8List> readBoundedByteStream(
  Stream<List<int>> stream, {
  required int maximumBytes,
}) async {
  if (maximumBytes < 0) {
    throw ArgumentError.value(maximumBytes, 'maximumBytes');
  }

  final bytes = BytesBuilder(copy: false);
  var byteCount = 0;
  await for (final chunk in stream) {
    byteCount += chunk.length;
    if (byteCount > maximumBytes) {
      throw FileReadLimitExceeded(maximumBytes);
    }
    bytes.add(chunk);
  }
  return bytes.takeBytes();
}
