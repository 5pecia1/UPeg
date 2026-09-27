/// Routes uncaught Flutter framework and platform errors to the native,
/// redacted diagnostics store after Rust has initialized.
library;

import 'package:flutter/foundation.dart';
import 'package:upeg/src/rust/api/diagnostics.dart' as diagnostics;

void installFlutterDiagnosticReporter() {
  final priorFrameworkHandler = FlutterError.onError;
  FlutterError.onError = (details) {
    _record(details.exception, details.stack ?? StackTrace.empty);
    if (priorFrameworkHandler != null) {
      priorFrameworkHandler(details);
    } else {
      FlutterError.presentError(details);
    }
  };

  final dispatcher = PlatformDispatcher.instance;
  final priorPlatformHandler = dispatcher.onError;
  dispatcher.onError = (error, stack) {
    _record(error, stack);
    return priorPlatformHandler?.call(error, stack) ?? false;
  };
}

void _record(Object error, StackTrace stack) {
  try {
    diagnostics.recordFlutterError(message: '$error', stack: '$stack');
  } on Object catch (recordError) {
    debugPrint('upeg: could not persist Flutter diagnostic: $recordError');
  }
}
