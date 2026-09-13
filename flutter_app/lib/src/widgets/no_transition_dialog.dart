/// Dialog helper with zero route transition.
///
/// Delegates to Material [showDialog] so localization, inherited themes,
/// traversal behavior, safe areas, and barrier semantics stay identical to the
/// framework default while the route animation is removed.
library;

import 'package:flutter/material.dart';

Future<T?> showNoTransitionDialog<T>({
  required BuildContext context,
  required WidgetBuilder builder,
  Color barrierColor = Colors.black54,
  bool barrierDismissible = true,
  String? barrierLabel,
}) {
  return showDialog<T>(
    context: context,
    barrierColor: barrierColor,
    barrierDismissible: barrierDismissible,
    barrierLabel: barrierLabel,
    animationStyle: AnimationStyle.noAnimation,
    builder: builder,
  );
}
