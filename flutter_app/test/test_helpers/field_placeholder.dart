/// Reads a compact form field's placeholder as it is drawn.
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

/// The placeholder of the field keyed `field-[fieldKey]`: the text of
/// everything drawn inside the field, in tree order. A required field draws
/// its text and its required marker as separate pieces of a hint widget, so
/// `InputDecoration.hintText` says nothing about what the user sees.
String drawnPlaceholder(WidgetTester tester, String fieldKey) => tester
    .widgetList<Text>(
      find.descendant(
        of: find.byKey(Key('field-$fieldKey')),
        matching: find.byType(Text),
      ),
    )
    .map((text) => text.data ?? '')
    .join();
