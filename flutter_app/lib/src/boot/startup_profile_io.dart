import 'dart:convert';
import 'dart:io';

const _prefix = 'UPEG_STARTUP_PROFILE ';
final _enabled = Platform.environment['UPEG_STARTUP_PROFILE'] == '1';
Stopwatch? _clock;

bool get startupProfileEnabled => _enabled;

/// Elapsed time is relative to the first marker, `bootstrap_enter`.
void markStartupPhase(String phase) {
  if (!_enabled) return;
  final first = _clock == null;
  final clock = _clock ??= (Stopwatch()..start());
  stdout.writeln(
    '$_prefix${jsonEncode(<String, Object>{'phase': phase, 'dart_elapsed_us': first ? 0 : clock.elapsedMicroseconds})}',
  );
}
