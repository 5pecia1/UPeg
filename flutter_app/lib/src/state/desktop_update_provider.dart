import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/platform/desktop_update.dart';
import 'package:upeg/src/platform/update_dismissal.dart';

class DesktopUpdateState {
  const DesktopUpdateState({
    this.version,
    this.dismissed = false,
    this.busy = false,
    this.failed = false,
    this.error,
  });
  final String? version;
  final bool dismissed;
  final bool busy;
  final bool failed;
  final String? error;
  bool get unread => version != null && !dismissed;
  DesktopUpdateState copyWith({
    String? version,
    bool? dismissed,
    bool? busy,
    bool? failed,
    String? error,
  }) => DesktopUpdateState(
    version: version ?? this.version,
    dismissed: dismissed ?? this.dismissed,
    busy: busy ?? this.busy,
    failed: failed ?? this.failed,
    error: error,
  );
}

final updateDismissalReaderProvider = Provider<Future<String?> Function()>(
  (ref) => readDismissedUpdate,
);
final updateDismissalWriterProvider = Provider<Future<void> Function(String)>(
  (ref) => writeDismissedUpdate,
);

final desktopUpdateServiceProvider = Provider<DesktopUpdateService>(
  (ref) => DesktopUpdateService(),
);
final desktopUpdateProvider =
    NotifierProvider<DesktopUpdateNotifier, DesktopUpdateState>(
      DesktopUpdateNotifier.new,
    );

class DesktopUpdateNotifier extends Notifier<DesktopUpdateState> {
  bool _started = false;
  @override
  DesktopUpdateState build() => const DesktopUpdateState();

  Future<void> check(String currentVersion) async {
    if (_started ||
        kIsWeb ||
        !{
          TargetPlatform.macOS,
          TargetPlatform.windows,
          TargetPlatform.linux,
        }.contains(defaultTargetPlatform)) {
      return;
    }
    _started = true;
    final version = await ref
        .read(desktopUpdateServiceProvider)
        .check(currentVersion);
    if (version == null || !ref.mounted) return;
    final dismissed = await ref.read(updateDismissalReaderProvider)();
    if (!ref.mounted) return;
    state = DesktopUpdateState(
      version: version,
      dismissed: dismissed == version,
    );
  }

  Future<void> dismiss() async {
    final version = state.version;
    if (version == null) return;
    state = state.copyWith(dismissed: true);
    try {
      await ref.read(updateDismissalWriterProvider)(version);
    } on Exception {
      /* In-memory dismissal still applies. */
    }
  }

  Future<DesktopUpdateAction?> perform() async {
    final version = state.version;
    if (version == null || state.busy) return null;
    state = state.copyWith(busy: true, failed: false);
    try {
      final result = await ref
          .read(desktopUpdateServiceProvider)
          .perform(version);
      if (ref.mounted) state = state.copyWith(busy: false);
      return result;
    } on Exception catch (error) {
      if (ref.mounted) {
        state = state.copyWith(
          busy: false,
          failed: true,
          error: error.toString(),
        );
      }
      return null;
    }
  }
}
