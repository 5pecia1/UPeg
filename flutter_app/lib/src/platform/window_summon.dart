/// Launcher-immediacy window summoning (summon / toggle), separate
/// from the popup ↔ full geometry logic in `window.dart` (SoC).
///
/// Three entry points converge here:
///   * the global hotkey (`platform/global_hotkey.dart`),
///   * the tray icon left click (`platform/tray.dart`),
///   * an incoming deep link (`widgets/launch_intent_applier.dart`).
///
/// The mode-change pipeline (`windowModeProvider` →
/// `WindowModeApplier`) deliberately no-ops when the mode is unchanged,
/// so a hidden window whose mode already matches would never
/// re-surface through it. [summonWindow] is the unconditional
/// "show + focus (+ restore)" primitive that fixes that gap without
/// weakening the applier's dedupe contract.
///
/// Type-system notes: the OS window state is captured ONCE into an
/// immutable [WindowSnapshot] before any async step runs (snapshot
/// capture, no mid-flow re-query), the toggle outcome is the
/// [SummonToggleDecision] enum, and the summon sequence is a pure
/// `List<SummonStep>` plan — all three are unit-testable without
/// platform channels.
library;

import 'package:flutter/foundation.dart' show immutable;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:window_manager/window_manager.dart';

import 'package:upeg/src/platform/window.dart' show isWindowManagerSupported;
import 'package:upeg/src/state/window_mode_provider.dart';

/// One-shot capture of the OS window state. Both fields are read
/// before any mutation so a decision never mixes pre- and post-step
/// observations.
@immutable
class WindowSnapshot {
  const WindowSnapshot({required this.isVisible, required this.isMinimized});

  final bool isVisible;
  final bool isMinimized;

  /// "Actually on screen for the user": visible AND not minimized.
  /// A minimized window reports `isVisible == true` on some platforms,
  /// so visibility alone would mis-route the toggle into `hide`.
  bool get isShown => isVisible && !isMinimized;
}

/// Outcome of the summon toggle — the launcher semantics are
/// "shown → hide, otherwise → summon as popup".
enum SummonToggleDecision { hideWindow, summonPopup }

/// Pure toggle judgment. Kept free of platform calls so the truth
/// table is unit-testable.
SummonToggleDecision decideSummonToggle(WindowSnapshot snapshot) =>
    snapshot.isShown
    ? SummonToggleDecision.hideWindow
    : SummonToggleDecision.summonPopup;

/// One platform step of the summon sequence.
enum SummonStep { restore, show, focus }

/// Pure state transition: which steps a summon needs, in order.
/// `restore` only fires for a minimized window; `show` + `focus`
/// always run so a hidden-but-same-mode window still comes to front.
List<SummonStep> summonPlan({required bool isMinimized}) => [
  if (isMinimized) SummonStep.restore,
  SummonStep.show,
  SummonStep.focus,
];

/// Platform seam for summon/hide. Production routes to
/// `window_manager`; tests pass a recording double so no platform
/// channel is ever touched.
abstract class WindowSummonDriver {
  const WindowSummonDriver();

  Future<bool> isVisible();
  Future<bool> isMinimized();
  Future<void> restore();
  Future<void> show();
  Future<void> focus();
  Future<void> hide();
}

/// Default production driver — delegates to `window_manager`.
class WindowManagerSummonDriver extends WindowSummonDriver {
  const WindowManagerSummonDriver();

  @override
  Future<bool> isVisible() => windowManager.isVisible();

  @override
  Future<bool> isMinimized() => windowManager.isMinimized();

  @override
  Future<void> restore() => windowManager.restore();

  @override
  Future<void> show() => windowManager.show();

  @override
  Future<void> focus() => windowManager.focus();

  @override
  Future<void> hide() => windowManager.hide();
}

/// Capture the [WindowSnapshot] through the driver.
Future<WindowSnapshot> captureWindowSnapshot(WindowSummonDriver driver) async {
  final isVisible = await driver.isVisible();
  final isMinimized = await driver.isMinimized();
  return WindowSnapshot(isVisible: isVisible, isMinimized: isMinimized);
}

/// Resolve the driver, gating the production default behind
/// `isWindowManagerSupported` so web/tests never touch the channel.
/// An explicitly injected driver always wins (tests).
WindowSummonDriver? _resolveDriver(WindowSummonDriver? driver) {
  if (driver != null) return driver;
  if (!isWindowManagerSupported) return null;
  return const WindowManagerSummonDriver();
}

/// Execute a pre-computed summon plan against the driver.
Future<void> runSummonPlan(
  WindowSummonDriver driver, {
  required bool isMinimized,
}) async {
  for (final step in summonPlan(isMinimized: isMinimized)) {
    switch (step) {
      case SummonStep.restore:
        await driver.restore();
      case SummonStep.show:
        await driver.show();
      case SummonStep.focus:
        await driver.focus();
    }
  }
}

/// Unconditionally bring the window to the foreground: restore (if
/// minimized) + show + focus. Mode-agnostic on purpose — callers that
/// also want a mode change flip `windowModeProvider` themselves.
/// No-op on unsupported platforms.
Future<void> summonWindow({WindowSummonDriver? driver}) async {
  final resolved = _resolveDriver(driver);
  if (resolved == null) return;
  final snapshot = await captureWindowSnapshot(resolved);
  await runSummonPlan(resolved, isMinimized: snapshot.isMinimized);
}

/// Launcher toggle: shown → hide; hidden/minimized → force popup mode
/// and summon. Shared by the global hotkey and the tray left click so
/// both surfaces stay behaviorally identical. No-op on unsupported
/// platforms.
Future<void> runSummonToggle(
  ProviderContainer container, {
  WindowSummonDriver? driver,
}) async {
  final resolved = _resolveDriver(driver);
  if (resolved == null) return;
  final snapshot = await captureWindowSnapshot(resolved);
  switch (decideSummonToggle(snapshot)) {
    case SummonToggleDecision.hideWindow:
      await resolved.hide();
    case SummonToggleDecision.summonPopup:
      // Mode first: `WindowModeApplier` reacts to the state change and
      // applies popup geometry; the plan below guarantees show+focus
      // even when the mode was already `popup` (applier dedupes).
      container.read(windowModeProvider.notifier).set(WindowMode.popup);
      await runSummonPlan(resolved, isMinimized: snapshot.isMinimized);
  }
}
