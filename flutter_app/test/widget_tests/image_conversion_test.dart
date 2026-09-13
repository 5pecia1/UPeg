import 'package:flutter_test/flutter_test.dart';
import '../test_helpers/image_conversion_robot.dart';

void main() {
  testWidgets('형식을_바꾸면_필요한_옵션만_보이고_숨겨진_오류는_전송하지_않는다', (tester) async {
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
  });
  testWidgets('에스브이지_폭_옵션은_좁은_다크_화면에서도_보인다', (tester) async {
    final robot = ImageConversionRobot(tester);
    await robot.pumpForm(svg: true, dark: true);
    robot.expectSvgVisible();
    robot.expectNoLayoutErrors();
  });
  testWidgets('이미지를_미리_보고_저장하며_중복_저장을_막는다', (tester) async {
    final robot = ImageConversionRobot(tester);
    await robot.pumpOutput(pending: true);
    robot.expectPreviewVisible();
    await robot.save();
    robot.expectSaving();
    await robot.finishSave();
    robot.expectSaved();
  });
  testWidgets('저장_실패_후_다시_시도할_수_있다', (tester) async {
    final robot = ImageConversionRobot(tester);
    await robot.pumpOutput(fail: true);
    await robot.save();
    robot.expectSaveFailed();
    robot.expectNotSaved();
    await robot.retrySave();
    robot.expectSaved();
    robot.expectSaveNotFailed();
  });
  testWidgets('저장을_취소하면_완료로_표시하지_않는다', (tester) async {
    final robot = ImageConversionRobot(tester);
    await robot.pumpOutput(cancel: true, mime: 'image/tiff');
    robot.expectPreviewNotVisible();
    await robot.save();
    robot.expectNotSaved();
    robot.expectSaveNotFailed();
  });
}
