import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/app.dart';
import 'package:upeg/src/boot/bootstrap.dart';

void main() {
  group('bootstrapUpegApp', () {
    testWidgets('bootstrapUpegApp은_RustLib_init_시작과_완료를_로그한다', (tester) async {
      final logs = <String>[];
      Widget? launched;
      var flutterInitialized = false;
      var rustInitialized = false;

      await bootstrapUpegApp(
        ensureFlutterInitialized: () {
          flutterInitialized = true;
          return tester.binding;
        },
        windowManagerSupported: false,
        ensureWindowManagerInitialized: () async {
          fail('window manager should not initialize when unsupported');
        },
        initializeRustLib: () async {
          rustInitialized = true;
        },
        runAppFn: (app) => launched = app,
        log: logs.add,
      );

      expect(flutterInitialized, isTrue);
      expect(rustInitialized, isTrue);
      expect(logs, contains('upeg: RustLib.init — start'));
      expect(logs, contains('upeg: RustLib.init — done'));
      expect(logs.any((line) => line.contains('ERROR')), isFalse);
      expect(launched, isA<ProviderScope>());
      final scope = launched! as ProviderScope;
      expect(scope.child, isA<UpegApp>());
    });

    testWidgets('bootstrapUpegApp은_binding_다음_window_manager를_초기화한다', (
      tester,
    ) async {
      final steps = <String>[];

      await bootstrapUpegApp(
        ensureFlutterInitialized: () {
          steps.add('flutter');
          return tester.binding;
        },
        windowManagerSupported: true,
        ensureWindowManagerInitialized: () async {
          steps.add('window');
        },
        initializeRustLib: () async {
          steps.add('rust');
        },
        runAppFn: (_) {
          steps.add('runApp');
        },
      );

      expect(steps, const <String>['flutter', 'window', 'rust', 'runApp']);
    });
  });
}
