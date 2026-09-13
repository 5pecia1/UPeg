/// The pin footer: kind label, running spinner, stale dot, restored
/// badge and the right-aligned invoker label.
library;

import 'package:flutter/material.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/pin/pin_keys.dart';

/// Font size of the restored-result badge (matches the footer's
/// kind-label scale).
const double _restoredBadgeFontSize = 9;

class PinFooter extends StatelessWidget {
  const PinFooter({
    required this.tokens,
    required this.kind,
    required this.invokerLabel,
    required this.restoredLabel,
    required this.stale,
    required this.running,
    super.key,
  });

  final UpegTokens tokens;
  final UpegPinKind? kind;
  final String? invokerLabel;

  /// Pre-localised "last run · N ago" badge copy for a restored result;
  /// null hides the badge (fresh result or no result). Lives in the
  /// footer (not the body) so it never competes with output rows for
  /// the pin's fixed body height.
  final String? restoredLabel;

  final bool stale;
  final bool running;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: UpegSizing.pinFooterPadding,
      decoration: BoxDecoration(
        border: Border(top: BorderSide(color: tokens.lineSoft)),
      ),
      child: Row(
        children: [
          if (kind != null)
            Text(
              kind!.label,
              style: TextStyle(
                fontFamily: upegMonoFontFamily,
                fontFamilyFallback: upegMonoFontFamilyFallback,
                fontSize: 9,
                letterSpacing: 0.4,
                color: tokens.fg3,
              ),
            ),
          if (running) ...[
            const SizedBox(width: 6),
            SizedBox(
              key: const Key('pin-running-indicator'),
              width: 8,
              height: 8,
              child: CircularProgressIndicator(
                strokeWidth: 1.5,
                valueColor: AlwaysStoppedAnimation<Color>(tokens.accent),
              ),
            ),
          ],
          if (stale) ...[
            const SizedBox(width: 6),
            Container(
              key: const Key('pin-stale-dot'),
              width: 4,
              height: 4,
              decoration: BoxDecoration(
                // fg3, not fg4: the stale dot is always-visible status
                // information and must clear the WCAG non-text minimum
                // (UpegTokens.minNonTextContrast) against the surface.
                color: tokens.fg3,
                shape: BoxShape.circle,
              ),
            ),
          ],
          // Restored badge shrinks (Flexible) before the invoker so a long
          // "last run · N ago" never pushes the footer past the pin edge;
          // the invoker below is the greedy Expanded that right-aligns and
          // ellipsises, keeping the whole footer overflow-safe on narrow pins.
          if (restoredLabel != null) ...[
            const SizedBox(width: 6),
            Flexible(
              child: Text(
                restoredLabel!,
                key: pinRestoredBadgeKey,
                style: TextStyle(
                  fontFamily: upegMonoFontFamily,
                  fontFamilyFallback: upegMonoFontFamilyFallback,
                  fontSize: _restoredBadgeFontSize,
                  // fg3, not fg4: always-visible status text must clear
                  // the large-text contrast floor against the surface.
                  color: tokens.fg3,
                ),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
            ),
          ],
          const SizedBox(width: 6),
          Expanded(
            child: Text(
              invokerLabel ?? '',
              textAlign: TextAlign.right,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                fontFamily: upegMonoFontFamily,
                fontFamilyFallback: upegMonoFontFamilyFallback,
                fontSize: 10,
                // fg3, not fg4: always-visible 10px status text — fg4's
                // ~2.4:1 misses even the large-text 3:1 floor.
                color: tokens.fg3,
              ),
            ),
          ),
        ],
      ),
    );
  }
}
