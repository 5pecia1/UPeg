import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/widgets/controlled_embed/runner.dart';
import '../test_helpers/controlled_embed_tile_harness.dart';

void main() {
  useStubbedControlledEmbedSeams();

  testWidgets('실행 전에는 안내만 보여주고 브라우저 세션을 만들지 않는다', (tester) async {
    final robot = ControlledEmbedTileRobot(tester);
    await robot.pump();
    robot.expectRunHint();
    expect(controlledEmbedFixture.calls, isEmpty);
    expect(controlledEmbedFixture.factory.sessions, isEmpty);
  });

  testWidgets('실행 결과를 선언된 라벨과 값으로 표시한다', (tester) async {
    controlledEmbedFixture.executor =
        ({required toolId, required args, boardKey}) async =>
            canonicalSuccess({'intro': 'short article preview'});
    final robot = ControlledEmbedTileRobot(tester);
    await robot.pump();
    await robot.tapRun();
    expect(controlledEmbedFixture.calls.single.args.isEmpty, isTrue);
    robot.expectOutputRow(label: 'Intro', value: 'short article preview');
  });

  testWidgets('디스패처 오류를 결과 영역에 표시한다', (tester) async {
    controlledEmbedFixture.executor =
        ({required toolId, required args, boardKey}) async =>
            canonicalError('selector miss: #intro');
    final robot = ControlledEmbedTileRobot(tester);
    await robot.pump();
    await robot.tapRun();
    robot.expectErrorContaining('selector miss');
  });

  testWidgets('불투명한 자바스크립트 오류도 원문을 보존해 표시한다', (tester) async {
    controlledEmbedFixture.executor =
        ({required toolId, required args, boardKey}) async =>
            canonicalError(kOpaqueObservedError);
    final robot = ControlledEmbedTileRobot(tester);
    await robot.pump();
    await robot.tapRun();
    robot.expectErrorContaining(kOpaqueObservedError);
  });

  testWidgets('실행기 예외를 표시한 뒤 실행 버튼을 다시 활성화한다', (tester) async {
    controlledEmbedFixture.executor =
        ({required toolId, required args, boardKey}) async =>
            throw StateError('executor exploded');
    final robot = ControlledEmbedTileRobot(tester);
    await robot.pump();
    await robot.tapRun();
    robot.expectErrorContaining('executor exploded');
    robot.expectRunEnabled(true);
  });

  testWidgets('정규화된 오류는 플랫폼 내부 문구 없이 표시한다', (tester) async {
    controlledEmbedFixture.executor =
        ({required toolId, required args, boardKey}) async =>
            canonicalError(kControlledEmbedNullJavaScriptResultMessage);
    final robot = ControlledEmbedTileRobot(tester);
    await robot.pump();
    await robot.tapRun();
    robot.expectErrorContaining(kControlledEmbedNullJavaScriptResultMessage);
    robot.expectNoTextContaining('Invalid argument(s)');
    robot.expectNoTextContaining('platform-channels');
  });

  testWidgets('재실행이 성공하면 이전 오류를 결과로 교체한다', (tester) async {
    var attempts = 0;
    controlledEmbedFixture.executor =
        ({required toolId, required args, boardKey}) async {
          attempts++;
          return attempts == 1
              ? canonicalError('first error')
              : canonicalSuccess({'result': kRecoveredOutput});
        };
    final robot = ControlledEmbedTileRobot(tester);
    await robot.pump();
    await robot.tapRun();
    robot.expectErrorContaining('first error');
    await robot.tapRun();
    robot.expectNoError();
    robot.expectOutputContaining(kRecoveredOutput);
  });

  testWidgets('브라우저 연결 오류는 타일의 오류 영역에서 확인할 수 있다', (tester) async {
    controlledEmbedFixture.executor =
        ({required toolId, required args, boardKey}) async =>
            throw StateError('The browser session is unavailable.');
    final robot = ControlledEmbedTileRobot(tester);
    await robot.pump();
    await robot.tapRun();
    robot.expectErrorContaining('browser session is unavailable');
    robot.expectRunEnabled(true);
  });
}
