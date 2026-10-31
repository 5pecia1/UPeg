library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/app.dart';
import 'package:upeg/src/boot/bootstrap_gate.dart';
import 'package:upeg/src/boot/logging.dart';
import 'package:upeg/src/boot/startup_profile.dart';

typedef EnsureFlutterInitialized = WidgetsBinding Function();
typedef AsyncBootStep = Future<void> Function();
typedef RunAppFn = void Function(Widget app);
typedef InternalErrorReporterInstaller = void Function();

Future<void> _waitForFirstFrame() => WidgetsBinding.instance.endOfFrame;

Future<void> bootstrapUpegApp({
  required EnsureFlutterInitialized ensureFlutterInitialized,
  required bool windowManagerSupported,
  required AsyncBootStep ensureWindowManagerInitialized,
  required AsyncBootStep initializeRustLib,
  required RunAppFn runAppFn,
  AsyncBootStep waitForFirstFrame = _waitForFirstFrame,
  InternalErrorReporterInstaller? installInternalErrorReporter,
  BootLog log = defaultBootLog,
}) async {
  markStartupPhase('bootstrap_enter');
  ensureFlutterInitialized();
  if (startupProfileEnabled) {
    unawaited(
      WidgetsBinding.instance.waitUntilFirstFrameRasterized.then(
        (_) => markStartupPhase('first_flutter_raster'),
      ),
    );
  }
  final outcome = Completer<BootstrapOutcome>();
  runAppFn(
    BootstrapGate(
      outcome: outcome.future,
      readyChild: const ProviderScope(child: UpegApp()),
    ),
  );
  try {
    // `runApp` only schedules a frame. Let the Flutter-only splash paint
    // before plugin initialization or synchronous Rust loading starts.
    await waitForFirstFrame();
    if (windowManagerSupported) {
      markStartupPhase('window_manager_start');
      log('upeg: windowManager.ensureInitialized — start');
      await ensureWindowManagerInitialized();
      log('upeg: windowManager.ensureInitialized — done');
      markStartupPhase('window_manager_done');
    }
    markStartupPhase('rust_lib_start');
    log('upeg: RustLib.init — start');
    await initializeRustLib();
    log('upeg: RustLib.init — done');
    markStartupPhase('rust_lib_done');
    installInternalErrorReporter?.call();
    outcome.complete(const BootstrapReady());
  } on Object catch (error, stack) {
    outcome.complete(BootstrapFailed(error));
    Error.throwWithStackTrace(error, stack);
  }
}
