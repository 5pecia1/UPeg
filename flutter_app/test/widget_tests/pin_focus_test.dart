import 'package:flutter_test/flutter_test.dart';

import '../test_helpers/pin_focus_robot.dart';

void main() {
  testWidgets('should focus the inline pin on field pointer entry', (
    tester,
  ) async {
    final robot = PinFocusRobot(tester);
    await robot.pumpInlineBoard();
    robot.expectNoFocusedPin();

    await robot.tapFirstField();
    robot.expectFirstFocused();

    robot.focusSecondLogically();
    await robot.tapFirstField();
    robot.expectFirstFocused();
  });

  testWidgets(
    'should focus the inline pin when its field gains keyboard focus',
    (tester) async {
      final robot = PinFocusRobot(tester);
      await robot.pumpInlineBoard();
      robot.focusSecondLogically();

      await robot.focusFirstFieldWithKeyboard();

      robot.expectFirstFocused();
    },
  );

  testWidgets('should describe the inline pin tap as opening its full view', (
    tester,
  ) async {
    final robot = PinFocusRobot(tester);
    await robot.pumpInlineBoard();

    robot.expectFirstPinOpenHint();
  });

  testWidgets(
    'should keep resize tooltip on handles without extra focus nodes',
    (tester) async {
      final robot = PinFocusRobot(tester);
      await robot.pumpInlineBoard();

      robot.expectResizeTooltipOnlyOnHandles();
    },
  );
}
