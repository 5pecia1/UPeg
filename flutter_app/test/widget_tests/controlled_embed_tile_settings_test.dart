import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/embed.dart';
import 'package:upeg/src/widgets/controlled_embed/settings.dart';
import '../test_helpers/controlled_embed_tile_harness.dart';

void main() {
  useStubbedControlledEmbedSeams();

  testWidgets('showing_the_tile_alone_creates_no_browser_even_with_settings', (
    tester,
  ) async {
    final robot = ControlledEmbedTileRobot(tester);
    await robot.pump(
      settings: const ControlledEmbedSettingsDto(
        userAgent: ControlledEmbedUserAgentDto.custom(value: 'test-agent'),
        viewport: ControlledEmbedViewportDto.preset(
          preset: ControlledEmbedViewportPresetDto.tablet,
        ),
      ),
    );
    expect(controlledEmbedFixture.factory.sessions, isEmpty);
    robot.expectRunEnabled(true);
  });

  testWidgets(
    'debug_requests_an_app_session_with_the_manifest_user_agent_and_viewport',
    (tester) async {
      const agent = 'test-agent';
      final robot = ControlledEmbedTileRobot(tester);
      await robot.pump(
        settings: const ControlledEmbedSettingsDto(
          userAgent: ControlledEmbedUserAgentDto.custom(value: agent),
          viewport: ControlledEmbedViewportDto.preset(
            preset: ControlledEmbedViewportPresetDto.tablet,
          ),
        ),
      );
      await robot.tapDebug();
      final browser = controlledEmbedFixture.factory.sessions.single;
      expect(browser.settings?.userAgent, agent);
      expect(browser.settings?.viewportSize, ViewPortPresetSizes.tablet);
      ControlledEmbedDebugRobot(tester).expectPanesVisible();
    },
  );

  testWidgets(
    'omitted_settings_use_browser_defaults_without_forced_mobile_settings',
    (tester) async {
      final robot = ControlledEmbedTileRobot(tester);
      await robot.pump();
      await robot.tapDebug();
      final browser = controlledEmbedFixture.factory.sessions.single;
      expect(browser.settings?.hasUserAgentOverride ?? false, isFalse);
      expect(browser.settings?.userAgent, isNull);
      expect(browser.viewportSize, ViewPortPresetSizes.desktop);
    },
  );
}
