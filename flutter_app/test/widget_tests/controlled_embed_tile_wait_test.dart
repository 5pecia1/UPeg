import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/widgets/controlled_embed/runner.dart';
import '../test_helpers/controlled_embed_tile_harness.dart';

void main() {
  useStubbedControlledEmbedSeams();

  testWidgets('대기 시간 초과는 디스패처가 반환한 메시지 그대로 표시한다', (tester) async {
    const message =
        'wait for input element to exist timed out at "#ready" after 5000ms';
    controlledEmbedFixture.executor =
        ({required toolId, required args, boardKey}) async =>
            const CanonicalToolResult(
              ok: false,
              outputs: [],
              error: CanonicalToolError(
                code: kControlledEmbedWaitTimeoutCode,
                message: message,
              ),
            );
    final robot = ControlledEmbedTileRobot(tester);
    await robot.pump();
    await robot.tapRun();
    robot.expectErrorContaining(message);
    robot.expectRunEnabled(true);
  });

  testWidgets('대기를 마친 결과도 공통 결과 영역에 표시한다', (tester) async {
    controlledEmbedFixture.executor =
        ({required toolId, required args, boardKey}) async =>
            canonicalSuccess({'intro': 'waited successfully'});
    final robot = ControlledEmbedTileRobot(tester);
    await robot.pump();
    await robot.tapRun();
    robot.expectNoError();
    robot.expectOutputRow(label: 'Intro', value: 'waited successfully');
  });
}
