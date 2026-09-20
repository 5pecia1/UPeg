import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/widgets/controlled_embed/runner.dart';
import '../test_helpers/controlled_embed_tile_harness.dart';

void main() {
  useStubbedControlledEmbedSeams();

  testWidgets('a wait timeout shows the dispatcher message verbatim', (
    tester,
  ) async {
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

  testWidgets('a result after waiting also shows in the shared result area', (
    tester,
  ) async {
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
