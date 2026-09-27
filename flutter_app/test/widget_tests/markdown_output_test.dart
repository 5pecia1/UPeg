import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/expanded_modal/structured_output.dart';

void main() {
  testWidgets(
    'renders headings links tables and fenced code through Markdown',
    (tester) async {
      final tokens = UpegTheme.darkTheme().extension<UpegTokens>()!;
      await tester.pumpWidget(
        MaterialApp(
          theme: UpegTheme.darkTheme(),
          home: Scaffold(
            body: MarkdownOutput(
              tokens: tokens,
              markdown: '''# Knowledge

See [reference](https://example.test/docs).

| State | Count |
| --- | ---: |
| Ready | 3 |

```toml
mode = "safe"
```''',
            ),
          ),
        ),
      );

      expect(
        find.byKey(const Key('structured-output-markdown')),
        findsOneWidget,
      );
      expect(find.text('Knowledge'), findsOneWidget);
      expect(
        tester
            .widgetList<SelectableText>(find.byType(SelectableText))
            .any(
              (widget) =>
                  widget.textSpan?.toPlainText().contains('reference') ?? false,
            ),
        isTrue,
      );
      expect(find.text('State'), findsOneWidget);
      expect(find.text('Ready'), findsOneWidget);
      expect(find.text('mode = "safe"'), findsOneWidget);
    },
  );
}
