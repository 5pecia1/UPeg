import 'dart:async';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/features/controlled_embed/session_service.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/widgets/controlled_embed/settings.dart';
import '../test_helpers/controlled_embed_tile_harness.dart';

void main() {
  useStubbedControlledEmbedSeams();

  testWidgets(
    'after_an_output_conversion_failure_reopening_debug_or_clearing_the_log_never_shows_the_raw_success_output',
    (tester) async {
      final canonicalErrorResult = canonicalError('Expected a numeric output.');
      controlledEmbedFixture.normalizeResult = (_, _, _) =>
          canonicalErrorResult;
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
    },
  );

  testWidgets('the_modal_shows_the_app_session_browser_and_run_console', (
    tester,
  ) async {
    final robot = ControlledEmbedDebugRobot(tester);
    await robot.pump();
    robot.expectVisible(true);
    robot.expectPanesVisible();
    robot.expectRunEnabled(true);
    expect(controlledEmbedFixture.calls, isEmpty);
  });

  testWidgets(
    'an_input_snapshot_is_shown_before_running_and_number_types_are_passed_through',
    (tester) async {
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
    },
  );

  testWidgets('the_run_button_stays_disabled_until_the_page_is_ready', (
    tester,
  ) async {
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

  testWidgets(
    'rerunning_keeps_the_modal_and_page_while_updating_the_shared_run_events',
    (tester) async {
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
    },
  );

  testWidgets(
    'clearing_the_console_keeps_the_result_and_page_and_only_clears_the_run_history',
    (tester) async {
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
    },
  );

  testWidgets(
    'closing_the_modal_does_not_end_the_session_and_only_releases_the_debug_reservation',
    (tester) async {
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
    },
  );

  testWidgets('a_run_failure_is_shown_in_the_console_and_can_be_rerun', (
    tester,
  ) async {
    controlledEmbedFixture.executor =
        ({required toolId, required args, boardKey}) async =>
            canonicalError('selector miss: #result');
    final robot = ControlledEmbedDebugRobot(tester);
    await robot.pump();
    await robot.tapRun();
    robot.expectConsoleContaining('selector miss: #result');
    robot.expectRunEnabled(true);
  });

  testWidgets(
    'explicit_browser_settings_apply_to_the_shared_session_without_a_separate_page',
    (tester) async {
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
    },
  );
}
