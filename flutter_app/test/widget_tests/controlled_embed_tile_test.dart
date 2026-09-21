import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/widgets/controlled_embed/runner.dart';
import '../test_helpers/controlled_embed_tile_harness.dart';

void main() {
  useStubbedControlledEmbedSeams();

  testWidgets(
    'before_running_only_a_hint_is_shown_and_no_browser_session_is_created',
    (tester) async {
      final robot = ControlledEmbedTileRobot(tester);
      await robot.pump();
      robot.expectRunHint();
      expect(controlledEmbedFixture.calls, isEmpty);
      expect(controlledEmbedFixture.factory.sessions, isEmpty);
    },
  );

  testWidgets('the_run_result_is_shown_with_its_declared_label_and_value', (
    tester,
  ) async {
    controlledEmbedFixture.executor =
        ({required toolId, required args, boardKey}) async =>
            canonicalSuccess({'intro': 'short article preview'});
    final robot = ControlledEmbedTileRobot(tester);
    await robot.pump();
    await robot.tapRun();
    expect(controlledEmbedFixture.calls.single.args.isEmpty, isTrue);
    robot.expectOutputRow(label: 'Intro', value: 'short article preview');
  });

  testWidgets('a_dispatcher_error_is_shown_in_the_result_area', (tester) async {
    controlledEmbedFixture.executor =
        ({required toolId, required args, boardKey}) async =>
            canonicalError('selector miss: #intro');
    final robot = ControlledEmbedTileRobot(tester);
    await robot.pump();
    await robot.tapRun();
    robot.expectErrorContaining('selector miss');
  });

  testWidgets('an_opaque_javascript_error_is_shown_with_its_text_preserved', (
    tester,
  ) async {
    controlledEmbedFixture.executor =
        ({required toolId, required args, boardKey}) async =>
            canonicalError(kOpaqueObservedError);
    final robot = ControlledEmbedTileRobot(tester);
    await robot.pump();
    await robot.tapRun();
    robot.expectErrorContaining(kOpaqueObservedError);
  });

  testWidgets('a_runner_exception_is_shown_and_the_run_button_is_re_enabled', (
    tester,
  ) async {
    controlledEmbedFixture.executor =
        ({required toolId, required args, boardKey}) async =>
            throw StateError('executor exploded');
    final robot = ControlledEmbedTileRobot(tester);
    await robot.pump();
    await robot.tapRun();
    robot.expectErrorContaining('executor exploded');
    robot.expectRunEnabled(true);
  });

  testWidgets(
    'a_normalized_error_is_shown_without_platform_internal_phrasing',
    (tester) async {
      controlledEmbedFixture.executor =
          ({required toolId, required args, boardKey}) async =>
              canonicalError(kControlledEmbedNullJavaScriptResultMessage);
      final robot = ControlledEmbedTileRobot(tester);
      await robot.pump();
      await robot.tapRun();
      robot.expectErrorContaining(kControlledEmbedNullJavaScriptResultMessage);
      robot.expectNoTextContaining('Invalid argument(s)');
      robot.expectNoTextContaining('platform-channels');
    },
  );

  testWidgets(
    'a_successful_rerun_replaces_the_previous_error_with_the_result',
    (tester) async {
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
    },
  );

  testWidgets('a_browser_connection_error_is_visible_in_the_tile_error_area', (
    tester,
  ) async {
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
