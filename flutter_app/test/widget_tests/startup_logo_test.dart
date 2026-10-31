import 'dart:async';
import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/boot/bootstrap_gate.dart';
import 'package:upeg/src/brand/startup_logo.dart';
import 'package:upeg/src/brand/upeg_logo_geometry.g.dart';
import 'package:upeg/src/pages/splash_page.dart';

void main() {
  test('should create startup states before a frame exists', () {
    expect(
      () => const StartupLogoSession(child: SizedBox.shrink()).createState(),
      returnsNormally,
    );
    expect(() => const UpegStartupLogo().createState(), returnsNormally);
  });

  test('should settle at rest after a finite damped impulse', () {
    expect(startupLogoAngle(0), 0);
    expect(startupLogoAngle(1), 0);
    expect(startupLogoAngle(2), 0);
    expect(startupLogoAngle(0.1), greaterThan(0));
    expect(startupLogoAngle(0.3), lessThan(0));
    expect(startupLogoAngle(0.9).abs(), lessThan(startupLogoAngle(0.1).abs()));
    for (var frame = 0; frame <= 140; frame++) {
      expect(
        startupLogoAngle(frame / 140).abs(),
        lessThanOrEqualTo(math.pi / 18),
      );
    }
  });

  testWidgets('should stop scheduling logo frames after settling', (
    tester,
  ) async {
    final robot = _StartupLogoRobot(tester);
    await robot.pumpLogo();
    await tester.pump(const Duration(milliseconds: 140));
    robot.expectMoving();

    await tester.pump(startupLogoMotionDuration);
    robot.expectAtRest();
    robot.expectNoScheduledFrame();
    await robot.removeLogo();
  });

  testWidgets('should keep the pin and plate fixed while the U moves', (
    tester,
  ) async {
    final robot = _StartupLogoRobot(tester);
    await tester.runAsync(robot.expectFixedPinAndPlate);
  });

  testWidgets('should remain still when animations are disabled', (
    tester,
  ) async {
    final robot = _StartupLogoRobot(tester);
    await robot.pumpLogo(disableAnimations: true);
    await tester.pump(const Duration(milliseconds: 140));
    robot.expectAtRest();
    robot.expectNoScheduledFrame();
    await robot.removeLogo();
  });

  testWidgets('should stay still until the native motion preference is known', (
    tester,
  ) async {
    final robot = _StartupLogoRobot(tester);
    final preference = Completer<bool>();
    robot.useNativePreference(() => preference.future);
    await robot.pumpLogo();
    await tester.pump(const Duration(milliseconds: 140));
    robot.expectAtRest();

    preference.complete(false);
    await tester.pump();
    await tester.pump();
    robot.expectAngle(startupLogoAngle(0.1));
    await robot.removeLogo();
  });

  testWidgets(
    'should settle when the native preference changes during motion',
    (tester) async {
      final robot = _StartupLogoRobot(tester);
      var disabled = false;
      robot.useNativePreference(() async => disabled);
      await robot.pumpLogo();
      await tester.pump(const Duration(milliseconds: 140));
      robot.expectMoving();

      disabled = true;
      await tester.pump(const Duration(milliseconds: 100));
      await tester.pump();
      robot.expectAtRest();
      robot.expectNoScheduledFrame();
      await robot.removeLogo();
    },
  );

  testWidgets('should continue the same impulse when native boot completes', (
    tester,
  ) async {
    final robot = _StartupLogoRobot(tester);
    final outcome = Completer<BootstrapOutcome>();
    await tester.pumpWidget(
      BootstrapGate(
        outcome: outcome.future,
        readyChild: const RepaintBoundary(
          child: MaterialApp(home: SplashPage()),
        ),
      ),
    );
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 420));
    robot.expectAngle(startupLogoAngle(0.3));
    robot.rememberLogoInstance();

    outcome.complete(const BootstrapReady());
    await tester.pump();
    await tester.pump();
    robot.expectDifferentLogoInstance();
    robot.expectAngle(startupLogoAngle(0.3));
    await tester.pump(const Duration(milliseconds: 980));
    robot.expectAtRest();
    await robot.removeLogo();
  });

  testWidgets('should show ready content before the impulse finishes', (
    tester,
  ) async {
    final robot = _StartupLogoRobot(tester);
    final outcome = Completer<BootstrapOutcome>();
    await tester.pumpWidget(
      BootstrapGate(
        outcome: outcome.future,
        readyChild: const Directionality(
          textDirection: TextDirection.ltr,
          child: Text('Ready'),
        ),
      ),
    );
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 140));
    robot.expectMoving();

    outcome.complete(const BootstrapReady());
    await tester.pump();
    robot.expectReady();
    await robot.removeLogo();
  });

  testWidgets(
    'should stay settled when app boot begins after the impulse ends',
    (tester) async {
      final robot = _StartupLogoRobot(tester);
      final outcome = Completer<BootstrapOutcome>();
      await tester.pumpWidget(
        BootstrapGate(
          outcome: outcome.future,
          readyChild: const RepaintBoundary(
            child: MaterialApp(home: SplashPage()),
          ),
        ),
      );
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 1600));
      robot.rememberLogoInstance();

      outcome.complete(const BootstrapReady());
      await tester.pump();
      await tester.pump();
      robot.expectDifferentLogoInstance();
      robot.expectAtRest();
      await robot.removeLogo();
    },
  );

  testWidgets('should settle without replaying when its ticker is disabled', (
    tester,
  ) async {
    final robot = _StartupLogoRobot(tester);
    await robot.pumpLogo();
    await tester.pump(const Duration(milliseconds: 140));
    robot.expectMoving();

    await robot.pumpLogo(tickerEnabled: false);
    robot.expectAtRest();
    await robot.pumpLogo();
    await tester.pump(const Duration(milliseconds: 140));
    robot.expectAtRest();
    robot.expectNoScheduledFrame();
    await robot.removeLogo();
  });

  testWidgets('should settle without replaying after the app is hidden', (
    tester,
  ) async {
    final robot = _StartupLogoRobot(tester);
    await robot.pumpLogo();
    await tester.pump(const Duration(milliseconds: 140));
    robot.expectMoving();

    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.hidden);
    await tester.pump();
    robot.expectAtRest();
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
    await tester.pump(const Duration(milliseconds: 140));
    robot.expectAtRest();
    robot.expectNoScheduledFrame();
    await robot.removeLogo();
  });
}

