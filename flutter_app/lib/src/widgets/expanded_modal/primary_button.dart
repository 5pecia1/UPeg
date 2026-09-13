/// Accent-filled primary action button for the expanded modal.
///
/// Lives outside `expanded_modal_page.dart` so the page stays inside the
/// hand-written Dart file budget; it is shared by the Run action and the
/// in-flight Cancel action, which differ only in colour and label.
library;

import 'package:flutter/material.dart';

import 'package:upeg/src/theme/upeg_theme.dart';

class ExpandedModalPrimaryButton extends StatelessWidget {
  const ExpandedModalPrimaryButton({
    required this.label,
    required this.onPressed,
    this.trailingHint,
    this.enabled = true,
    this.loading = false,
    super.key,
  });

  final String label;
  final String? trailingHint;
  final VoidCallback onPressed;

  /// When `false`, the InkWell ignores taps and the accent dims so the
  /// user sees the field validators block dispatch.
  final bool enabled;

  /// True while the tool is dispatching — shows an inline spinner so the
  /// user gets in-flight feedback instead of a frozen button.
  final bool loading;

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    final tintedBg = enabled
        ? tokens.accent
        : tokens.accent.withValues(alpha: 0.4);
    return InkWell(
      onTap: enabled ? onPressed : null,
      borderRadius: BorderRadius.circular(UpegSizing.radius1),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
        decoration: BoxDecoration(
          color: tintedBg,
          borderRadius: BorderRadius.circular(UpegSizing.radius1),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            if (loading) ...[
              SizedBox(
                key: const Key('expanded-modal-run-progress'),
                width: 12,
                height: 12,
                child: CircularProgressIndicator(
                  strokeWidth: 2,
                  valueColor: AlwaysStoppedAnimation<Color>(tokens.onAccent),
                ),
              ),
              const SizedBox(width: 8),
            ],
            Text(
              label,
              style: TextStyle(
                fontFamily: upegMonoFontFamily,
                fontFamilyFallback: upegMonoFontFamilyFallback,
                fontSize: 12,
                color: tokens.onAccent,
                fontWeight: FontWeight.w600,
              ),
            ),
            if (trailingHint != null) ...[
              const SizedBox(width: 6),
              Text(
                trailingHint!,
                style: TextStyle(
                  fontFamily: upegMonoFontFamily,
                  fontFamilyFallback: upegMonoFontFamilyFallback,
                  fontSize: 10,
                  color: tokens.onAccent,
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}
