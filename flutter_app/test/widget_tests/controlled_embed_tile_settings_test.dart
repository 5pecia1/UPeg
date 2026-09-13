import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/embed.dart';
import 'package:upeg/src/widgets/controlled_embed/settings.dart';
import '../test_helpers/controlled_embed_tile_harness.dart';

void main() {
  useStubbedControlledEmbedSeams();

  testWidgets('설정이 있어도 타일 표시만으로 브라우저를 생성하지 않는다', (tester) async {
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

  testWidgets('디버그는 매니페스트의 사용자 에이전트와 뷰포트로 앱 세션을 요청한다', (tester) async {
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
  });

  testWidgets('설정 생략은 강제 모바일 설정 없이 브라우저 기본값을 사용한다', (tester) async {
    final robot = ControlledEmbedTileRobot(tester);
    await robot.pump();
    await robot.tapDebug();
    final browser = controlledEmbedFixture.factory.sessions.single;
    expect(browser.settings?.hasUserAgentOverride ?? false, isFalse);
    expect(browser.settings?.userAgent, isNull);
    expect(browser.viewportSize, ViewPortPresetSizes.desktop);
  });
}
