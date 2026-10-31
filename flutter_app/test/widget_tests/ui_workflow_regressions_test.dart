import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/board_tabs.dart';
import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';

import '../shared/controlled_clipboard_writer.dart';
import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/pegboard_selection_overrides.dart';
import '../test_helpers/ui_workflow_robot.dart';

Widget _copyHarness(ControlledClipboardWriter writer) => ProviderScope(
  overrides: [...i18nTestOverrides],
  child: MaterialApp(
    home: Scaffold(
      body: CopyToClipboardButton(textToCopy: 'result', writer: writer),
    ),
  ),
);

void main() {
  testWidgets('should keep search and settings reachable at 390 pixels', (
    tester,
  ) async {
    var searches = 0;
    var settings = 0;
    final robot = UiWorkflowRobot(tester);
    final container = ProviderContainer(
      overrides: [
        ...pegboardSelectionOverrides(),
        ...i18nTestOverrides,
        boardsLoaderProvider.overrideWith(
          (_) =>
              () => const [
                BoardDto(key: 'dev', title: 'Development tools and converters'),
                BoardDto(key: 'media', title: 'Media'),
              ],
        ),
      ],
    );
    addTearDown(container.dispose);
    container
        .read(currentBoardKeyProvider.notifier)
        .select(BoardKey.parse('dev'));
    await robot.pumpScreen(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          theme: UpegTheme.darkTheme(),
          home: Scaffold(
            body: SizedBox(
              width: 390,
              child: BoardTabs(
                onOpenPalette: () => searches++,
                onOpenSettings: () => settings++,
              ),
            ),
          ),
        ),
      ),
    );

    robot.expectPrimaryActionsReachable(390);
    await robot.openSearch();
    await robot.openSettingsFromMore();

    expect(searches, 1);
    expect(settings, 1);
  });

  testWidgets(
    'should announce and activate a single board tab through semantics',
    (tester) async {
      final robot = UiWorkflowRobot(tester);
      final semantics = tester.ensureSemantics();

      final container = ProviderContainer(
        overrides: [
          ...pegboardSelectionOverrides(),
          ...i18nTestOverrides,
          boardsLoaderProvider.overrideWith(
            (_) =>
                () => const [
                  BoardDto(key: 'dev', title: 'Dev'),
                  BoardDto(key: 'media', title: 'Media'),
                ],
          ),
        ],
      );
      addTearDown(container.dispose);
      container
          .read(currentBoardKeyProvider.notifier)
          .select(BoardKey.parse('dev'));
      await robot.pumpScreen(
        UncontrolledProviderScope(
          container: container,
          child: MaterialApp(
            home: Scaffold(
              body: BoardTabs(onOpenPalette: () {}, onOpenSettings: () {}),
            ),
          ),
        ),
      );

      robot.expectSelectedBoard('dev', 'media');

      await robot.tapBoardThroughSemantics('media');

      expect(container.read(currentBoardKeyProvider), BoardKey.parse('media'));
      robot.expectSelectedBoard('media', 'dev', selectedLabel: 'Media');
      semantics.dispose();
    },
  );

  testWidgets('should report copying only after clipboard write completes', (
    tester,
  ) async {
    final writer = ControlledClipboardWriter();
    final robot = UiWorkflowRobot(tester);
    await robot.pumpScreen(_copyHarness(writer));

    await robot.tapCopy();

    robot.expectCopyDisabled();
    robot.expectCopiedNotVisible();
    writer.succeed();
    await tester.pump();
    robot.expectCopiedVisible();
    robot.expectCopyFailureNotVisible();
    await tester.pump(const Duration(seconds: 3));
    robot.expectCopiedNotVisible();
  });

  testWidgets('should show failure when clipboard write fails', (tester) async {
    final writer = ControlledClipboardWriter();
    final robot = UiWorkflowRobot(tester);
    await robot.pumpScreen(_copyHarness(writer));

    await robot.tapCopy();
    writer.fail();
    await tester.pump();

    robot.expectCopyFailureVisible();
    robot.expectCopiedNotVisible();
    await tester.pump(const Duration(seconds: 3));
    robot.expectCopyFailureNotVisible();
  });
}
