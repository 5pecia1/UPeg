import 'package:flutter_test/flutter_test.dart';
import '../test_helpers/image_conversion_robot.dart';

void main() {
  for (final imageConversion in [true, false]) {
    testWidgets(
      'should show only the appropriate modal description for ${imageConversion ? "image conversion" : "other tools"}',
      (tester) async {
        final robot = ImageConversionRobot(tester);
        await robot.pumpModal(imageConversion: imageConversion);
        robot.expectModalDescription(imageConversion: imageConversion);
      },
    );
  }

  testWidgets('selecting_a_heic_input_keeps_the_chosen_output_format_in_args', (
    tester,
  ) async {
    final robot = ImageConversionRobot(tester);
    await robot.pumpForm(heicInput: true);
    await robot.pickHeic();
    await robot.selectFormat('JPEG');

    robot.expectHeicSubmittedAs('jpeg');
  });

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
  for (final batch in [false, true]) {
    testWidgets(
      'should show SVG options and AI DPI for ${batch ? "batch" : "single"} conversion',
      (tester) async {
        final robot = ImageConversionRobot(tester);
        await robot.pumpForm(ai: true, batch: batch);
        robot.expectVectorOptions(visible: false, ai: true);
        await robot.selectFormat('SVG');
        robot.expectVectorOptions(visible: true, ai: true);
        await robot.selectFormat('PNG');
        robot.expectVectorOptions(visible: false, ai: true);
      },
    );
  }
  testWidgets('should save SVG with its own extension', (tester) async {
    final robot = ImageConversionRobot(tester);
    await robot.pumpOutput(mime: 'image/svg+xml', name: 'result.svg');
    robot.expectPreviewVisible();
    await robot.save();
    robot.expectSvgSaved();
  });
  testWidgets(
    'should reject external SVG content from preview while allowing save',
    (tester) async {
      final robot = ImageConversionRobot(tester);
      await robot.pumpOutput(
        mime: 'image/svg+xml',
        name: 'result.svg',
        svgContent:
            '<svg xmlns="http://www.w3.org/2000/svg"><image href="https://example.com/a.png"/></svg>',
      );
      robot.expectSvgUnavailable();
      await robot.save();
      robot.expectSvgSaved();
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
