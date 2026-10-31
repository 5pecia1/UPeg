import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/theme/upeg_theme.dart';

import '../test_helpers/pin_header_robot.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async {
    final fonts = FontLoader(upegMonoFontFamily)
      ..addFont(rootBundle.load('fonts/JetBrainsMono-Regular.ttf'))
      ..addFont(rootBundle.load('fonts/JetBrainsMono-Medium.ttf'));
    await fonts.load();
  });

  for (final light in [false, true]) {
    for (final operation in ['encode', 'decode']) {
      testWidgets(
        'should distinguish Base64 $operation at 168 pixels in ${light ? "light" : "dark"} mode',
        (tester) async {
          final robot = PinHeaderRobot(tester);
          final id = 'convert.base64_$operation';
          final title = 'Base64 $operation';
          await robot.pumpHeader(
            width: 168,
            label: title,
            id: id,
            light: light,
          );

          robot.expectTitleFits(title);
          robot.expectToolkitNotVisible(id);
        },
      );
    }
  }
  testWidgets('should retain the toolkit badge in a wider pin', (tester) async {
    final robot = PinHeaderRobot(tester);
    await robot.pumpHeader(
      width: 220,
      label: 'Base64 decode',
      id: 'convert.base64_decode',
    );

    robot.expectTitleFits('Base64 decode');
    robot.expectToolkitVisible('convert.base64_decode');
  });

  for (final operation in ['encode', 'decode']) {
    testWidgets(
      'should keep Base64 $operation visible with an open title and move handle',
      (tester) async {
        final robot = PinHeaderRobot(tester);
        await robot.pumpHeader(
          width: 168,
          label: 'Base64 $operation',
          id: 'convert.base64_$operation',
          showMoveHandle: true,
          onOpen: () {},
        );

        robot.expectTitleFits('Base64 $operation');
      },
    );
  }
}
