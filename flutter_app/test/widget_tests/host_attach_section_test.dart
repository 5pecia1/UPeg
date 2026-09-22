/// Settings → Host attach section tests (Task B3).
///
/// Drives the pairing fields + the check-connection action through the
/// `hostAttachConfigProvider` / `attachClientProvider` seams — no browser
/// storage, no network.
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/features/host_attach/attach_client.dart';
import 'package:upeg/src/features/host_attach/host_attach_config.dart';
import 'package:upeg/src/features/host_attach/host_attach_config_provider.dart';
import 'package:upeg/src/features/host_attach/host_attach_dispatch_provider.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/readiness.dart';
import 'package:upeg/src/rust/api/tweaks.dart';
import 'package:upeg/src/state/tweaks_provider.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';
import 'package:upeg/src/widgets/expanded_modal/external_readiness_panel.dart';
import 'package:upeg/src/widgets/external_readiness_guidance.dart';
import 'package:upeg/src/widgets/host_attach_section.dart';

import '../test_helpers/i18n_test_catalog.dart';

class _FakeStore implements HostAttachStore {
  _FakeStore(this.config);
  HostAttachConfig config;
  int writes = 0;
  @override
  HostAttachConfig read() => config;
  @override
  void write(HostAttachConfig next) {
    config = next;
    writes += 1;
  }
}

class _FakeAttachClient implements AttachClient {
  _FakeAttachClient(
    this.healthzResult, {
    this.listResult = const AttachListOk([]),
    this.readinessResult = const AttachReadinessUnreachable(),
  });
  final HealthzResult healthzResult;
  final AttachListResult listResult;
  AttachReadinessResult readinessResult;
  int readinessCalls = 0;
  int dispatchCalls = 0;
  int listCalls = 0;

  @override
  Future<HealthzResult> checkHealth() async => healthzResult;
  @override
  Future<AttachListResult> listTools() async {
    listCalls += 1;
    return listResult;
  }

  @override
  Future<AttachReadinessResult> inspectReadiness({
    required ToolId toolId,
    String? boardKey,
  }) async {
    readinessCalls += 1;
    return readinessResult;
  }

  @override
  Future<AttachDispatchResult> dispatch({
    required ToolId toolId,
    required ToolArgs args,
    String? boardKey,
  }) async {
    dispatchCalls += 1;
    return const AttachDispatchUnreachable();
  }
}

final class _DeferredHealthClient extends _FakeAttachClient {
  _DeferredHealthClient()
    : super(const HealthzOk(name: 'old-host', version: '1'));

  final Completer<HealthzResult> health = Completer<HealthzResult>();

  @override
  Future<HealthzResult> checkHealth() => health.future;
}

final class _HostAttachRobot {
  const _HostAttachRobot(this.tester);

  final WidgetTester tester;

  Future<void> checkConnection() async {
    await tester.tap(find.byKey(kHostAttachCheckButtonKey));
    await tester.pumpAndSettle();
    await tester.pump(const Duration(milliseconds: 1));
    await tester.pumpAndSettle();
  }

  Future<void> changeHost(String value) async {
    await tester.enterText(find.byKey(kHostAttachBaseUrlFieldKey), value);
    await tester.pumpAndSettle();
  }

  Future<void> recheckReadiness() async {
    await tester.tap(find.byKey(externalReadinessRecheckKey));
    await tester.pumpAndSettle();
  }

  void expectVisible(String text) => expect(find.text(text), findsOneWidget);

  void expectNotVisible(String text) => expect(find.text(text), findsNothing);
}

Future<void> _pump(
  WidgetTester tester, {
  required HostAttachStore store,
  required AttachClient client,
}) async {
  await tester.pumpWidget(
    ProviderScope(
      overrides: [
        ...i18nTestOverrides,
        hostAttachStoreProvider.overrideWithValue(store),
        attachClientProvider.overrideWithValue(client),
      ],
      child: const MaterialApp(home: Scaffold(body: HostAttachSection())),
    ),
  );
  await tester.pumpAndSettle();
}

