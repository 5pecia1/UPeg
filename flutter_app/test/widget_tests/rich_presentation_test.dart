import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/expanded_modal/rich_presentation.dart';

Widget _harness(RichPresentationView view) => MaterialApp(
  theme: UpegTheme.darkTheme(),
  home: Scaffold(
    body: RichPresentationPanel(
      view: view,
      tokens: UpegTheme.darkTheme().extension<UpegTokens>()!,
    ),
  ),
);

void main() {
  testWidgets(
    'renders fixed rich slots and keeps raw output explicitly available',
    (tester) async {
      await tester.pumpWidget(
        _harness(
          const RichPresentationView(
            title: 'Update preview',
            subtitle: 'Target: sample',
            status: PresentationStatus(
              label: 'Ready to apply',
              tone: PresentationTone.success,
            ),
            summary: [
              PresentationTextValue(label: 'Changed files', value: '3'),
            ],
            notices: [
              PresentationNotice(
                text: 'Environment setup remains.',
                tone: PresentationTone.warning,
              ),
            ],
            detail: PresentationDetail(
              fields: [PresentationTextValue(label: 'Path', value: '.upeg/a')],
              diff: '@@ -1 +1 @@\n-old\n+new',
            ),
            rawJson: '{"legacy":"preserved"}',
          ),
        ),
      );

      expect(find.text('Update preview'), findsOneWidget);
      expect(find.text('Changed files'), findsOneWidget);
      expect(find.text('Environment setup remains.'), findsOneWidget);
      expect(find.byKey(const Key('rich-presentation-diff')), findsOneWidget);
      expect(find.text('{"legacy":"preserved"}'), findsNothing);

      await tester.tap(find.byKey(const Key('rich-presentation-raw')));
      await tester.pumpAndSettle();
      expect(find.text('{"legacy":"preserved"}'), findsOneWidget);
    },
  );

  testWidgets('does not render a panel for an unresolved rich projection', (
    tester,
  ) async {
    await tester.pumpWidget(_harness(const RichPresentationView()));
    expect(find.byKey(const Key('rich-presentation')), findsNothing);
  });

  testWidgets('raw disclosure opens inside a colored result container', (
    tester,
  ) async {
    final tokens = UpegTheme.darkTheme().extension<UpegTokens>()!;
    await tester.pumpWidget(
      MaterialApp(
        theme: UpegTheme.darkTheme(),
        home: Scaffold(
          body: Container(
            color: tokens.bg2,
            child: RichPresentationRaw(raw: '{"ok":true}', tokens: tokens),
          ),
        ),
      ),
    );
    await tester.tap(find.byKey(const Key('rich-presentation-raw')));
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    expect(find.text('{"ok":true}'), findsOneWidget);
  });
}
