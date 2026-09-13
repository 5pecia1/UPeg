/// PinKind-driven chrome selection.
///
/// A pin's *silhouette* — the icon in its header and the shape of its
/// chrome (tinted header, panelled body, framed body, live glyph) — is
/// driven entirely by [UpegPinKind], never by the specific tool. This
/// used to be a runtime `ToolId -> PinRenderer` map (the bespoke
/// renderer escape hatch); it started empty and stayed empty because
/// nothing needed a per-tool renderer, only a per-*kind* one. Replaced
/// with an immutable descriptor + an exhaustive switch: a ninth
/// `UpegPinKind` variant fails to compile here until it's given an
/// icon + [PinChromeVariant], instead of silently falling back to a
/// runtime-missing lookup.
library;

import 'package:flutter/material.dart' show Icons;
import 'package:flutter/widgets.dart';
import 'package:upeg/src/theme/upeg_theme.dart';

/// Visual family a [UpegPinKind] belongs to. Drives the chrome tweaks
/// `pin.dart` applies around the header/body — see call sites of
/// `pinKindChrome` there for the exact treatment per variant.
enum PinChromeVariant {
  /// Action-shaped kinds: tapping fires a side effect. Header reads as
  /// a button (tinted background).
  button,

  /// Kinds whose defining trait is a text/number result. Body reads as
  /// an output readout (panelled background).
  output,

  /// Kinds that embed a live webview. Body reads as a framed viewport
  /// (top accent strip + border).
  frame,

  /// Kinds that poll/refresh on their own cadence. Header carries a
  /// small refresh glyph alongside the kind icon.
  live,
}

/// Immutable per-kind chrome descriptor: the icon drawn in the pin
/// header plus the [PinChromeVariant] silhouette treatment.
@immutable
class PinKindChrome {
  const PinKindChrome({required this.icon, required this.variant});

  final IconData icon;
  final PinChromeVariant variant;

  @override
  bool operator ==(Object other) =>
      other is PinKindChrome && other.icon == icon && other.variant == variant;

  @override
  int get hashCode => Object.hash(icon, variant);
}

/// Resolve the chrome descriptor for [kind]. Exhaustive switch over
/// [UpegPinKind] — no default branch, so the analyzer/compiler flags a
/// new Rust-side `PinKind` variant the moment it lands in Dart instead
/// of it silently rendering the generic dot chrome forever.
PinKindChrome pinKindChrome(UpegPinKind kind) {
  switch (kind) {
    case UpegPinKind.action:
      return const PinKindChrome(
        icon: Icons.bolt,
        variant: PinChromeVariant.button,
      );
    case UpegPinKind.launcher:
      return const PinKindChrome(
        icon: Icons.launch,
        variant: PinChromeVariant.button,
      );
    case UpegPinKind.chain:
      return const PinKindChrome(
        icon: Icons.account_tree,
        variant: PinChromeVariant.button,
      );
    case UpegPinKind.llm:
      return const PinKindChrome(
        icon: Icons.auto_awesome,
        variant: PinChromeVariant.button,
      );
    case UpegPinKind.inline:
      return const PinKindChrome(
        icon: Icons.short_text,
        variant: PinChromeVariant.output,
      );
    case UpegPinKind.live:
      return const PinKindChrome(
        icon: Icons.sensors,
        variant: PinChromeVariant.live,
      );
    case UpegPinKind.embed:
      return const PinKindChrome(
        icon: Icons.public,
        variant: PinChromeVariant.frame,
      );
    case UpegPinKind.controlledEmbed:
      return const PinKindChrome(
        icon: Icons.tune,
        variant: PinChromeVariant.frame,
      );
  }
}
