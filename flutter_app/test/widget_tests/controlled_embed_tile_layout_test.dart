import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/theme/upeg_theme.dart' show UpegSizing;
import '../test_helpers/controlled_embed_tile_harness.dart';

void main() {
  useStubbedControlledEmbedSeams();

  testWidgets('a_long_output_shows_as_selectable_text_without_truncation', (
    tester,
  ) async {
    controlledEmbedFixture.executor =
        ({required toolId, required args, boardKey}) async =>
            canonicalSuccess({'intro': kLongControlledEmbedOutput});
    final robot = ControlledEmbedTileRobot(tester);
    await robot.pump();
    await robot.tapRun();
    robot.expectSelectableOutput('intro', kLongControlledEmbedOutput);
  });

  testWidgets('the_output_copy_button_passes_the_full_value_to_the_clipboard', (
    tester,
  ) async {
    const fullValue = 'hello world copy test';
    final writer = RecordingClipboardWriterForTile();
    controlledEmbedFixture.executor =
        ({required toolId, required args, boardKey}) async =>
            canonicalSuccess({'intro': fullValue});
    final robot = ControlledEmbedTileRobot(tester);
    await robot.pump(clipboard: writer);
    await robot.tapRun();
    await robot.copyOutput('intro');
    expect(writer.writes, [fullValue]);
  });

  testWidgets('an_error_also_shows_as_selectable_text_without_truncation', (
    tester,
  ) async {
    controlledEmbedFixture.executor =
        ({required toolId, required args, boardKey}) async =>
            canonicalError(kSelectorMissControlledEmbedError);
    final robot = ControlledEmbedTileRobot(tester);
    await robot.pump();
    await robot.tapRun();
    robot.expectSelectableError(kSelectorMissControlledEmbedError);
  });

  testWidgets(
    'a_narrow_tile_still_lays_out_the_run_button_without_creating_a_browser',
    (tester) async {
      final robot = ControlledEmbedTileRobot(tester);
      await robot.pump(width: UpegSizing.pinCellWidth, height: 240);
      robot.expectNoFrameworkError();
      robot.expectRunEnabled(true);
      await robot.pump(width: 80, height: 240);
      robot.expectNoFrameworkError();
      robot.expectDebugEnabled(true);
      expect(controlledEmbedFixture.factory.sessions, isEmpty);
    },
  );
}
