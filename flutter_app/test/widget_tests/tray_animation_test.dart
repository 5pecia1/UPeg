import 'dart:async';

import 'package:fake_async/fake_async.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/platform/motion_preferences.dart';
import 'package:upeg/src/platform/tray_animation.dart';
import 'package:upeg/src/platform/tray_icon.dart';

import '../shared/fake_tray_icon_driver.dart';

void main() {
  group('TraySwayAnimation', () {
    test('should finish one nine-frame cue in 900ms then stay idle', () {
      fakeAsync((time) {
        final driver = FakeTrayIconDriver();
        final animation = TraySwayAnimation(
          platform: TargetPlatform.macOS,
          setIcon: driver.setIcon,
          motionDisabled: () async => false,
        );
        var completed = false;

        animation.play().then((_) => completed = true);
        animation.play(); // A burst while active must not enqueue another cue.
        time.flushMicrotasks();
        expect(driver.calls, hasLength(1));
        time.elapse(const Duration(milliseconds: 99));
        expect(driver.calls, hasLength(1));
        time.elapse(const Duration(milliseconds: 800));
        expect(driver.calls, hasLength(9));
        expect(completed, isFalse);
        time.elapse(const Duration(milliseconds: 1));

        expect(completed, isTrue);
        expect(driver.calls, hasLength(10));
        expect(driver.calls.last, trayIconSpecFor(TargetPlatform.macOS));
        expect(driver.calls.every((spec) => spec.isTemplate), isTrue);
        expect(driver.calls.every((spec) => spec.iconSize == 22), isTrue);
        time.elapse(const Duration(minutes: 1));
        expect(driver.calls, hasLength(10));
        expect(time.nonPeriodicTimerCount, 0);
        expect(time.periodicTimerCount, 0);
      });
    });

    test('should keep the static icon when motion is disabled at startup', () {
      fakeAsync((time) {
        final driver = FakeTrayIconDriver();
        final animation = TraySwayAnimation(
          platform: TargetPlatform.linux,
          setIcon: driver.setIcon,
          motionDisabled: () async => true,
        );

        animation.play();
        time.flushMicrotasks();

        expect(driver.calls, isEmpty);
        expect(time.nonPeriodicTimerCount, 0);
        expect(time.periodicTimerCount, 0);
      });
    });

    test('should restore promptly when motion becomes disabled mid-cue', () {
      fakeAsync((time) {
        final driver = FakeTrayIconDriver();
        var reduceMotion = false;
        final animation = TraySwayAnimation(
          platform: TargetPlatform.windows,
          setIcon: driver.setIcon,
          motionDisabled: () async => reduceMotion,
        );
        animation.play();
        time.flushMicrotasks();

        reduceMotion = true;
        time.elapse(const Duration(milliseconds: 100));

        expect(driver.calls, hasLength(2));
        expect(driver.calls.last.assetPath, 'assets/tray/tray_icon.ico');
        expect(time.nonPeriodicTimerCount, 0);
        expect(time.periodicTimerCount, 0);
      });
    });

    test('should cancel the delay and reject playback after disposal', () {
      fakeAsync((time) {
        final driver = FakeTrayIconDriver();
        final animation = TraySwayAnimation(
          platform: TargetPlatform.linux,
          setIcon: driver.setIcon,
          motionDisabled: () async => false,
        );
        animation.play();
        time.flushMicrotasks();

        animation.dispose();
        time.flushMicrotasks();
        animation.play();
        time.elapse(const Duration(seconds: 5));

        expect(driver.calls, hasLength(2));
        expect(driver.calls.last.assetPath, 'assets/tray_icon.png');
        expect(time.nonPeriodicTimerCount, 0);
        expect(time.periodicTimerCount, 0);
      });
    });

    test('should await an in-flight write before restoring on stop', () {
      fakeAsync((time) {
        final write = Completer<void>();
        final driver = FakeTrayIconDriver()..beforeWrite = (_) => write.future;
        final animation = TraySwayAnimation(
          platform: TargetPlatform.macOS,
          setIcon: driver.setIcon,
          motionDisabled: () async => false,
        );
        animation.play();
        time.flushMicrotasks();
        var stopped = false;

        animation.stop().then((_) => stopped = true);
        time.flushMicrotasks();
        expect(stopped, isFalse);
        expect(driver.calls, hasLength(1));
        write.complete();
        time.flushMicrotasks();

        expect(stopped, isTrue);
        expect(driver.calls, hasLength(2));
        expect(driver.calls.last, trayIconSpecFor(TargetPlatform.macOS));
        expect(driver.maxInFlight, 1);
        expect(time.nonPeriodicTimerCount, 0);
        expect(time.periodicTimerCount, 0);
      });
    });

    test('should restore static and stop when a frame write fails', () {
      fakeAsync((time) {
        final driver = FakeTrayIconDriver()
          ..beforeWrite = (spec) async {
            if (spec.assetPath.contains('/sway/')) {
              throw PlatformException(code: 'failed');
            }
          };
        final animation = TraySwayAnimation(
          platform: TargetPlatform.linux,
          setIcon: driver.setIcon,
          motionDisabled: () async => false,
        );

        animation.play();
        time.flushMicrotasks();

        expect(driver.calls, hasLength(2));
        expect(driver.calls.last.assetPath, 'assets/tray_icon.png');
        expect(time.nonPeriodicTimerCount, 0);
        expect(time.periodicTimerCount, 0);
      });
    });

    test(
      'should bound disposal and avoid a late restore after a stalled write',
      () {
        fakeAsync((time) {
          final write = Completer<void>();
          final driver = FakeTrayIconDriver()
            ..beforeWrite = (_) => write.future;
          final animation = TraySwayAnimation(
            platform: TargetPlatform.macOS,
            setIcon: driver.setIcon,
            motionDisabled: () async => false,
          );
          animation.play();
          time.flushMicrotasks();
          var disposed = false;

          animation.dispose().then((_) => disposed = true);
          time.elapse(const Duration(milliseconds: 200));

          expect(disposed, isTrue);
          write.complete();
          time.flushMicrotasks();
          expect(driver.calls, hasLength(1));
          expect(time.nonPeriodicTimerCount, 0);
          expect(time.periodicTimerCount, 0);
        });
      },
    );
  });

  testWidgets('should query native reduced motion and fail closed', (
    tester,
  ) async {
    const channel = MethodChannel('upeg/accessibility');
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    final messenger = tester.binding.defaultBinaryMessenger;
    addTearDown(() => messenger.setMockMethodCallHandler(channel, null));
    messenger.setMockMethodCallHandler(channel, (call) async {
      expect(call.method, 'reduceMotion');
      return true;
    });

    try {
      expect(await motionDisabledBySystem(), isTrue);
      messenger.setMockMethodCallHandler(channel, (call) async => false);
      expect(await motionDisabledBySystem(), isFalse);
      messenger.setMockMethodCallHandler(channel, (call) async {
        throw MissingPluginException('runner query unavailable');
      });
      expect(await motionDisabledBySystem(), isTrue);
    } finally {
      debugDefaultTargetPlatformOverride = null;
    }
  });

  testWidgets('should load every platform sway asset from the bundle', (
    tester,
  ) async {
    for (final platform in [
      TargetPlatform.linux,
      TargetPlatform.macOS,
      TargetPlatform.windows,
    ]) {
      for (var frame = 0; frame < 9; frame++) {
        final spec = traySwayFrameFor(platform, frame);
        final data = await tester.runAsync(
          () => rootBundle.load(spec.assetPath),
        );
        expect(data!.lengthInBytes, greaterThan(0), reason: spec.assetPath);
        if (platform == TargetPlatform.windows) {
          expect(data.getUint16(0, Endian.little), 0);
          expect(data.getUint16(2, Endian.little), 1);
        } else {
          final size = platform == TargetPlatform.macOS ? 44 : 22;
          expect(data.getUint32(16), size, reason: spec.assetPath);
          expect(data.getUint32(20), size, reason: spec.assetPath);
        }
        if (frame == 8) {
          final rest = await tester.runAsync(
            () => rootBundle.load(trayIconSpecFor(platform).assetPath),
          );
          expect(
            data.buffer.asUint8List(data.offsetInBytes, data.lengthInBytes),
            orderedEquals(
              rest!.buffer.asUint8List(rest.offsetInBytes, rest.lengthInBytes),
            ),
            reason: 'the final frame must equal the installed static asset',
          );
        }
      }
    }
  });

  testWidgets('should keep static when the native motion query stalls', (
    tester,
  ) async {
    const channel = MethodChannel('upeg/accessibility');
    final response = Completer<bool>();
    final messenger = tester.binding.defaultBinaryMessenger;
    debugDefaultTargetPlatformOverride = TargetPlatform.windows;
    messenger.setMockMethodCallHandler(channel, (_) => response.future);
    try {
      var completed = false;
      final preference = motionDisabledBySystem()
        ..then((_) => completed = true);

      await tester.pump(const Duration(milliseconds: 199));
      expect(completed, isFalse);
      await tester.pump(const Duration(milliseconds: 1));
      expect(await preference, isTrue);
      response.complete(false);
      await tester.pump();
    } finally {
      debugDefaultTargetPlatformOverride = null;
      messenger.setMockMethodCallHandler(channel, null);
    }
  });
}
