import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import '../test_helpers/controlled_embed_tile_harness.dart';

void main() {
  useStubbedControlledEmbedSeams();

  testWidgets('브라우저를 지원하지 않는 환경은 실행을 막고 외부 열기를 안내한다', (tester) async {
    controlledEmbedFixture.nativeSupported = false;
    final robot = ControlledEmbedTileRobot(tester);
    await robot.pump();
    robot.expectRunEnabled(false);
    robot.expectDebugEnabled(false);
    robot.expectUnsupported(true);
    expect(controlledEmbedFixture.factory.sessions, isEmpty);
  });

  testWidgets('지원 환경에서는 실행과 디버그를 사용할 수 있다', (tester) async {
    final robot = ControlledEmbedTileRobot(tester);
    await robot.pump();
    robot.expectRunEnabled(true);
    robot.expectDebugEnabled(true);
    robot.expectUnsupported(false);
  });

  testWidgets('디버그를 열면 앱 세션을 표시하고 도구는 자동으로 실행하지 않는다', (tester) async {
    final robot = ControlledEmbedTileRobot(tester);
    await robot.pump();
    await robot.tapDebug();
    ControlledEmbedDebugRobot(tester).expectVisible(true);
    expect(controlledEmbedFixture.calls, isEmpty);
    expect(controlledEmbedFixture.factory.sessions, hasLength(1));
    final entry = controlledEmbedFixture.sessions.entryFor(
      ToolId.parse(fixtureTool().id),
    )!;
    expect(entry.displayedInDebugger, isTrue);
    await ControlledEmbedDebugRobot(tester).tapClose();
    expect(entry.displayedInDebugger, isFalse);
    expect(controlledEmbedFixture.factory.sessions.single.closeCalls, 0);
  });

  testWidgets('일반 실행 후 디버그를 열어도 브라우저와 페이지 상태가 유지된다', (tester) async {
    final robot = ControlledEmbedTileRobot(tester);
    await robot.pump();
    await robot.tapRun();
    final browser = controlledEmbedFixture.factory.sessions.single;
    await robot.tapDebug();
    final debug = ControlledEmbedDebugRobot(tester);
    debug.expectVisible(true);
    debug.expectOutputContaining('1');
    await debug.tapRun();
    debug.expectOutputContaining('2');
    expect(controlledEmbedFixture.factory.sessions.single, same(browser));
    expect(browser.triggerCount, 2);
    await debug.tapClose();
    robot.expectOutputContaining('2');
  });

  testWidgets('폼의 숫자와 문자열을 타입과 보드 맥락을 유지해 디스패처에 전달한다', (tester) async {
    controlledEmbedFixture.boardKey = 'dev';
    controlledEmbedFixture.executor =
        ({required toolId, required args, boardKey}) async =>
            canonicalSuccess({'intro': 'done'});
    final tool = fixtureTool(
      inputFields: const [
        InputFieldDto(
          key: 'query',
          label: 'Query',
          fieldType: InputFieldType.text(),
          required_: true,
        ),
        InputFieldDto(
          key: 'count',
          label: 'Count',
          fieldType: InputFieldType.integer(),
          required_: true,
        ),
      ],
    );
    final robot = ControlledEmbedTileRobot(tester);
    await robot.pump(tool: tool);
    robot.expectRunEnabled(false);
    await robot.enterField('query', '한글 검색어');
    await robot.enterField('count', '3');
    robot.expectRunEnabled(true);
    await robot.tapRun();
    final call = controlledEmbedFixture.calls.single;
    expect(call.toolId, ToolId.parse(tool.id));
    expect(call.args.toJsonObject(), {'query': '한글 검색어', 'count': 3});
    expect(call.boardKey, 'dev');
    robot.expectOutputRow(label: 'Intro', value: 'done');
  });
}
