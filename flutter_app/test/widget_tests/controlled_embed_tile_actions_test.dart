import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import '../test_helpers/controlled_embed_tile_harness.dart';

void main() {
  useStubbedControlledEmbedSeams();

  testWidgets(
    'an_environment_without_browser_support_blocks_running_and_guides_toward_opening_externally',
    (tester) async {
      controlledEmbedFixture.nativeSupported = false;
      final robot = ControlledEmbedTileRobot(tester);
      await robot.pump();
      robot.expectRunEnabled(false);
      robot.expectDebugEnabled(false);
      robot.expectUnsupported(true);
      expect(controlledEmbedFixture.factory.sessions, isEmpty);
    },
  );

  testWidgets('run_and_debug_are_available_in_a_supported_environment', (
    tester,
  ) async {
    final robot = ControlledEmbedTileRobot(tester);
    await robot.pump();
    robot.expectRunEnabled(true);
    robot.expectDebugEnabled(true);
    robot.expectUnsupported(false);
  });

  testWidgets(
    'opening_debug_shows_the_app_session_without_running_the_tool_automatically',
    (tester) async {
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
    },
  );

  testWidgets(
    'opening_debug_after_a_normal_run_keeps_the_browser_and_page_state',
    (tester) async {
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
    },
  );

  testWidgets(
    'form_numbers_and_strings_reach_the_dispatcher_with_their_types_and_board_context_preserved',
    (tester) async {
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
    },
  );
}
