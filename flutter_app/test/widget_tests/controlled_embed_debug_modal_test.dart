import 'dart:async';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/features/controlled_embed/session_service.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/widgets/controlled_embed/settings.dart';
import '../test_helpers/controlled_embed_tile_harness.dart';

void main() {
  useStubbedControlledEmbedSeams();

  testWidgets('출력 변환 실패 뒤 디버그를 다시 열거나 기록을 지워도 원시 성공 출력이 나타나지 않는다', (
    tester,
  ) async {
    final canonicalErrorResult = canonicalError('Expected a numeric output.');
    controlledEmbedFixture.normalizeResult = (_, _, _) => canonicalErrorResult;
    final robot = ControlledEmbedDebugRobot(tester);
    await robot.pump();
    controlledEmbedFixture.factory.sessions.single.readPayload =
        '{"result":"not-a-number"}';
    await robot.tapRun();
    robot.expectConsoleContaining('Expected a numeric output.');
    robot.expectNoOutput();
    await robot.tapClose();

    await robot.pump();
    robot.expectConsoleContaining('Expected a numeric output.');
    robot.expectNoOutput();
    await robot.tapClear();
    await controlledEmbedFixture.sessions.ensure(
      ControlledEmbedSessionSpec(
        toolId: ToolId.parse('test.other.session'),
        url: 'https://example.test/other',
      ),
    );
    await robot.finishLoading();

    robot.expectNoOutput();
    final entry = controlledEmbedFixture.sessions.entryFor(
      ToolId.parse(fixtureTool().id),
    )!;
    expect(entry.phase, ControlledEmbedSessionPhase.failed);
    expect(entry.lastResult, same(canonicalErrorResult));
    expect(controlledEmbedFixture.factory.sessions, hasLength(2));
  });

  testWidgets('모달은 앱 세션의 브라우저와 실행 콘솔을 표시한다', (tester) async {
    final robot = ControlledEmbedDebugRobot(tester);
    await robot.pump();
    robot.expectVisible(true);
    robot.expectPanesVisible();
    robot.expectRunEnabled(true);
    expect(controlledEmbedFixture.calls, isEmpty);
  });

  testWidgets('실행 전에 입력 스냅샷을 보여주고 숫자 타입을 유지해 전달한다', (tester) async {
    controlledEmbedFixture.boardKey = 'dev';
    final robot = ControlledEmbedDebugRobot(tester);
    await robot.pump(inputs: {'query': 'hello', 'count': 3});
    robot.expectInputsContaining('hello');
    robot.expectInputsContaining('3');
    expect(controlledEmbedFixture.calls, isEmpty);
    await robot.tapRun();
    final call = controlledEmbedFixture.calls.single;
    expect(call.args.toJsonObject(), {'query': 'hello', 'count': 3});
    expect(call.boardKey, 'dev');
    robot.expectOutputContaining('1');
  });

  testWidgets('페이지 준비가 끝나기 전에는 실행 버튼을 비활성화한다', (tester) async {
    final ready = Completer<void>();
    controlledEmbedFixture.factory.nextReadyGate = ready;
    final robot = ControlledEmbedDebugRobot(tester);
    await robot.pump(settle: false);
    robot.expectRunEnabled(false);
    expect(controlledEmbedFixture.calls, isEmpty);
    ready.complete();
    await robot.finishLoading();
    robot.expectRunEnabled(true);
  });

  testWidgets('재실행은 모달과 페이지를 유지하면서 공통 실행 이벤트를 갱신한다', (tester) async {
    final robot = ControlledEmbedDebugRobot(tester);
    await robot.pump();
    final browser = controlledEmbedFixture.factory.sessions.single;
    await robot.tapRun();
    robot.expectOutputContaining('1');
    await robot.tapRerun();
    robot.expectVisible(true);
    robot.expectOutputContaining('2');
    robot.expectConsoleContaining('Outputs read successfully');
    expect(controlledEmbedFixture.factory.sessions.single, same(browser));
  });

  testWidgets('콘솔 지우기는 결과와 페이지를 유지하고 실행 기록만 지운다', (tester) async {
    final robot = ControlledEmbedDebugRobot(tester);
    await robot.pump();
    await robot.tapRun();
    final entry = controlledEmbedFixture.sessions.entryFor(
      ToolId.parse(fixtureTool().id),
    )!;
    expect(entry.events, isNotEmpty);
    await robot.tapClear();
    expect(entry.events, isEmpty);
    robot.expectVisible(true);
    robot.expectPanesVisible();
    robot.expectOutputContaining('1');
    expect(controlledEmbedFixture.factory.sessions.single.closeCalls, 0);
  });

  testWidgets('모달을 닫아도 세션을 종료하지 않고 디버그 예약만 해제한다', (tester) async {
    final robot = ControlledEmbedDebugRobot(tester);
    await robot.pump();
    final entry = controlledEmbedFixture.sessions.entryFor(
      ToolId.parse(fixtureTool().id),
    )!;
    expect(entry.displayedInDebugger, isTrue);
    await robot.tapClose();
    robot.expectVisible(false);
    expect(entry.displayedInDebugger, isFalse);
    expect(controlledEmbedFixture.factory.sessions.single.closeCalls, 0);
    expect(entry.phase, ControlledEmbedSessionPhase.ready);
  });

  testWidgets('실행 실패는 콘솔에 표시하고 재실행할 수 있다', (tester) async {
    controlledEmbedFixture.executor =
        ({required toolId, required args, boardKey}) async =>
            canonicalError('selector miss: #result');
    final robot = ControlledEmbedDebugRobot(tester);
    await robot.pump();
    await robot.tapRun();
    robot.expectConsoleContaining('selector miss: #result');
    robot.expectRunEnabled(true);
  });

  testWidgets('명시한 브라우저 설정은 별도 페이지 없이 공유 세션에 적용된다', (tester) async {
    const settings = ResolvedBrowserSettings(
      hasUserAgentOverride: true,
      userAgent: 'debug-agent',
      viewportSize: ViewPortPresetSizes.tablet,
    );
    final robot = ControlledEmbedDebugRobot(tester);
    await robot.pump(settings: settings);
    expect(
      controlledEmbedFixture.factory.sessions.single.settings,
      same(settings),
    );
    await robot.tapRun();
    expect(controlledEmbedFixture.factory.sessions, hasLength(1));
    robot.expectOutputContaining('1');
  });
}
