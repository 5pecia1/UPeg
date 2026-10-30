import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/scheduler.dart';
import 'package:upeg/src/brand/upeg_logo_geometry.g.dart';
import 'package:upeg/src/platform/motion_preferences.dart';

const startupLogoMotionDuration = Duration(milliseconds: 1400);

/// One brief impulse, settling at the canonical resting pose.
double startupLogoAngle(double progress) {
  if (progress <= 0 || progress >= 1) return 0;
  const amplitude = 6 * math.pi / 180;
  final remaining = 1 - progress;
  return amplitude * math.sin(5 * math.pi * progress) * remaining * remaining;
}

/// Keeps the same animation phase when native boot hands over to app boot.
/// The clock has no ticker; only a visible logo schedules animation frames.
class StartupLogoSession extends StatefulWidget {
  const StartupLogoSession({required this.child, super.key});

  final Widget child;

  @override
  State<StartupLogoSession> createState() => _StartupLogoSessionState();
}

class _StartupLogoSessionState extends State<StartupLogoSession> {
  final _epoch = _StartupLogoEpoch();
  int? _firstFrame;

  @override
  void initState() {
    super.initState();
    _firstFrame = SchedulerBinding.instance.scheduleFrameCallback((timestamp) {
      _firstFrame = null;
      _epoch.startedAt = timestamp;
    });
  }

  @override
  void dispose() {
    if (_firstFrame case final frame?) {
      SchedulerBinding.instance.cancelFrameCallbackWithId(frame);
    }
    super.dispose();
  }

  @override
  Widget build(BuildContext context) =>
      _StartupLogoClock(epoch: _epoch, child: widget.child);
}

class _StartupLogoEpoch {
  Duration? startedAt;
}

class _StartupLogoClock extends InheritedWidget {
  const _StartupLogoClock({required this.epoch, required super.child});

  final _StartupLogoEpoch epoch;

  @override
  bool updateShouldNotify(_StartupLogoClock oldWidget) =>
      epoch != oldWidget.epoch;
}

/// The plate and pin stay fixed while the U (including its holes) rocks.
class UpegStartupLogo extends StatefulWidget {
  const UpegStartupLogo({this.size = 88, super.key});

  final double size;

  @override
  State<UpegStartupLogo> createState() => _UpegStartupLogoState();
}

class _UpegStartupLogoState extends State<UpegStartupLogo>
    with SingleTickerProviderStateMixin, WidgetsBindingObserver {
  late final _controller = AnimationController(
    vsync: this,
    duration: startupLogoMotionDuration,
  );
  Duration? _createdAt;
  _StartupLogoEpoch? _sessionEpoch;
  int? _startFrame;
  Timer? _preferencePoll;
  bool _started = false;
  bool _motionAllowed = false;
  bool _checkingPreference = false;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _controller.addStatusListener((status) {
      if (status == AnimationStatus.completed) _preferencePoll?.cancel();
    });
    _scheduleStart();
    unawaited(_readMotionPreference());
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final clock = context
        .dependOnInheritedWidgetOfExactType<_StartupLogoClock>();
    _sessionEpoch = clock?.epoch;
    final lifecycle = WidgetsBinding.instance.lifecycleState;
    if (MediaQuery.disableAnimationsOf(context) ||
        !TickerMode.valuesOf(context).enabled ||
        (lifecycle != null && lifecycle != AppLifecycleState.resumed)) {
      _settle();
    } else {
      _scheduleStart();
    }
  }

  void _scheduleStart() {
    if (_started || _startFrame != null) return;
    _startFrame = SchedulerBinding.instance.scheduleFrameCallback((timestamp) {
      _startFrame = null;
      _createdAt ??= timestamp;
      if (mounted) _startIfAllowed(timestamp);
    });
  }

  void _startIfAllowed(Duration timestamp) {
    if (_started || !_motionAllowed) return;
    _started = true;
    final elapsed =
        timestamp - (_sessionEpoch?.startedAt ?? _createdAt ?? timestamp);
    _controller.forward(
      from: (elapsed.inMicroseconds / startupLogoMotionDuration.inMicroseconds)
          .clamp(0.0, 1.0),
    );
    if (_controller.isAnimating) {
      _preferencePoll = Timer.periodic(
        const Duration(milliseconds: 100),
        (_) => unawaited(_readMotionPreference()),
      );
    }
  }

  Future<void> _readMotionPreference() async {
    if (_checkingPreference) return;
    _checkingPreference = true;
    final disabled = await motionDisabledBySystem();
    _checkingPreference = false;
    if (!mounted) return;
    if (disabled) {
      _settle();
    } else {
      _motionAllowed = true;
      _scheduleStart();
    }
  }

  void _settle() {
    _started = true;
    if (_startFrame case final frame?) {
      SchedulerBinding.instance.cancelFrameCallbackWithId(frame);
      _startFrame = null;
    }
    _preferencePoll?.cancel();
    _controller.value = 1;
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state != AppLifecycleState.resumed) _settle();
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    if (_startFrame case final frame?) {
      SchedulerBinding.instance.cancelFrameCallbackWithId(frame);
    }
    _preferencePoll?.cancel();
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => ExcludeSemantics(
    child: SizedBox.square(
      dimension: widget.size,
      child: RepaintBoundary(
        child: AnimatedBuilder(
          animation: _controller,
          builder: (context, child) => CustomPaint(
            painter: UpegLogoPainter(
              angle: startupLogoAngle(_controller.value),
            ),
          ),
        ),
      ),
    ),
  );
}

class UpegLogoPainter extends CustomPainter {
  UpegLogoPainter({required this.angle});

  final double angle;
  static final Path _body = buildUpegLogoPath();

  @override
  void paint(Canvas canvas, Size size) {
    canvas.save();
    canvas.scale(
      size.width / upegLogoViewBoxSize,
      size.height / upegLogoViewBoxSize,
    );
    canvas.drawRRect(
      RRect.fromRectAndRadius(
        const Rect.fromLTWH(0, 0, upegLogoViewBoxSize, upegLogoViewBoxSize),
        const Radius.circular(upegLogoPlateRadius),
      ),
      Paint()..color = const Color(upegLogoInkArgb),
    );
    canvas.save();
    canvas.translate(upegLogoPinCenter.dx, upegLogoPinCenter.dy);
    canvas.rotate(angle);
    canvas.translate(-upegLogoPinCenter.dx, -upegLogoPinCenter.dy);
    canvas.drawPath(_body, Paint()..color = const Color(upegLogoBoneArgb));
    canvas.restore();
    canvas.drawCircle(
      upegLogoPinCenter,
      upegLogoPinRadius,
      Paint()..color = const Color(upegLogoOrangeArgb),
    );
    canvas.restore();
  }

  @override
  bool shouldRepaint(UpegLogoPainter oldDelegate) => angle != oldDelegate.angle;
}
