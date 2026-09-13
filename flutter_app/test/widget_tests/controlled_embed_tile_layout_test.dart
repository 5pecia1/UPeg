import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/theme/upeg_theme.dart' show UpegSizing;
import '../test_helpers/controlled_embed_tile_harness.dart';

void main() {
  useStubbedControlledEmbedSeams();

  testWidgets('긴 출력은 생략 없이 선택 가능한 텍스트로 표시한다', (tester) async {
    controlledEmbedFixture.executor =
        ({required toolId, required args, boardKey}) async =>
            canonicalSuccess({'intro': kLongControlledEmbedOutput});
    final robot = ControlledEmbedTileRobot(tester);
    await robot.pump();
    await robot.tapRun();
    robot.expectSelectableOutput('intro', kLongControlledEmbedOutput);
  });

  testWidgets('출력 복사 버튼은 해당 값 전체를 클립보드에 전달한다', (tester) async {
    const fullValue = 'hello world copy test';
    final writer = RecordingClipboardWriterForTile();
    controlledEmbedFixture.executor =
        ({required toolId, required args, boardKey}) async =>
            canonicalSuccess({'intro': fullValue});
    final robot = ControlledEmbedTileRobot(tester);
    await robot.pump(clipboard: writer);
    await robot.tapRun();
    await robot.copyOutput('intro');
    expect(writer.writes, [fullValue]);
  });

  testWidgets('오류도 생략 없이 선택 가능한 텍스트로 표시한다', (tester) async {
    controlledEmbedFixture.executor =
        ({required toolId, required args, boardKey}) async =>
            canonicalError(kSelectorMissControlledEmbedError);
    final robot = ControlledEmbedTileRobot(tester);
    await robot.pump();
    await robot.tapRun();
    robot.expectSelectableError(kSelectorMissControlledEmbedError);
  });

  testWidgets('타일 폭이 좁아져도 브라우저를 만들지 않고 실행 버튼을 배치한다', (tester) async {
    final robot = ControlledEmbedTileRobot(tester);
    await robot.pump(width: UpegSizing.pinCellWidth, height: 240);
    robot.expectNoFrameworkError();
    robot.expectRunEnabled(true);
    await robot.pump(width: 80, height: 240);
    robot.expectNoFrameworkError();
    robot.expectDebugEnabled(true);
    expect(controlledEmbedFixture.factory.sessions, isEmpty);
  });
}
