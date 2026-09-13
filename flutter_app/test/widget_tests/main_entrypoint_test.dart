import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

const String _mainEntrypointPath = 'lib/main.dart';
const String _windowManagerPackageImport =
    "package:window_manager/window_manager.dart";
const String _windowManagerSingletonInitialization =
    'windowManager.ensureInitialized';

void main() {
  test('main_entrypoint는_binding_초기화_전_windowManager_singleton을_건드리지_않는다', () {
    final source = File(_mainEntrypointPath).readAsStringSync();

    expect(source, isNot(contains(_windowManagerPackageImport)));
    expect(source, isNot(contains(_windowManagerSingletonInitialization)));
  });
}
