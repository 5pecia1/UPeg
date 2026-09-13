import 'package:flutter_test/flutter_test.dart';

import '../test_helpers/board_details_robot.dart';

void main() {
  testWidgets('보드 상단의 상세 버튼으로 선택한 보드 지침을 연다', (tester) async {
    final robot = BoardDetailsRobot(tester);
    await robot.open(throughTabs: true);
    robot.expectEditorText('검토용 보드', '# 점검\n먼저 상태를 확인한다.');
  });

  testWidgets('개인 보드의 설명과 Markdown 지침을 저장한다', (tester) async {
    final robot = BoardDetailsRobot(tester);
    await robot.open();
    await robot.editGuidance('프로젝트 점검', '# 순서\n1. 먼저 상태를 확인한다.');
    await robot.save();
    robot.expectSavedGuidance('프로젝트 점검', '# 순서\n1. 먼저 상태를 확인한다.');
    robot.expectReconnectNotice();
  });

  testWidgets('프로젝트 보드는 원본 경로와 읽기 전용 지침을 표시한다', (tester) async {
    final robot = BoardDetailsRobot(tester);
    await robot.open(project: true);
    robot.expectProjectGuidance();
    robot.expectSaveUnavailable();
  });

  testWidgets('프로젝트 원본이 바뀌면 이전 지침 대신 재시작 안내를 표시한다', (tester) async {
    final robot = BoardDetailsRobot(tester);
    await robot.open(projectChanged: true);
    robot.expectProjectRestartRequired();
  });

  testWidgets('연결 설정 복사는 실행 폴더와 보드를 보존하며 연결 완료로 표시하지 않는다', (tester) async {
    final robot = BoardDetailsRobot(tester);
    await robot.open();
    await robot.openConnection();
    await robot.expectEffectiveTools();
    await robot.copyConfiguration();
    robot.expectBoundConfigurationCopied();
    robot.expectConnectionNotConfirmed();
  });

  testWidgets('저장 실패 시 성공 메시지 없이 입력을 유지한다', (tester) async {
    final robot = BoardDetailsRobot(tester);
    await robot.open(saveFails: true);
    await robot.editGuidance('보존할 설명', '# 아직 저장되지 않음');
    await robot.save();
    robot.expectSaveFailure();
    robot.expectEditorText('보존할 설명', '# 아직 저장되지 않음');
  });

  testWidgets('웹에서는 로컬 MCP 연결 설정을 제공하지 않는다', (tester) async {
    final robot = BoardDetailsRobot(tester);
    await robot.open(native: false);
    robot.expectNativeConnectionUnavailable();
  });

  testWidgets('웹 저장은 브라우저 저장 결과만 표시한다', (tester) async {
    final robot = BoardDetailsRobot(tester);
    await robot.open(native: false);
    await robot.save();
    robot.expectBrowserSave();
  });

  testWidgets('CLI 실행 파일이 없으면 연결 준비가 되지 않았음을 표시한다', (tester) async {
    final robot = BoardDetailsRobot(tester);
    await robot.open(cliAvailable: false);
    await robot.openConnection();
    robot.expectCliUnavailable();
  });

  testWidgets('미로딩 핀은 새 도구 추가 대신 준비 상태 확인을 안내한다', (tester) async {
    final robot = BoardDetailsRobot(tester);
    await robot.open(unresolvedPins: true);
    await robot.openConnection();
    robot.expectUnresolvedPinsNotice();
  });

  testWidgets('클립보드 실패를 복사 성공으로 표시하지 않는다', (tester) async {
    final robot = BoardDetailsRobot(tester);
    await robot.open(copyFails: true);
    await robot.openConnection();
    await robot.copyConfiguration();
    robot.expectCopyFailure();
  });
}
