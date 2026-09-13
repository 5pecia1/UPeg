/// Small coloured chip rendering [`PinKindDto`].
///
/// Lives next to the modal because the header is its sole caller —
/// the Pin widget already has its own footer-baked KindBadge surface
/// (see `widgets/pin.dart`). Both copies share the colour table in
/// `UpegTokens.pinKindColor` so a future palette change touches one
/// place.
///
/// [iconForTool] sits in the same file so the header builder can read
/// "what icon goes with this kind?" without dragging in a separate
/// import — the answer is just a Material icon constant per kind, no
/// FRB hop needed.
library;

import 'package:flutter/material.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/theme/upeg_theme.dart';

/// Resolve a `ToolDto`'s display icon from its `pinKind`. A Material
/// icon per kind so the modal header looks distinct at a glance.
/// Unknown variants (forward-compat for new Rust-side kinds) fall back
/// to `Icons.extension` so a UI shipped ahead of Dart catches up still
/// renders something meaningful.
IconData iconForTool(ToolDto tool) {
  switch (tool.pinKind) {
    case PinKindDto.inline:
      return Icons.flash_on;
    case PinKindDto.launcher:
      return Icons.launch;
    case PinKindDto.live:
      return Icons.bolt;
    case PinKindDto.action:
      return Icons.play_arrow;
    case PinKindDto.embed:
      return Icons.web;
    case PinKindDto.controlledEmbed:
      return Icons.web_asset;
    case PinKindDto.chain:
      return Icons.link;
    case PinKindDto.llm:
      return Icons.psychology;
  }
}

/// Small kind-coloured chip rendering the PinKind label ("INLINE",
/// "LAUNCHER", …). Reuses `UpegTokens.pinKindColor` so the colour
/// table stays in one place.
class KindBadge extends StatelessWidget {
  const KindBadge({required this.pinKind, super.key});

  final PinKindDto pinKind;

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    final color = tokens.pinKindColor(UpegPinKind.fromDto(pinKind));
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.18),
        border: Border.all(color: color.withValues(alpha: 0.55)),
        borderRadius: BorderRadius.circular(UpegSizing.radius1),
      ),
      child: Text(
        UpegPinKind.fromDto(pinKind).label,
        style: TextStyle(
          color: color,
          fontFamily: upegMonoFontFamily,
          fontFamilyFallback: upegMonoFontFamilyFallback,
          fontSize: 9,
          letterSpacing: 0.6,
          fontWeight: FontWeight.w600,
        ),
      ),
    );
  }
}
