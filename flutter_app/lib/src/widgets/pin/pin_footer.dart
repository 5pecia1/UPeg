/// The pin footer: running spinner, stale dot and restored-result badge.
///
/// The row is always present, at a minimum height that follows the text
/// scale, so a status flipping on or off (an Inline pin runs on every
/// keystroke) never reflows the pin body. Constant strings (the pin kind,
/// the invoker) are not drawn here: the header icon, the Semantics label and
/// the expanded modal already carry them.
library;

import 'package:flutter/material.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/pin/pin_keys.dart';

/// Font size of the restored-result badge.
const double _restoredBadgeFontSize = 9;

/// Minimum height of the footer's content row at the default text scale: one
/// 10px line at the theme's 1.43 body line height (about 14px), which is what
/// the retired kind and invoker labels gave it. Keeps the idle footer at its
/// old 23px footprint. It is scaled with the text, as those labels were, so
/// the smaller restored badge never makes the row taller at any scale.
const double _footerContentMinHeight = 14;

/// Gap between the footer's status markers.
const double _footerMarkerGap = 6;

class PinFooter extends StatelessWidget {
  const PinFooter({
    required this.tokens,
    required this.restoredLabel,
    required this.stale,
    required this.running,
    super.key,
  });

  final UpegTokens tokens;

  /// Pre-localised "result saved · N ago" badge copy for a restored result;
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
      child: ConstrainedBox(
        constraints: BoxConstraints(
          minHeight: MediaQuery.textScalerOf(
            context,
          ).scale(_footerContentMinHeight),
        ),
        child: Row(
          spacing: _footerMarkerGap,
          children: [
            if (running)
              SizedBox(
                key: const Key('pin-running-indicator'),
                width: 8,
                height: 8,
                child: CircularProgressIndicator(
                  strokeWidth: 1.5,
                  valueColor: AlwaysStoppedAnimation<Color>(tokens.accent),
                ),
              ),
            if (stale)
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
            // The restored badge is the only flexible marker: it shrinks and
            // ellipsises, so a long "result saved · N ago" never pushes the
            // footer past the edge of a narrow pin.
            if (restoredLabel != null)
              Flexible(
                child: Tooltip(
                  message: restoredLabel!,
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
              ),
          ],
        ),
      ),
    );
  }
}