class _StartupLogoRobot {
  _StartupLogoRobot(this.tester);

  final WidgetTester tester;
  State? _previousLogo;
  bool _usingNativePreference = false;

  void useNativePreference(Future<bool> Function() query) {
    _usingNativePreference = true;
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    const channel = MethodChannel('upeg/accessibility');
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(channel, (_) => query());
    addTearDown(() {
      messenger.setMockMethodCallHandler(channel, null);
      debugDefaultTargetPlatformOverride = null;
    });
  }

  void rememberLogoInstance() {
    _previousLogo = tester.state(find.byType(UpegStartupLogo));
  }

  void expectDifferentLogoInstance() => expect(
    identical(_previousLogo, tester.state(find.byType(UpegStartupLogo))),
    false,
  );

  Future<void> pumpLogo({
    bool disableAnimations = false,
    bool tickerEnabled = true,
  }) async {
    await tester.pumpWidget(
      MaterialApp(
        home: MediaQuery(
          data: MediaQueryData(disableAnimations: disableAnimations),
          child: TickerMode(
            enabled: tickerEnabled,
            child: const Center(child: UpegStartupLogo()),
          ),
        ),
      ),
    );
    await tester.pump();
  }

  UpegLogoPainter get _painter =>
      tester
              .widget<CustomPaint>(
                find.byWidgetPredicate(
                  (widget) =>
                      widget is CustomPaint &&
                      widget.painter is UpegLogoPainter,
                ),
              )
              .painter!
          as UpegLogoPainter;

  void expectMoving() => expect(_painter.angle.abs(), greaterThan(0.001));
  void expectAtRest() => expect(_painter.angle, 0);
  void expectAngle(double angle) =>
      expect(_painter.angle, closeTo(angle, 0.00001));
  void expectNoScheduledFrame() =>
      expect(tester.binding.hasScheduledFrame, false);
  void expectReady() {
    expect(find.text('Ready'), findsOneWidget);
    expect(find.byType(UpegStartupLogo), findsNothing);
  }

  Future<void> removeLogo() async {
    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pump(startupLogoMotionDuration);
    expect(tester.takeException(), isNull);
    if (_usingNativePreference) debugDefaultTargetPlatformOverride = null;
  }

  Future<void> expectFixedPinAndPlate() async {
    Future<List<int>> paint(double angle) async {
      final recorder = ui.PictureRecorder();
      UpegLogoPainter(
        angle: angle,
      ).paint(Canvas(recorder), const Size.square(upegLogoViewBoxSize));
      final picture = recorder.endRecording();
      final image = await picture.toImage(512, 512);
      final pixels = await image.toByteData();
      final bytes = pixels!.buffer.asUint8List().toList();
      image.dispose();
      picture.dispose();
      return bytes;
    }

    final resting = await paint(0);
    final rocking = await paint(startupLogoAngle(0.1));
    expect(rocking, isNot(orderedEquals(resting)));
    List<int> pinPixels(List<int> bytes) => [
      for (var index = 0; index < bytes.length; index += 4)
        if (bytes[index] == (upegLogoOrangeArgb >> 16 & 255) &&
            bytes[index + 1] == (upegLogoOrangeArgb >> 8 & 255) &&
            bytes[index + 2] == (upegLogoOrangeArgb & 255) &&
            bytes[index + 3] == 255)
          index,
    ];
    final restingPin = pinPixels(resting);
    expect(restingPin.length, greaterThan(100));
    expect(pinPixels(rocking), orderedEquals(restingPin));
    for (final point in [
      const Offset(256, 40),
      upegLogoPinCenter,
      upegLogoPinCenter + Offset(upegLogoPinRadius * 0.7, 0),
      upegLogoPinCenter - Offset(upegLogoPinRadius * 0.7, 0),
      upegLogoPinCenter + Offset(0, upegLogoPinRadius * 0.7),
      upegLogoPinCenter - Offset(0, upegLogoPinRadius * 0.7),
    ]) {
      final index = (point.dy.floor() * 512 + point.dx.floor()) * 4;
      expect(
        rocking.sublist(index, index + 4),
        resting.sublist(index, index + 4),
      );
    }
  }
}
