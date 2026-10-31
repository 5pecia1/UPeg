import 'package:flutter_test/flutter_test.dart';

import '../test_helpers/pin_header_open_robot.dart';

void main() {
  testWidgets('should open the modal from the underlined header title', (
    tester,
  ) async {
    final robot = PinHeaderOpenRobot(tester);
    await robot.pumpPin();
    robot.expectOpenTitleVisible();

    await robot.tapOpenTitle();

    robot.expectOpenedWithoutRun();
  });

  testWidgets('should open the focused header title with Enter', (
    tester,
  ) async {
    final robot = PinHeaderOpenRobot(tester);
    await robot.pumpPin();

    await robot.pressOpenTitle();

    robot.expectOpenedWithoutRun();
  });
}
