import 'package:flutter_test/flutter_test.dart';
import '../test_helpers/image_conversion_robot.dart';

void main() {
  testWidgets(
    'switching_the_format_shows_only_relevant_options_and_never_sends_a_hidden_error',
    (tester) async {
      final robot = ImageConversionRobot(tester);
      await robot.pumpForm();
      robot.expectJpegNotVisible();
      robot.expectSvgNotVisible();
      await robot.selectFormat('JPEG');
      robot.expectJpegVisible();
      await robot.enterInvalidBackground();
      await robot.selectFormat('PNG');
      robot.expectJpegNotVisible();
      robot.expectValidWithoutHiddenBackground();
    },
  );
  testWidgets('the_svg_width_option_stays_visible_on_a_narrow_dark_screen', (
    tester,
  ) async {
    final robot = ImageConversionRobot(tester);
    await robot.pumpForm(svg: true, dark: true);
    robot.expectSvgVisible();
    robot.expectNoLayoutErrors();
  });
  testWidgets(
    'the_image_can_be_previewed_and_saved_while_double_saves_are_blocked',
    (tester) async {
      final robot = ImageConversionRobot(tester);
      await robot.pumpOutput(pending: true);
      robot.expectPreviewVisible();
      await robot.save();
      robot.expectSaving();
      await robot.finishSave();
      robot.expectSaved();
    },
  );
  testWidgets('a_failed_save_can_be_retried', (tester) async {
    final robot = ImageConversionRobot(tester);
    await robot.pumpOutput(fail: true);
    await robot.save();
    robot.expectSaveFailed();
    robot.expectNotSaved();
    await robot.retrySave();
    robot.expectSaved();
    robot.expectSaveNotFailed();
  });
  testWidgets('a_cancelled_save_is_not_reported_as_done', (tester) async {
    final robot = ImageConversionRobot(tester);
    await robot.pumpOutput(cancel: true, mime: 'image/tiff');
    robot.expectPreviewNotVisible();
    await robot.save();
    robot.expectNotSaved();
    robot.expectSaveNotFailed();
  });
}
