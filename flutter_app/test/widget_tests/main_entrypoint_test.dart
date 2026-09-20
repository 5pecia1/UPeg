import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

const String _mainEntrypointPath = 'lib/main.dart';
const String _windowManagerPackageImport =
    "package:window_manager/window_manager.dart";
const String _windowManagerSingletonInitialization =
    'windowManager.ensureInitialized';

void main() {
  test(
    'the_main_entrypoint_never_touches_the_windowmanager_singleton_before_binding_init',
    () {
      final source = File(_mainEntrypointPath).readAsStringSync();

      expect(source, isNot(contains(_windowManagerPackageImport)));
      expect(source, isNot(contains(_windowManagerSingletonInitialization)));
    },
  );
}
