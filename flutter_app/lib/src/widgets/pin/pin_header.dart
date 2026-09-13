/// The `.cell-hd` row: kind icon, tool id, toolkit, live glyph and the
/// move handle.
library;

import 'package:flutter/material.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/pin/pin_keys.dart';
import 'package:upeg/src/widgets/pin_renderers/registry.dart'
    show PinChromeVariant, PinKindChrome;

// PinKind chrome silhouette tuning (Batch — "PinKind별 실루엣"). Every
// magic number driving the per-variant header/body treatment lives
// here as a named constant instead of inline literals.
/// Size of the kind icon painted in the header (replaces the old
/// uniform dot when `pinKind` is known).
const double _pinKindIconSize = 12;

/// Size of the small refresh glyph next to the header icon for `live`
/// kinds.
const double _liveIndicatorIconSize = 10;

/// Alpha applied to `accentColor` for the `button` variant's header
/// background tint.
const double _buttonHeaderTintAlpha = 0.10;

class PinHeader extends StatelessWidget {
  const PinHeader({
    required this.tokens,
    required this.accentColor,
    required this.chrome,
    required this.toolId,
    required this.toolkit,
    required this.showMoveHandle,
    this.moveHandle,
    super.key,
  });

  final UpegTokens tokens;
  final Color accentColor;

  /// Null when `pinKind` hasn't resolved yet (tools catalogue still
  /// loading) — falls back to the plain dot chrome.
  final PinKindChrome? chrome;
  final String toolId;
  final String toolkit;
  final bool showMoveHandle;
  final Widget? moveHandle;

  @override
  Widget build(BuildContext context) {
    final variant = chrome?.variant;
    return Container(
      key: pinHeaderKey,
      padding: UpegSizing.pinHeaderPadding,
      decoration: BoxDecoration(
        color: variant == PinChromeVariant.button
            ? accentColor.withValues(alpha: _buttonHeaderTintAlpha)
            : null,
        border: Border(bottom: BorderSide(color: tokens.lineSoft)),
      ),
      child: Row(
        children: [
          if (chrome == null)
            Container(
              width: 8,
              height: 8,
              decoration: BoxDecoration(
                color: accentColor,
                shape: BoxShape.circle,
              ),
            )
          else
            Icon(
              chrome!.icon,
              key: pinKindIconKey,
              size: _pinKindIconSize,
              color: accentColor,
            ),
          const SizedBox(width: 6),
          Expanded(
            child: Text(
              toolId.toUpperCase(),
              style: TextStyle(
                fontFamily: upegMonoFontFamily,
                fontFamilyFallback: upegMonoFontFamilyFallback,
                fontSize: 10,
                letterSpacing: 0.4,
                color: tokens.fg2,
                fontWeight: FontWeight.w500,
              ),
              overflow: TextOverflow.ellipsis,
            ),
          ),
          const SizedBox(width: 6),
          Text(
            toolkit.toUpperCase(),
            style: TextStyle(
              fontFamily: upegMonoFontFamily,
              fontFamilyFallback: upegMonoFontFamilyFallback,
              fontSize: 10,
              letterSpacing: 0.4,
              color: tokens.fg3,
            ),
          ),
          if (variant == PinChromeVariant.live) ...[
            const SizedBox(width: 4),
            Icon(
              Icons.autorenew,
              key: pinLiveIndicatorKey,
              size: _liveIndicatorIconSize,
              color: accentColor,
            ),
          ],
          if (showMoveHandle) ...[
            const SizedBox(width: 6),
            moveHandle ??
                Tooltip(
                  message: pinMoveTooltip,
                  child: Icon(
                    Icons.drag_indicator,
                    key: pinMoveHandleKey,
                    size: 13,
                    color: tokens.accent,
                  ),
                ),
          ],
        ],
      ),
    );
  }
}
