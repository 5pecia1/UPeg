/// Geometry re-apply observer (inventory row C03).
///
/// Wraps a child subtree, watches [`windowModeProvider`], and fires
/// `applyWindowMode` on every mode change — including the initial
/// mount — so the OS window geometry follows Riverpod state changes
/// regardless of which call site drove them (tray menu, popup-page
/// "open desktop", launch-intent applier, …).
///
/// Type system / SoC notes:
/// - The platform call is typed as a top-level `Future&lt;void&gt;`
///   `Function(WindowMode)` (alias [ApplyWindowMode]) so tests inject a
///   recording closure without touching `window_manager`. Production
///   passes `platform/window.dart::applyWindowMode`.
/// - Equal-state re-sets are NOT applied a second time — the
///   `Notifier`'s built-in state equality handles the common case, and
///   the observer's own `previous == next` guard pins the contract so
///   manual `addListener` migrations have to re-add the dedupe.
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/platform/window.dart' as platform;
import 'package:upeg/src/state/window_mode_provider.dart';

/// Platform-call seam. Production hands in
/// `platform/window.dart::applyWindowMode`; tests pass a recording
/// closure.
typedef ApplyWindowMode = Future<void> Function(WindowMode mode);

class WindowModeApplier extends ConsumerStatefulWidget {
  const WindowModeApplier({
    super.key,
    required this.child,
    this.applyWindowMode = platform.applyWindowMode,
  });

  /// Subtree this observer wraps. Kept as a vanilla [Widget] so the
  /// observer can mount above any route — the production wiring lives
  /// at the `MaterialApp.home` level next to `PopupAutoHideObserver`.
  final Widget child;

  /// Injected platform call — defaults to
  /// `platform/window.dart::applyWindowMode`. The default reads the
  /// `isWindowManagerSupported` gate internally so the call is safe
  /// on unsupported targets.
  final ApplyWindowMode applyWindowMode;

  @override
  ConsumerState<WindowModeApplier> createState() => _WindowModeApplierState();
}

class _WindowModeApplierState extends ConsumerState<WindowModeApplier> {
  WindowMode? _last;

  @override
  void initState() {
    super.initState();
    // Boot-time application: fire once for the initial mode. The
    // platform call ignores unsupported targets internally, so this is
    // safe in widget tests that override `applyWindowMode:` and in
    // production on web (where the seam no-ops).
    final initial = ref.read(windowModeProvider);
    _last = initial;
    // Defer until after the first frame so we never block the build
    // pipeline on a platform-channel round-trip.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      // ignore: discarded_futures — boot-time apply is fire-and-forget.
      _applyBestEffort(initial);
    });
  }

  Future<void> _applyBestEffort(WindowMode mode) async {
    try {
      await widget.applyWindowMode(mode);
    } on Object catch (err, stack) {
      debugPrint('upeg: applyWindowMode($mode) failed: $err');
      debugPrint(stack.toString());
    }
  }

  @override
  Widget build(BuildContext context) {
    // Riverpod 3 listen — fires whenever windowModeProvider's state
    // changes. The Notifier deduplicates equal states so the `previous
    // != next` guard below is belt-and-braces for manual listener
    // migrations.
    ref.listen<WindowMode>(windowModeProvider, (previous, next) {
      if (previous == next || _last == next) return;
      _last = next;
      // ignore: discarded_futures — observer dispatch is fire-and-forget.
      _applyBestEffort(next);
    });
    return widget.child;
  }
}
