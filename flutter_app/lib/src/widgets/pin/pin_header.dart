/// The `.cell-hd` row: kind icon, tool title, toolkit, live glyph and the
/// move handle.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/rust/api/tweaks.dart';
import 'package:upeg/src/state/tweaks_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/pin/pin_keys.dart';
import 'package:upeg/src/widgets/pin/pin_move_handle.dart';
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
const Key pinHeaderPegConnectorKey = Key('pin-header-peg-connector');

class PinHeader extends ConsumerWidget {
  const PinHeader({
    required this.tokens,
    required this.accentColor,
    required this.chrome,
    required this.toolId,
    this.toolLabel,
    required this.toolkit,
    required this.showMoveHandle,
    this.moveHandle,
    this.onOpen,
    this.openLabel,
    super.key,
  }) : assert(onOpen == null || openLabel != null);

  final UpegTokens tokens;
  final Color accentColor;

  /// Null when `pinKind` hasn't resolved yet (tools catalogue still
  /// loading) — falls back to the plain dot chrome.
  final PinKindChrome? chrome;
  final String toolId;
  final String? toolLabel;
  final String toolkit;
  final bool showMoveHandle;
  final Widget? moveHandle;
  final VoidCallback? onOpen;
  final String? openLabel;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final showPegHandle =
        showMoveHandle &&
        ref.watch(pinHandleStyleProvider) == PinHandleStyleDto.peg;
    final variant = chrome?.variant;
    return Stack(
      clipBehavior: Clip.hardEdge,
      children: [
        if (showPegHandle)
          Positioned.fill(
            child: IgnorePointer(
              child: CustomPaint(
                key: pinHeaderPegConnectorKey,
                painter: _PegConnectorPainter(
                  color: tokens.lineSoft,
                  padding: UpegSizing.pinHeaderPadding,
                ),
              ),
            ),
          ),
        Container(
          key: pinHeaderKey,
          padding: UpegSizing.pinHeaderPadding,
          decoration: BoxDecoration(
            color: variant == PinChromeVariant.button
                ? accentColor.withValues(alpha: _buttonHeaderTintAlpha)
                : null,
            border: Border(bottom: BorderSide(color: tokens.lineSoft)),
          ),
          child: LayoutBuilder(
            builder: (context, constraints) {
              final showToolkit = constraints.maxWidth >= 180;
              return Row(
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
                    child: Tooltip(
                      message: onOpen == null
                          ? (showToolkit
                                ? toolId
                                : '$toolId · ${toolkit.toUpperCase()}')
                          : '$openLabel · $toolId',
                      child: InkWell(
                        key: onOpen == null ? null : pinHeaderOpenKey,
                        onTap: onOpen,
                        mouseCursor: onOpen == null
                            ? MouseCursor.defer
                            : SystemMouseCursors.click,
                        focusColor: tokens.accent.withValues(alpha: 0.18),
                        hoverColor: tokens.accent.withValues(alpha: 0.10),
                        child: Text(
                          toolLabel ?? toolId,
                          style: TextStyle(
                            fontFamily: upegMonoFontFamily,
                            fontFamilyFallback: upegMonoFontFamilyFallback,
                            fontSize: 12,
                            height: 1,
                            color: tokens.fg2,
                            fontWeight: FontWeight.w500,
                            decoration: onOpen == null
                                ? null
                                : TextDecoration.underline,
                          ),
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                    ),
                  ),
                  if (showToolkit) ...[
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
                  ],
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
                    moveHandle ?? PinMoveHandle(tokens: tokens),
                  ],
                ],
              );
            },
          ),
        ),
      ],
    );
  }
}

class _PegConnectorPainter extends CustomPainter {
  const _PegConnectorPainter({required this.color, required this.padding});

  final Color color;
  final EdgeInsets padding;

  @override
  void paint(Canvas canvas, Size size) {
    final x = size.width - padding.right - pinMoveHandleExtent / 2;
    canvas.drawLine(
      Offset(x, 0),
      Offset(x, size.height / 2),
      Paint()
        ..color = color
        ..strokeWidth = 0.75,
    );
  }

  @override
  bool shouldRepaint(_PegConnectorPainter oldDelegate) =>
      color != oldDelegate.color || padding != oldDelegate.padding;
}
