/// Settings → Host attach section tests (Task B3).
///
/// Drives the pairing fields + the check-connection action through the
/// `hostAttachConfigProvider` / `attachClientProvider` seams — no browser
/// storage, no network.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/features/host_attach/attach_client.dart';
import 'package:upeg/src/features/host_attach/host_attach_config.dart';
import 'package:upeg/src/features/host_attach/host_attach_config_provider.dart';
import 'package:upeg/src/features/host_attach/host_attach_dispatch_provider.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/tweaks.dart';
import 'package:upeg/src/state/tweaks_provider.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';
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
  _FakeAttachClient(this.healthzResult);
  final HealthzResult healthzResult;

  @override
  Future<HealthzResult> checkHealth() async => healthzResult;
  @override
  Future<AttachListResult> listTools() async => const AttachListOk([]);
  @override
  Future<AttachDispatchResult> dispatch({
    required ToolId toolId,
    required ToolArgs args,
    String? boardKey,
  }) async => const AttachDispatchUnreachable();
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
  });
}
