library;

import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'package:http/http.dart' as http;

/// Maximum untrusted wire bytes accepted from one host attach dispatch.
///
/// This caps the encoded JSON response. Canonical File decoding applies its
/// separate decoded-tree budget after this boundary.
const int kAttachDispatchResponseMaxBytes = 96 * 1024 * 1024;
const Duration _attachStreamCancellationGrace = Duration(milliseconds: 50);

sealed class AttachResponseBodyReadResult {
  const AttachResponseBodyReadResult();
}

final class AttachResponseBodyOk extends AttachResponseBodyReadResult {
  const AttachResponseBodyOk(this.body);

  final String body;
}

final class AttachResponseBodyTooLarge extends AttachResponseBodyReadResult {
  const AttachResponseBodyTooLarge();
}

final class AttachResponseBodyMalformed extends AttachResponseBodyReadResult {
  const AttachResponseBodyMalformed();
}

final class AttachResponseBodyUnreachable extends AttachResponseBodyReadResult {
  const AttachResponseBodyUnreachable();
}

/// Collect [response] without allowing an untrusted host to grow memory
/// without bound.
///
/// A declared oversized Content-Length is rejected before reading data. For
/// chunked responses, exactly [maxBytes] is accepted and the first byte beyond
/// it cancels the source stream.
Future<AttachResponseBodyReadResult> readAttachResponseBody(
  http.StreamedResponse response, {
  required Duration timeout,
  int maxBytes = kAttachDispatchResponseMaxBytes,
}) async {
  if (maxBytes <= 0) {
    throw ArgumentError.value(maxBytes, 'maxBytes', 'must be positive');
  }
  final stopwatch = Stopwatch()..start();
  if (timeout.inMicroseconds <= 0) {
    await _cancelWithoutReading(response.stream, Duration.zero);
    return const AttachResponseBodyUnreachable();
  }
  final declaredLength = response.contentLength;
  if (declaredLength != null && declaredLength > maxBytes) {
    await _cancelWithoutReading(
      response.stream,
      _cancellationBudget(timeout, stopwatch.elapsed),
    );
    return const AttachResponseBodyTooLarge();
  }

  final iterator = StreamIterator<List<int>>(response.stream);
  final bytes = BytesBuilder(copy: false);
  var collectedBytes = 0;
  var streamFinished = false;
  try {
    while (true) {
      final remainingMicros =
          timeout.inMicroseconds - stopwatch.elapsedMicroseconds;
      if (remainingMicros <= 0) {
        return const AttachResponseBodyUnreachable();
      }
      final hasNext = await iterator.moveNext().timeout(
        Duration(microseconds: remainingMicros),
      );
      if (!hasNext) {
        streamFinished = true;
        break;
      }
      final chunk = iterator.current;
      if (chunk.length > maxBytes - collectedBytes) {
        return const AttachResponseBodyTooLarge();
      }
      bytes.add(chunk);
      collectedBytes += chunk.length;
    }
  } on TimeoutException {
    return const AttachResponseBodyUnreachable();
  } on Object {
    return const AttachResponseBodyUnreachable();
  } finally {
    if (!streamFinished) {
      await _settleCancellation(
        iterator.cancel,
        _cancellationBudget(timeout, stopwatch.elapsed),
      );
    }
    stopwatch.stop();
  }

  try {
    return AttachResponseBodyOk(utf8.decode(bytes.takeBytes()));
  } on FormatException {
    return const AttachResponseBodyMalformed();
  }
}

Future<void> _cancelWithoutReading(
  Stream<List<int>> stream,
  Duration budget,
) async {
  StreamSubscription<List<int>> subscription;
  try {
    subscription = stream.listen(null);
  } on Object {
    return;
  }
  await _settleCancellation(subscription.cancel, budget);
}

Duration _cancellationBudget(Duration timeout, Duration elapsed) {
  final remainingMicros = timeout.inMicroseconds - elapsed.inMicroseconds;
  if (remainingMicros <= 0) return Duration.zero;
  final remaining = Duration(microseconds: remainingMicros);
  return remaining < _attachStreamCancellationGrace
      ? remaining
      : _attachStreamCancellationGrace;
}

Future<void> _settleCancellation(
  Future<void> Function() cancel,
  Duration budget,
) async {
  final Future<void> cancellation;
  try {
    cancellation = _ignoreCancellationFailure(cancel());
  } on Object {
    return;
  }
  if (budget.inMicroseconds <= 0) {
    unawaited(cancellation);
    return;
  }
  try {
    await cancellation.timeout(budget);
  } on TimeoutException {
    // Cancellation has started; the caller's response deadline wins.
  }
}

Future<void> _ignoreCancellationFailure(Future<void> cancellation) async {
  try {
    await cancellation;
  } on Object {
    // Transport cleanup failure must not replace the controlled attach result.
  }
}
