/// Ghost-label overlay shown while a move or a resize is in flight, so
/// the user can see which neighbours a commit would displace.
library;

import 'package:flutter/material.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/board_canvas/board_grid_metrics.dart';

/// F16 — minimal push-preview overlay. Renders one small dot + label
/// per displaced placement so the user can see what the projected
/// layout looks like before committing. Polish (full ghost pin
/// chrome, animated transitions, etc.) can come later.
const double _pushPreviewLabelInset = 4.0;
const double _pushPreviewLabelHorizontalPadding = 6.0;
const double _pushPreviewLabelVerticalPadding = 2.0;
const double _pushPreviewLabelBackgroundAlpha = 0.18;
const double _pushPreviewLabelBorderRadius = 4.0;
const double _pushPreviewLabelFontSize = 10.0;

class PushPreviewOverlay extends StatelessWidget {
  const PushPreviewOverlay({
    required this.previewPlacements,
    required this.activeToolId,
    super.key,
  });

  final List<PlacementDto> previewPlacements;

  /// The tool being moved/resized — skipped when labelling, since its
  /// own position is already shown by the drag feedback (move) or the
  /// span highlight rect (resize). Typed [ToolId] (not a bare String)
  /// so callers can't hand over arbitrary display text. Generalized
  /// from `MoveModeActive` so the SAME overlay serves both the move
  /// push preview (F16) and the resize push preview — no copy-paste
  /// twin.
  final ToolId activeToolId;

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    return IgnorePointer(
      child: Stack(
        clipBehavior: Clip.none,
        children: [
          for (final placement in previewPlacements)
            // Skip the moved/resized tool itself — only displaced
            // placements need labels.
            if (placement.toolId != activeToolId.value)
              Positioned(
                left:
                    BoardGridMetrics.pixelX(placement.x) +
                    _pushPreviewLabelInset,
                top:
                    BoardGridMetrics.pixelY(placement.y) +
                    _pushPreviewLabelInset,
                child: Container(
                  padding: const EdgeInsets.symmetric(
                    horizontal: _pushPreviewLabelHorizontalPadding,
                    vertical: _pushPreviewLabelVerticalPadding,
                  ),
                  decoration: BoxDecoration(
                    color: tokens.accent.withValues(
                      alpha: _pushPreviewLabelBackgroundAlpha,
                    ),
                    border: Border.all(color: tokens.accent),
                    borderRadius: BorderRadius.circular(
                      _pushPreviewLabelBorderRadius,
                    ),
                  ),
                  child: Text(
                    placement.toolId,
                    style: TextStyle(
                      fontSize: _pushPreviewLabelFontSize,
                      color: tokens.accent,
                      fontFamily: upegMonoFontFamily,
                      fontFamilyFallback: upegMonoFontFamilyFallback,
                    ),
                  ),
                ),
              ),
        ],
      ),
    );
  }
}
