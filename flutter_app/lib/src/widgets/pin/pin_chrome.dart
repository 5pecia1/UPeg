/// The `.cell` shell of a pin: border, corner holes, keyboard focus
/// ring, and the per-kind silhouette treatment applied to the body slot.
library;

import 'package:flutter/material.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/pin/pin_keys.dart';
import 'package:upeg/src/widgets/pin_renderers/registry.dart'
    show PinChromeVariant;

const double _pinBorderWidth = 1;

/// Keyboard-focus ring thickness (WCAG 2.4.13 asks for ≥2px).
const double _focusRingWidth = 2;

/// Gap between the pin border and the focus ring. The ring floats
/// OUTSIDE the pin so it is painted against the board background —
/// `tokens.focusRing` guarantees contrast there no matter what
/// `pinColorOverride` did to the pin border itself.
const double _focusRingOffset = 2;

/// Inset around the `output` variant's body panel.
const double _outputPanelMargin = 4;

/// Height of the accent strip along the top of the `frame` variant's
/// body wrapper (mimics a webview toolbar).
const double _frameStripHeight = 4;

/// Alpha applied to `accentColor` for the `frame` variant's top strip.
const double _frameStripAlpha = 0.16;

/// Border width around the `frame` variant's body wrapper.
const double _frameBorderWidth = 1;

/// The `.cell` shell — surface + line border + 4 corner-hole dots.
class PinChrome extends StatelessWidget {
  const PinChrome({
    required this.tokens,
    required this.focused,
    required this.borderColor,
    required this.clipContent,
    required this.header,
    required this.body,
    required this.footer,
    super.key,
  });

  final UpegTokens tokens;
  final bool focused;
  final Color borderColor;
  final bool clipContent;
  final Widget header;
  final Widget body;
  final Widget footer;

  @override
  Widget build(BuildContext context) {
    return Stack(
      // The focus ring floats OUTSIDE the pin bounds (offset + width);
      // the board grid stack already uses `Clip.none`, so the ring
      // survives up the tree.
      clipBehavior: Clip.none,
      children: [
        Container(
          key: pinChromeContainerKey,
          decoration: BoxDecoration(
            color: tokens.surface,
            border: Border.all(color: borderColor, width: _pinBorderWidth),
            borderRadius: BorderRadius.circular(UpegSizing.radius2),
          ),
          clipBehavior: clipContent ? Clip.antiAlias : Clip.none,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              header,
              Expanded(child: body),
              footer,
            ],
          ),
        ),
        // Four corner-hole indicators (mirrors `cell::before / ::after /
        // .holeBL / .holeBR` in input.css).
        const _CornerHole(top: 5, left: 5),
        const _CornerHole(top: 5, right: 5),
        const _CornerHole(bottom: 5, left: 5),
        const _CornerHole(bottom: 5, right: 5),
        // Keyboard-focus ring: ≥2px, offset outside the pin border, in
        // a token whose contrast against the board background is
        // guaranteed for both themes — independent of both the accent
        // choice and any `pinColorOverride` on the border underneath.
        if (focused)
          Positioned.fill(
            left: -(_focusRingWidth + _focusRingOffset),
            top: -(_focusRingWidth + _focusRingOffset),
            right: -(_focusRingWidth + _focusRingOffset),
            bottom: -(_focusRingWidth + _focusRingOffset),
            child: IgnorePointer(
              child: DecoratedBox(
                key: pinFocusRingKey,
                decoration: BoxDecoration(
                  border: Border.all(
                    color: tokens.focusRing,
                    width: _focusRingWidth,
                  ),
                  borderRadius: BorderRadius.circular(
                    UpegSizing.radius2 + _focusRingOffset,
                  ),
                ),
              ),
            ),
          ),
      ],
    );
  }
}

/// Applies the [PinKindChrome] silhouette treatment to the body slot.
///
/// `button`/`live` variants carry their entire chrome in the header
/// (tint / refresh glyph) and leave the body untouched. `output`
/// wraps the body in a `surface2` panel (readout emphasis); `frame`
/// wraps it in a thin bordered viewport with a top accent strip
/// (webview emphasis). Applied uniformly whether the body is the
/// default `PinBody` or a caller-supplied `bodyOverride` — Embed-kind
/// pins get the `frame` treatment either way.
///
/// Deliberately never sets `clipBehavior`: `PinChrome` already skips
/// clipping the outer shell when `bodyOverride` is present (Embed's
/// live `WebViewPanel` misbehaves under a clip), so this wrapper
/// leaves the default `Clip.none` too instead of reintroducing it.
Widget pinBodyChrome({
  required PinChromeVariant? variant,
  required UpegTokens tokens,
  required Color accentColor,
  required Widget child,
}) {
  switch (variant) {
    case PinChromeVariant.output:
      return Container(
        key: pinOutputPanelKey,
        margin: const EdgeInsets.all(_outputPanelMargin),
        decoration: BoxDecoration(
          color: tokens.surface2,
          borderRadius: BorderRadius.circular(UpegSizing.radius1),
        ),
        child: child,
      );
    case PinChromeVariant.frame:
      return Container(
        key: pinEmbedFrameKey,
        decoration: BoxDecoration(
          border: Border.all(color: tokens.lineSoft, width: _frameBorderWidth),
          borderRadius: BorderRadius.circular(UpegSizing.radius1),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Container(
              height: _frameStripHeight,
              color: accentColor.withValues(alpha: _frameStripAlpha),
            ),
            Expanded(child: child),
          ],
        ),
      );
    case PinChromeVariant.button:
    case PinChromeVariant.live:
    case null:
      return child;
  }
}

class _CornerHole extends StatelessWidget {
  const _CornerHole({this.top, this.bottom, this.left, this.right});
  final double? top;
  final double? bottom;
  final double? left;
  final double? right;

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    return Positioned(
      top: top,
      bottom: bottom,
      left: left,
      right: right,
      child: IgnorePointer(
        child: Container(
          width: 4,
          height: 4,
          decoration: BoxDecoration(
            color: tokens.holeDeep,
            shape: BoxShape.circle,
            border: Border.all(color: tokens.line, width: 0.5),
          ),
        ),
      ),
    );
  }
}
