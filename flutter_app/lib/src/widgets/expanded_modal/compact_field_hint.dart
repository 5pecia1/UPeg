/// The placeholder of a compact (inline pin) form field.
///
/// A placeholder is one line: wrapping would grow the field and push the Run
/// button out of the pin. The required marker sits outside the ellipsis, so a
/// long label or description never cuts it off.
library;

import 'package:flutter/material.dart';

/// A compact placeholder never wraps: a long text would otherwise grow the
/// field and push the Run button out of the pin.
const int _hintMaxLines = 1;

/// Drawn after the label or description of a required field.
const String requiredFieldMarker = ' *';

/// What a compact field asks the user to enter: the [text] (the field's
/// description, or its label when it declares none) and, for a required
/// field, the required marker.
class CompactFieldHint extends StatelessWidget {
  const CompactFieldHint({
    required this.text,
    required this.isRequired,
    required this.style,
    super.key,
  });

  final String text;
  final bool isRequired;
  final TextStyle? style;

  @override
  Widget build(BuildContext context) {
    // One label, as a single placeholder string would give: the two pieces
    // below would otherwise be announced as separate lines.
    return Semantics(
      label: isRequired ? '$text$requiredFieldMarker' : text,
      excludeSemantics: true,
      child: Row(
        children: [
          Flexible(
            child: Text(
              text,
              maxLines: _hintMaxLines,
              overflow: TextOverflow.ellipsis,
              style: style,
            ),
          ),
          if (isRequired) Text(requiredFieldMarker, style: style),
        ],
      ),
    );
  }
}