void main() {
  group('HostAttachSection', () {
    testWidgets('the_healthz_check_connection_action', (tester) async {
      await _pump(
        tester,
        store: _FakeStore(
          const HostAttachConfig(baseUrl: 'http://127.0.0.1:7173', token: 't'),
        ),
        client: _FakeAttachClient(
          const HealthzOk(name: 'upeg', version: '0.1'),
        ),
      );

      await tester.tap(find.byKey(kHostAttachCheckButtonKey));
      await tester.pumpAndSettle();
      expect(find.text(i18nEn(kHostAttachConnectedKey)), findsOneWidget);
    });

    testWidgets('an_unreachable_host_shows_the_failure_notice', (tester) async {
      final store = _FakeStore(HostAttachConfig.empty);
      await _pump(
        tester,
        store: store,
        client: _FakeAttachClient(const HealthzUnreachable()),
      );

      await tester.tap(find.byKey(kHostAttachCheckButtonKey));
      await tester.pumpAndSettle();
      expect(find.text(i18nEn(kHostAttachUnreachableKey)), findsOneWidget);
      expect(store.config.baseUrl, isEmpty);
    });

    testWidgets('editing_the_host_field_persists_to_settings', (tester) async {
      final store = _FakeStore(HostAttachConfig.empty);
      await _pump(
        tester,
        store: store,
        client: _FakeAttachClient(const HealthzUnreachable()),
      );

      await tester.enterText(
        find.byKey(kHostAttachTokenFieldKey),
        'pasted-token',
      );
      await tester.pumpAndSettle();
      expect(store.config.token, 'pasted-token');
      expect(store.writes, greaterThan(0));
    });

    testWidgets(
      'switching_the_locale_to_korean_renders_the_host_attach_section_in_korean',
      (tester) async {
        TweaksDto tweaks(String locale) => TweaksDto(
          theme: 'Dark',
          accent: 'Green',
          showHoles: true,
          locale: locale,
          localHttpHost: false,
        );
        final container = ProviderContainer(
          overrides: [
            ...i18nTestOverrides,
            hostAttachStoreProvider.overrideWithValue(
              _FakeStore(HostAttachConfig.empty),
            ),
            attachClientProvider.overrideWithValue(
              _FakeAttachClient(const HealthzUnreachable()),
            ),
            tweaksLoaderProvider.overrideWith(
              (ref) =>
                  () => tweaks('En'),
            ),
            tweaksSaverProvider.overrideWith((ref) => (TweaksDto _) {}),
          ],
        );
        addTearDown(container.dispose);
        await container.read(tweaksProvider.future);

        await tester.pumpWidget(
          UncontrolledProviderScope(
            container: container,
            child: const MaterialApp(home: Scaffold(body: HostAttachSection())),
          ),
        );
        await tester.pumpAndSettle();

        expect(find.text(i18nEn(kHostAttachBaseUrlLabelKey)), findsOneWidget);
        expect(find.text(i18nEn(kHostAttachCheckLabelKey)), findsOneWidget);
        expect(find.text(i18nKo(kHostAttachBaseUrlLabelKey)), findsNothing);

        // Live flip to Korean — labels re-render in place (K05).
        await container.read(tweaksProvider.notifier).save(tweaks('Ko'));
        await tester.pumpAndSettle();

        expect(find.text(i18nKo(kHostAttachBaseUrlLabelKey)), findsOneWidget);
        expect(find.text(i18nKo(kHostAttachTokenLabelKey)), findsOneWidget);
        expect(find.text(i18nKo(kHostAttachCheckLabelKey)), findsOneWidget);
        expect(find.text(i18nEn(kHostAttachBaseUrlLabelKey)), findsNothing);
      },
    );

    testWidgets(
      'should show only local External host tools with platform and cache checks across rebuilds',
      (tester) async {
        final client = _FakeAttachClient(
          const HealthzOk(name: 'upeg', version: '1'),
          listResult: const AttachListOk([
            AttachToolSummary(
              id: 'setup.echo',
              label: 'External Tool',
              invoker: AttachToolInvoker.external,
              source: 'local',
            ),
            AttachToolSummary(
              id: 'builtin.uuid',
              label: 'Function Tool',
              invoker: AttachToolInvoker.other,
              source: 'local',
            ),
            AttachToolSummary(
              id: 'github.issue',
              label: 'Imported MCP Tool',
              invoker: AttachToolInvoker.external,
              source: 'mcp-import:github',
            ),
          ]),
          readinessResult: const AttachReadinessOk(
            ExternalReadinessDto(
              status: ExternalReadinessStatusDto.ready,
              platform: 'linux',
              installCommands: [],
            ),
          ),
        );
        await _pump(
          tester,
          store: _FakeStore(
            const HostAttachConfig(baseUrl: 'http://host', token: 'token'),
          ),
          client: client,
        );
        final robot = _HostAttachRobot(tester);

        await robot.checkConnection();
        robot.expectVisible('External Tool · setup.echo · linux');
        robot.expectVisible('ready on linux');
        robot.expectNotVisible('Function Tool');
        robot.expectNotVisible('Imported MCP Tool');
        expect(client.readinessCalls, 1);

        await tester.pump();
        expect(client.readinessCalls, 1);
        expect(client.dispatchCalls, 0);

        await robot.recheckReadiness();
        expect(client.readinessCalls, 2);
        expect(client.dispatchCalls, 0);
      },
    );

    testWidgets(
      'should clear stale catalog then fetch once after changed host is checked',
      (tester) async {
        final client = _FakeAttachClient(
          const HealthzOk(name: 'upeg', version: '1'),
          listResult: const AttachListOk([
            AttachToolSummary(
              id: 'setup.echo',
              label: 'External Tool',
              invoker: AttachToolInvoker.external,
              source: 'local',
            ),
          ]),
          readinessResult: const AttachReadinessOk(
            ExternalReadinessDto(
              status: ExternalReadinessStatusDto.ready,
              platform: 'macos',
              installCommands: [],
            ),
          ),
        );
        await _pump(
          tester,
          store: _FakeStore(
            const HostAttachConfig(baseUrl: 'http://old', token: 'token'),
          ),
          client: client,
        );
        final robot = _HostAttachRobot(tester);
        await robot.checkConnection();
        expect(client.readinessCalls, 1);

        await robot.changeHost('http://new');
        robot.expectNotVisible('External Tool · setup.echo · macos');
        robot.expectNotVisible(i18nEn(kHostAttachConnectedKey));
        expect(client.readinessCalls, 1);

        await robot.checkConnection();
        expect(client.readinessCalls, 2);
        await tester.pump();
        expect(client.readinessCalls, 2);
      },
    );

    testWidgets('should ignore an old host check after config changes', (
      tester,
    ) async {
      final client = _DeferredHealthClient();
      await _pump(
        tester,
        store: _FakeStore(
          const HostAttachConfig(baseUrl: 'http://old', token: 'token'),
        ),
        client: client,
      );
      final robot = _HostAttachRobot(tester);

      await tester.tap(find.byKey(kHostAttachCheckButtonKey));
      await tester.pump();
      await robot.changeHost('http://new');
      client.health.complete(const HealthzOk(name: 'old-host', version: '1'));
      await tester.pumpAndSettle();

      robot.expectNotVisible(i18nEn(kHostAttachConnectedKey));
      expect(client.listCalls, 0);
    });

    testWidgets('should render guidance selected for the host platform', (
      tester,
    ) async {
      final client = _FakeAttachClient(
        const HealthzOk(name: 'upeg', version: '1'),
        listResult: const AttachListOk([
          AttachToolSummary(
            id: 'setup.echo',
            label: 'External Tool',
            invoker: AttachToolInvoker.external,
            source: 'local',
          ),
        ]),
        readinessResult: const AttachReadinessOk(
          ExternalReadinessDto(
            status: ExternalReadinessStatusDto.missingExecutable,
            platform: 'windows',
            instructions: 'Install the Windows probe package.',
            installCommands: ['winget install UPeg.Probe'],
          ),
        ),
      );
      await _pump(
        tester,
        store: _FakeStore(
          const HostAttachConfig(baseUrl: 'http://host', token: 'token'),
        ),
        client: client,
      );
      final robot = _HostAttachRobot(tester);

      await robot.checkConnection();
      robot.expectVisible('External Tool · setup.echo · windows');
      robot.expectVisible('Install the Windows probe package.');
      robot.expectVisible('winget install UPeg.Probe');
      robot.expectNotVisible('apt install upeg-probe');
    });

    testWidgets('should distinguish unauthorized inventory from connected', (
      tester,
    ) async {
      await _pump(
        tester,
        store: _FakeStore(
          const HostAttachConfig(baseUrl: 'http://host', token: 'bad'),
        ),
        client: _FakeAttachClient(
          const HealthzOk(name: 'upeg', version: '1'),
          listResult: const AttachListUnauthorized(),
        ),
      );
      final robot = _HostAttachRobot(tester);

      await robot.checkConnection();
      robot.expectVisible(i18nEn(kHostAttachUnauthorizedKey));
      robot.expectNotVisible(i18nEn(kHostAttachConnectedKey));
    });

    testWidgets('should show malformed readiness with a nonexecuting retry', (
      tester,
    ) async {
      final client = _FakeAttachClient(
        const HealthzOk(name: 'upeg', version: '1'),
        listResult: const AttachListOk([
          AttachToolSummary(
            id: 'setup.echo',
            label: 'External Tool',
            invoker: AttachToolInvoker.external,
            source: 'local',
          ),
        ]),
        readinessResult: const AttachReadinessMalformed(),
      );
      await _pump(
        tester,
        store: _FakeStore(
          const HostAttachConfig(baseUrl: 'http://host', token: 'token'),
        ),
        client: client,
      );
      final robot = _HostAttachRobot(tester);

      await robot.checkConnection();
      robot.expectVisible(i18nEn('readiness.host_malformed'));
      await tester.tap(find.byKey(externalReadinessRetryKey));
      await tester.pumpAndSettle();
      expect(client.readinessCalls, 2);
      expect(client.dispatchCalls, 0);
    });
  });
}
