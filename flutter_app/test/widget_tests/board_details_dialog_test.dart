import 'package:flutter_test/flutter_test.dart';

import '../test_helpers/board_details_robot.dart';

void main() {
  testWidgets(
    'the board header details button opens guidance for the selected board',
    (tester) async {
      final robot = BoardDetailsRobot(tester);
      await robot.open(throughTabs: true);
      robot.expectEditorText('검토용 보드', '# 점검\n먼저 상태를 확인한다.');
    },
  );

  testWidgets('saves a personal board description and Markdown guidance', (
    tester,
  ) async {
    final robot = BoardDetailsRobot(tester);
    await robot.open();
    await robot.editGuidance('프로젝트 점검', '# 순서\n1. 먼저 상태를 확인한다.');
    await robot.save();
    robot.expectSavedGuidance('프로젝트 점검', '# 순서\n1. 먼저 상태를 확인한다.');
    robot.expectReconnectNotice();
  });

  testWidgets('a project board shows its source path and read-only guidance', (
    tester,
  ) async {
    final robot = BoardDetailsRobot(tester);
    await robot.open(project: true);
    robot.expectProjectGuidance();
    robot.expectSaveUnavailable();
  });

  testWidgets(
    'a changed project source shows restart guidance instead of stale instructions',
    (tester) async {
      final robot = BoardDetailsRobot(tester);
      await robot.open(projectChanged: true);
      robot.expectProjectRestartRequired();
    },
  );

  testWidgets(
    'copying connection settings preserves the working directory and board without claiming a confirmed connection',
    (tester) async {
      final robot = BoardDetailsRobot(tester);
      await robot.open();
      await robot.openConnection();
      await robot.expectEffectiveTools();
      await robot.copyConfiguration();
      robot.expectBoundConfigurationCopied();
      robot.expectConnectionNotConfirmed();
    },
  );

  testWidgets(
    'a save failure preserves input without showing a success message',
    (tester) async {
      final robot = BoardDetailsRobot(tester);
      await robot.open(saveFails: true);
      await robot.editGuidance('보존할 설명', '# 아직 저장되지 않음');
      await robot.save();
      robot.expectSaveFailure();
      robot.expectEditorText('보존할 설명', '# 아직 저장되지 않음');
    },
  );

  testWidgets('the web surface does not offer local MCP connection settings', (
    tester,
  ) async {
    final robot = BoardDetailsRobot(tester);
    await robot.open(native: false);
    robot.expectNativeConnectionUnavailable();
  });

  testWidgets('saving on the web shows only the browser storage result', (
    tester,
  ) async {
    final robot = BoardDetailsRobot(tester);
    await robot.open(native: false);
    await robot.save();
    robot.expectBrowserSave();
  });

  testWidgets('a missing CLI executable is shown as not ready to connect', (
    tester,
  ) async {
    final robot = BoardDetailsRobot(tester);
    await robot.open(cliAvailable: false);
    await robot.openConnection();
    robot.expectCliUnavailable();
  });

  testWidgets(
    'unresolved pins suggest checking readiness rather than adding new tools',
    (tester) async {
      final robot = BoardDetailsRobot(tester);
      await robot.open(unresolvedPins: true);
      await robot.openConnection();
      robot.expectUnresolvedPinsNotice();
    },
  );

  testWidgets('a clipboard failure is not reported as a successful copy', (
    tester,
  ) async {
    final robot = BoardDetailsRobot(tester);
    await robot.open(copyFails: true);
    await robot.openConnection();
    await robot.copyConfiguration();
    robot.expectCopyFailure();
  });
}
