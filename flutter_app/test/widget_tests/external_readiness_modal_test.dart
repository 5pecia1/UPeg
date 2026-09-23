/// External readiness behavior in the shared expanded-modal run funnel.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/features/host_attach/attach_client.dart';
import 'package:upeg/src/features/host_attach/host_attach_config.dart';
import 'package:upeg/src/features/host_attach/host_attach_config_provider.dart';
import 'package:upeg/src/features/host_attach/host_attach_dispatch_provider.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/pages/expanded_modal_page.dart';
import 'package:upeg/src/rust/api/readiness.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/capability_provider.dart';
import 'package:upeg/src/state/dispatch_stream_provider.dart';
import 'package:upeg/src/state/last_outcome_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/expanded_modal/primary_button.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';
import 'package:upeg/src/widgets/external_readiness_guidance.dart';

import '../test_helpers/dispatch_stream_fixture.dart';
import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/tool_fixture.dart';

const ExternalReadinessDto _missing = ExternalReadinessDto(
  status: ExternalReadinessStatusDto.missingExecutable,
  platform: 'linux',
  command: 'upeg_setup_probe',
  instructions: 'Install the test probe.',
  installCommands: ['apt install upeg-setup-probe'],
);

const ExternalReadinessDto _ready = ExternalReadinessDto(
  status: ExternalReadinessStatusDto.ready,
  platform: 'linux',
  installCommands: [],
);

const ExternalReadinessDto _unchecked = ExternalReadinessDto(
  status: ExternalReadinessStatusDto.uncheckedCredentialPath,
  platform: 'linux',
  command: 'credential-tool',
  installCommands: [],
);

final ToolDto _externalTool = fixtureToolDto(
  id: 'setup.echo',
  label: 'Setup echo',
  invoker: InvokerDto.external_,
  inputFields: const [
    InputFieldDto(
      key: 'message',
      label: 'Message',
      fieldType: InputFieldType.text(),
      required_: true,
    ),
  ],
);

final class _SeededBoard extends CurrentBoardNotifier {
  @override
  BoardKey? build() => BoardKey.parse('dev');
}

final class _MemoryStore implements HostAttachStore {
  HostAttachConfig config = const HostAttachConfig(
    baseUrl: 'http://host',
    token: 'token',
  );

  @override
  HostAttachConfig read() => config;

  @override
  void write(HostAttachConfig config) => this.config = config;
}

final class _AttachRecorder implements AttachClient {
  _AttachRecorder(
    this.inspections, {
    this.dispatchResult = const AttachDispatchOk(
      CanonicalToolResult(ok: true, outputs: []),
    ),
  });

  final List<AttachReadinessResult> inspections;
  final AttachDispatchResult dispatchResult;
  int inspectionCalls = 0;
  int dispatchCalls = 0;
  ToolArgs? lastArgs;

  @override
  Future<HealthzResult> checkHealth() async =>
      const HealthzOk(name: 'upeg', version: '1');

  @override
  Future<AttachListResult> listTools() async => const AttachListOk([]);

  @override
  Future<AttachReadinessResult> inspectReadiness({
    required ToolId toolId,
    String? boardKey,
  }) async {
    final index = inspectionCalls.clamp(0, inspections.length - 1);
    inspectionCalls += 1;
    return inspections[index];
  }

  @override
  Future<AttachDispatchResult> dispatch({
    required ToolId toolId,
    required ToolArgs args,
    String? boardKey,
  }) async {
    dispatchCalls += 1;
    lastArgs = args;
    return dispatchResult;
  }
}

final class _ExternalModalRobot {
  const _ExternalModalRobot(this.tester);

  final WidgetTester tester;

  Future<void> enterMessage(String value) async {
    await tester.enterText(find.byKey(const Key('field-message')), value);
    await tester.pump();
  }

  Future<void> recheck() async {
    await tester.tap(find.byKey(externalReadinessRecheckKey));
    await tester.pumpAndSettle();
  }

  Future<void> run() async {
    final button = find.byKey(const Key('expanded-modal-run-btn'));
    await tester.tap(button);
    await tester.pumpAndSettle();
  }

