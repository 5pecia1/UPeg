/// Per-pin context menu vocabulary: the actions the menu can return and
/// the row widget that renders one entry with its keyboard mirror.
library;

import 'package:flutter/material.dart';
import 'package:upeg/src/theme/upeg_theme.dart';

/// The closed set of actions the per-pin context menu can return
/// (inventory row F03). `pin.dart` is its only caller today, but nothing
/// here enforces that: this is a public symbol of a public library, and
/// Dart has no visibility modifier that would say otherwise. What the
/// enum *is* is a vocabulary — it names choices, it does not perform
/// them. The write itself stays behind `pegboardMutationsProvider`, so a
/// second caller could reuse the menu without reaching around the
/// mutation seam.
enum PinMenuAction { open, editColor, resetSize, unpin }

/// Context-menu row: entry label plus its keyboard mirror (looked up
/// from the shared binding catalog) right-aligned as a dim mono cap.
/// No flex children — `showMenu` measures items with `IntrinsicWidth`,
/// which rejects `Spacer`/`Expanded`.
class PinMenuRow extends StatelessWidget {
  const PinMenuRow({
    required this.tokens,
    required this.label,
    required this.keyCap,
    super.key,
  });

  final UpegTokens tokens;
  final String label;
  final String? keyCap;

  @override
  Widget build(BuildContext context) {
    final cap = keyCap;
    return Row(
      mainAxisAlignment: MainAxisAlignment.spaceBetween,
      children: [
        Text(label),
        if (cap != null)
          Padding(
            padding: const EdgeInsets.only(left: 24),
            child: Text(
              cap,
              style: TextStyle(
                fontFamily: upegMonoFontFamily,
                fontFamilyFallback: upegMonoFontFamilyFallback,
                fontSize: 11,
                color: tokens.fg4,
              ),
            ),
          ),
      ],
    );
  }
}
