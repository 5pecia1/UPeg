import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/boot/bootstrap.dart';
import 'package:upeg/src/boot/bootstrap_gate.dart';
import 'package:upeg/src/pages/splash_page.dart';

void main() {
  group('bootstrapUpegApp', () {
    testWidgets('bootstrapUpegApp_logs_RustLib_init_start_and_completion', (
      tester,
    ) async {
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
        waitForFirstFrame: () async {},
        log: logs.add,
      );

      expect(flutterInitialized, isTrue);
      expect(rustInitialized, isTrue);
      expect(logs, contains('upeg: RustLib.init — start'));
      expect(logs, contains('upeg: RustLib.init — done'));
      expect(logs.any((line) => line.contains('ERROR')), isFalse);
      expect(launched, isA<BootstrapGate>());
      expect((launched! as BootstrapGate).readyChild, isNotNull);
    });

    testWidgets('bootstrapUpegApp_initializes_window_manager_after_binding', (
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
        installInternalErrorReporter: () {
          steps.add('error reporter');
        },
        runAppFn: (_) {
          steps.add('runApp');
        },
        waitForFirstFrame: () async {
          steps.add('first frame');
        },
      );

      expect(steps, const <String>[
        'flutter',
        'runApp',
        'first frame',
        'window',
        'rust',
        'error reporter',
      ]);
    });

    testWidgets('should show splash until native initialization completes', (
      tester,
    ) async {
      final windowReady = Completer<void>();
      final rustReady = Completer<void>();
      var firstFramePainted = false;
      var windowStarts = 0;
      Widget? launched;
      final boot = bootstrapUpegApp(
        ensureFlutterInitialized: () => tester.binding,
        windowManagerSupported: true,
        ensureWindowManagerInitialized: () {
          if (!firstFramePainted) {
            throw StateError(
              'window initialization ran before the first frame',
            );
          }
          windowStarts += 1;
          return windowReady.future;
        },
        initializeRustLib: () => rustReady.future,
        runAppFn: (app) => launched = app,
      );

      expect(launched, isA<BootstrapGate>());
      expect(windowStarts, 0);
      final gate = launched! as BootstrapGate;
      tester.binding.addPostFrameCallback((_) => firstFramePainted = true);
      await tester.pumpWidget(
        BootstrapGate(
          outcome: gate.outcome,
          readyChild: const Directionality(
            textDirection: TextDirection.ltr,
            child: Text('Ready'),
          ),
        ),
      );
      expect(find.byType(SplashPage), findsOneWidget);
      expect(find.text('Ready'), findsNothing);
      await tester.pump();
      expect(windowStarts, 1);

      windowReady.complete();
      await tester.pump();
      expect(find.byType(SplashPage), findsOneWidget);
      rustReady.complete();
      await boot;
      await tester.pump();
      expect(find.byType(SplashPage), findsNothing);
      expect(find.text('Ready'), findsOneWidget);
    });

    testWidgets('should show failure when native initialization throws', (
      tester,
    ) async {
      Widget? launched;
      final rustReady = Completer<void>();
      final boot = bootstrapUpegApp(
        ensureFlutterInitialized: () => tester.binding,
        windowManagerSupported: false,
        ensureWindowManagerInitialized: () async {},
        initializeRustLib: () => rustReady.future,
        runAppFn: (app) => launched = app,
      );
      final bootExpectation = expectLater(boot, throwsStateError);
      final gate = launched! as BootstrapGate;
      await tester.pumpWidget(
        BootstrapGate(outcome: gate.outcome, readyChild: const Text('Ready')),
      );
      expect(find.byType(SplashPage), findsOneWidget);

      rustReady.completeError(StateError('native init failed'));
      await bootExpectation;
      await tester.pump();
      expect(find.textContaining('native init failed'), findsOneWidget);
      expect(find.byType(SplashPage), findsNothing);
    });
  });
}
