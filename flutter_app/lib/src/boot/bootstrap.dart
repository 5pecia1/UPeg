library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/app.dart';
import 'package:upeg/src/boot/logging.dart';

typedef EnsureFlutterInitialized = WidgetsBinding Function();
typedef AsyncBootStep = Future<void> Function();
typedef RunAppFn = void Function(Widget app);

Future<void> bootstrapUpegApp({
  required EnsureFlutterInitialized ensureFlutterInitialized,
  required bool windowManagerSupported,
  required AsyncBootStep ensureWindowManagerInitialized,
  required AsyncBootStep initializeRustLib,
  required RunAppFn runAppFn,
  BootLog log = defaultBootLog,
}) async {
  ensureFlutterInitialized();
  if (windowManagerSupported) {
    log('upeg: windowManager.ensureInitialized — start');
    await ensureWindowManagerInitialized();
    log('upeg: windowManager.ensureInitialized — done');
  }
  log('upeg: RustLib.init — start');
  await initializeRustLib();
  log('upeg: RustLib.init — done');
  runAppFn(const ProviderScope(child: UpegApp()));
}
