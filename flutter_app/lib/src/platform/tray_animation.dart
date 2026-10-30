/// A finite startup cue; no timer or native icon update remains while idle.
library;

import 'dart:async';

import 'package:flutter/foundation.dart';

import 'package:upeg/src/platform/motion_preferences.dart';
import 'package:upeg/src/platform/tray_icon.dart';

const traySwayFrameCount = 9;
const traySwayFrameDuration = Duration(milliseconds: 100);

TrayIconSpec traySwayFrameFor(TargetPlatform platform, int frame) {
  RangeError.checkValueInInterval(frame, 0, traySwayFrameCount - 1, 'frame');
  final base = trayIconSpecFor(platform);
  final (directory, extension) = switch (platform) {
    TargetPlatform.macOS => ('macos', 'png'),
    TargetPlatform.windows => ('windows', 'ico'),
    _ => ('linux', 'png'),
  };
  return (
    assetPath: 'assets/tray/sway/$directory/frame_$frame.$extension',
    isTemplate: base.isTemplate,
    iconSize: base.iconSize,
  );
}

/// Runs icon writes sequentially, dropping repeated requests while active.
/// Cancellation waits for any in-flight write before restoring the static icon.
class TraySwayAnimation {
  TraySwayAnimation({
    required this.platform,
    this.setIcon = setPlatformTrayIcon,
    this.motionDisabled = motionDisabledBySystem,
  });

  final TargetPlatform platform;
  final SetTrayIconFn setIcon;
  final Future<bool> Function() motionDisabled;
  Future<void>? _running;
  Timer? _timer;
  Completer<void>? _delay;
  bool _cancelled = false;
  bool _disposed = false;
  bool _restoreAllowed = true;

  Future<void> play() {
    if (_disposed) return Future<void>.value();
    if (_running case final running?) return running;
    _cancelled = false;
    final running = _play();
    _running = running;
    return running.whenComplete(() => _running = null);
  }

  Future<void> _play() async {
    var changed = false;
    try {
      for (var frame = 0; frame < traySwayFrameCount; frame++) {
        if (_cancelled || await motionDisabled() || _cancelled) break;
        changed = true;
        await setIcon(traySwayFrameFor(platform, frame));
        if (_cancelled) break;
        final delay = Completer<void>();
        _delay = delay;
        _timer = Timer(traySwayFrameDuration, delay.complete);
        await delay.future;
        _timer = null;
        _delay = null;
      }
    } on Exception catch (error) {
      debugPrint('upeg: tray startup cue stopped: $error');
    } finally {
      if (changed && _restoreAllowed) {
        try {
          await setIcon(trayIconSpecFor(platform));
        } on Exception catch (error) {
          debugPrint('upeg: tray static icon restore failed: $error');
        }
      }
    }
  }

  Future<void> stop() async {
    _cancelled = true;
    _timer?.cancel();
    _timer = null;
    final delay = _delay;
    if (delay != null && !delay.isCompleted) delay.complete();
    await _running;
  }

  Future<void> dispose() async {
    _disposed = true;
    // Quit must not wait indefinitely on a native icon write. If it stalls,
    // never enqueue a late restore after the window has been destroyed.
    // An already-issued native call cannot be cancelled by this API.
    await stop().timeout(
      const Duration(milliseconds: 200),
      onTimeout: () => _restoreAllowed = false,
    );
  }
}

TraySwayAnimation? _startupAnimation;

/// Called only after the static icon and tray menu have installed successfully.
Future<void> playTrayStartupAnimation() async {
  if (_startupAnimation != null) return;
  final animation = TraySwayAnimation(platform: defaultTargetPlatform);
  _startupAnimation = animation;
  try {
    await animation.play();
  } finally {
    if (identical(_startupAnimation, animation)) _startupAnimation = null;
  }
}

Future<void> stopTrayAnimation() async {
  await _startupAnimation?.dispose();
}
