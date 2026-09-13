/// Collapsed monospace block for a failure's structured `error.details`.
///
/// A failing External tool ships its exit code and both captured
/// streams in `details`; that can be hundreds of lines of compiler
/// output, so it starts collapsed and the operator opens it when the
/// one-line message is not enough.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/theme/upeg_theme.dart';

class ErrorDetailsBlock extends ConsumerStatefulWidget {
  const ErrorDetailsBlock({
    required this.details,
    required this.tokens,
    super.key,
  });

  final String details;
  final UpegTokens tokens;

  @override
  ConsumerState<ErrorDetailsBlock> createState() => _ErrorDetailsBlockState();
}

class _ErrorDetailsBlockState extends ConsumerState<ErrorDetailsBlock> {
  bool _expanded = false;

  @override
  Widget build(BuildContext context) {
    final tokens = widget.tokens;
    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          InkWell(
            key: const ValueKey('expanded-modal-error-details-toggle'),
            onTap: () => setState(() => _expanded = !_expanded),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(
                  _expanded ? Icons.expand_more : Icons.chevron_right,
                  size: 12,
                  color: tokens.fg4,
                ),
                const SizedBox(width: 2),
                Text(
                  t(ref, 'modal.outcome.details'),
                  style: TextStyle(
                    fontFamily: upegMonoFontFamily,
                    fontFamilyFallback: upegMonoFontFamilyFallback,
                    fontSize: 10,
                    color: tokens.fg4,
                  ),
                ),
              ],
            ),
          ),
          if (_expanded) ...[
            const SizedBox(height: 2),
            SelectableText(
              widget.details,
              key: const ValueKey('expanded-modal-error-details-body'),
              style: TextStyle(
                fontFamily: upegMonoFontFamily,
                fontFamilyFallback: upegMonoFontFamilyFallback,
                fontSize: 11,
                color: tokens.fg2,
              ),
            ),
          ],
        ],
      ),
    );
  }
}