  void expectSetupVisible() {
    expect(find.byKey(externalReadinessGuidanceKey), findsOneWidget);
    expect(find.text('Install the test probe.'), findsOneWidget);
  }

  void expectSetupNotVisible() {
    expect(find.byKey(externalReadinessGuidanceKey), findsNothing);
    expect(find.text('Install the test probe.'), findsNothing);
  }

  void expectRunEnabled(bool enabled) {
    final button = tester.widget<ExpandedModalPrimaryButton>(
      find.byKey(const Key('expanded-modal-run-btn')),
    );
    expect(button.enabled, enabled);
  }
}

Future<void> _pump(WidgetTester tester, _AttachRecorder client) async {
  await tester.pumpWidget(
    ProviderScope(
      overrides: [
        ...i18nTestOverrides,
        isWasmRuntimeProvider.overrideWithValue(true),
        hostAttachStoreProvider.overrideWithValue(_MemoryStore()),
        attachClientProvider.overrideWithValue(client),
        currentBoardKeyProvider.overrideWith(_SeededBoard.new),
        lastOutcomeLoadProvider.overrideWithValue((_) => const []),
        lastOutcomePersistProvider.overrideWithValue(
          ({required boardKey, required toolId, required result}) {},
        ),
        toolsLoaderProvider.overrideWithValue(() => [_externalTool]),
        dispatchStreamFnProvider.overrideWithValue(
          stubDispatchStream(
            ({required toolId, required args, required approve}) async =>
                throw StateError('remote External used local dispatch'),
          ),
        ),
      ],
      child: MaterialApp(
        theme: UpegTheme.darkTheme(),
        home: ExpandedModalPage(tool: _externalTool),
      ),
    ),
  );
  await tester.pumpAndSettle();
}

void main() {
  testWidgets(
    'should block missing setup then preserve entered args and dispatch once after recheck',
    (tester) async {
      final client = _AttachRecorder([
        const AttachReadinessOk(_missing),
        const AttachReadinessOk(_ready),
      ]);
      await _pump(tester, client);
      final robot = _ExternalModalRobot(tester);

      await robot.enterMessage('kept across recheck');
      robot.expectSetupVisible();
      robot.expectRunEnabled(false);
      expect(client.dispatchCalls, 0);

      await robot.recheck();
      robot.expectSetupNotVisible();
      robot.expectRunEnabled(true);
      await robot.run();

      expect(client.dispatchCalls, 1);
      expect(client.lastArgs?.toJsonObject(), {
        'message': 'kept across recheck',
      });
    },
  );

  testWidgets('should allow unchecked credential PATH readiness to run', (
    tester,
  ) async {
    final client = _AttachRecorder([const AttachReadinessOk(_unchecked)]);
    await _pump(tester, client);
    final robot = _ExternalModalRobot(tester);

    await robot.enterMessage('runtime check');
    robot.expectRunEnabled(true);
    await robot.run();

    expect(client.dispatchCalls, 1);
  });

  testWidgets('should allow imported External with no requirements to run', (
    tester,
  ) async {
    final client = _AttachRecorder([const AttachReadinessNotApplicable()]);
    await _pump(tester, client);
    final robot = _ExternalModalRobot(tester);

    await robot.enterMessage('mcp proxy');
    robot.expectRunEnabled(true);
    await robot.run();

    expect(client.inspectionCalls, 1);
    expect(client.dispatchCalls, 1);
  });

  testWidgets(
    'should invalidate cached ready when dispatch reports missing readiness',
    (tester) async {
      final client = _AttachRecorder(
        [const AttachReadinessOk(_ready), const AttachReadinessOk(_missing)],
        dispatchResult: const AttachDispatchToolError(
          CanonicalToolError(
            code: 'tool_error',
            message: 'command disappeared',
            details:
                '{"readiness":{"status":"missing_executable","command":"upeg_setup_probe"}}',
          ),
        ),
      );
      await _pump(tester, client);
      final robot = _ExternalModalRobot(tester);

      await robot.enterMessage('do not retry automatically');
      robot.expectRunEnabled(true);
      await robot.run();
      robot.expectSetupVisible();

      expect(client.dispatchCalls, 1);
      expect(client.inspectionCalls, 2);
    },
  );
}
