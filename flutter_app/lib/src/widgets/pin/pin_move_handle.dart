import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/rust/api/tweaks.dart';
import 'package:upeg/src/state/tweaks_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/pin/pin_keys.dart';

const double pinMoveHandleExtent = 13;

/// Small selectable move affordance shared by both pin drag surfaces.
class PinMoveHandle extends ConsumerWidget {
  const PinMoveHandle({required this.tokens, super.key});

  final UpegTokens tokens;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final dots = ref.watch(pinHandleStyleProvider) == PinHandleStyleDto.dots;
    return MouseRegion(
      cursor: SystemMouseCursors.grab,
      child: Tooltip(
        message: pinMoveTooltip,
        child: dots
            ? Icon(
                Icons.drag_indicator,
                key: pinMoveHandleKey,
                size: pinMoveHandleExtent,
                color: tokens.fg3,
              )
            : CustomPaint(
                key: pinMoveHandleKey,
                size: const Size(pinMoveHandleExtent, pinMoveHandleExtent),
                painter: PinMoveHandlePainter(
                  head: tokens.pin.withValues(alpha: 0.75),
                  stem: tokens.fg3,
                ),
              ),
      ),
    );
  }
}

class PinMoveHandlePainter extends CustomPainter {
  const PinMoveHandlePainter({required this.head, required this.stem});

  final Color head;
  final Color stem;

  @override
  void paint(Canvas canvas, Size size) {
    canvas.drawRRect(
      RRect.fromRectAndRadius(
        Rect.fromLTWH((size.width - 11) / 2, 0, 11, 4),
        const Radius.circular(2),
      ),
      Paint()..color = head,
    );
    canvas.drawRRect(
      RRect.fromRectAndRadius(
        Rect.fromLTWH((size.width - 2.5) / 2, 4, 2.5, 6),
        const Radius.circular(1),
      ),
      Paint()..color = stem,
    );
  }

  @override
  bool shouldRepaint(PinMoveHandlePainter oldDelegate) =>
      head != oldDelegate.head || stem != oldDelegate.stem;
}
