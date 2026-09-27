/// Compact pin readiness badge and shared setup-details behavior.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/readiness.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/capability_provider.dart';
import 'package:upeg/src/state/external_readiness_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/board_canvas.dart';
import 'package:upeg/src/widgets/expanded_modal/external_readiness_panel.dart';
import 'package:upeg/src/widgets/external_readiness_guidance.dart';

import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/tool_fixture.dart';

final ToolDto _tool = fixtureToolDto(
  id: 'setup.echo',
  label: 'Setup echo',
  invoker: InvokerDto.external_,
);

const LayoutSnapshotDto _snapshot = LayoutSnapshotDto(
  boardKey: 'dev',
  boardCols: 4,
  placements: [
    PlacementDto(
      toolId: 'setup.echo',
      pinId: 'setup.echo',
      x: 0,
      y: 0,
      w: 1,
      h: 1,
    ),
  ],
);

final class _PinReadinessRobot {
  const _PinReadinessRobot(this.tester);

  final WidgetTester tester;

  void expectCompactBadgeWithoutCommand() {
    expect(find.byKey(externalReadinessBadgeKey), findsOneWidget);
    expect(find.text('upeg_setup_probe --verbose'), findsNothing);
  }

  Future<void> openDetails() async {
    await tester.tap(find.byKey(externalReadinessBadgeKey));
    await tester.pumpAndSettle();
  }

  void expectDetailsWithCommand() {
    expect(find.byKey(externalReadinessDialogKey), findsOneWidget);
    expect(find.text('upeg_setup_probe --verbose'), findsOneWidget);
    expect(find.text('Install the probe for this platform.'), findsOneWidget);
  }

  Future<void> recheck() async {
    await tester.tap(find.byKey(externalReadinessRecheckKey));
    await tester.pumpAndSettle();
  }
}

void main() {
  testWidgets(
    'should keep missing setup compact and open cached nonexecuting details',
    (tester) async {
      var inspections = 0;
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            ...i18nTestOverrides,
            isWasmRuntimeProvider.overrideWithValue(false),
            toolsLoaderProvider.overrideWithValue(() => [_tool]),
            localReadinessInspectorProvider.overrideWithValue(({
              required toolId,
              boardKey,
            }) async {
              inspections += 1;
              return const ExternalReadinessDto(
                status: ExternalReadinessStatusDto.missingExecutable,
                platform: 'linux',
                command: 'upeg_setup_probe --verbose',
                instructions: 'Install the probe for this platform.',
                installCommands: ['apt install upeg-setup-probe'],
              );
            }),
          ],
          child: MaterialApp(
            theme: UpegTheme.darkTheme(),
            home: Scaffold(
              body: debugBoardCanvasGrid(
                snapshot: _snapshot,
                boardKey: BoardKey.parse('dev'),
                onPinTap: (_) {},
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      final robot = _PinReadinessRobot(tester);

      robot.expectCompactBadgeWithoutCommand();
      expect(inspections, 1);
      await tester.pump();
      expect(inspections, 1);

      await robot.openDetails();
      robot.expectDetailsWithCommand();
      expect(inspections, 1);

      await robot.recheck();
      expect(inspections, 2);
    },
  );
}
