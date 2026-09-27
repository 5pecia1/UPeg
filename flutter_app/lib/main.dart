import 'package:flutter/material.dart';
import 'package:upeg/src/boot/bootstrap.dart';
import 'package:upeg/src/boot/internal_error_reporter.dart';
import 'package:upeg/src/platform/window.dart';
import 'package:upeg/src/rust/frb_generated.dart';

Future<void> main() {
  return bootstrapUpegApp(
    ensureFlutterInitialized: WidgetsFlutterBinding.ensureInitialized,
    windowManagerSupported: isWindowManagerSupported,
    ensureWindowManagerInitialized: ensureWindowManagerInitialized,
    initializeRustLib: RustLib.init,
    installInternalErrorReporter: installFlutterDiagnosticReporter,
    runAppFn: runApp,
  );
}
