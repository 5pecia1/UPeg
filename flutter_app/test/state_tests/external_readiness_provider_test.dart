import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/features/host_attach/attach_client.dart';
import 'package:upeg/src/features/host_attach/host_attach_config.dart';
import 'package:upeg/src/features/host_attach/host_attach_config_provider.dart';
import 'package:upeg/src/features/host_attach/host_attach_dispatch_provider.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/readiness.dart';
import 'package:upeg/src/state/capability_provider.dart';
import 'package:upeg/src/state/external_readiness_provider.dart';

import '../shared/fake_readiness.dart';

ExternalReadinessDto readiness(ExternalReadinessStatusDto status) =>
    ExternalReadinessDto(
      status: status,
      platform: 'linux',
      installCommands: const <String>[],
    );

Future<ExternalReadinessInspection> resolve(
  ProviderContainer container,
  ExternalReadinessTarget target,
) async {
  final provider = externalReadinessProvider(target);
  container.listen(provider, (_, _) {});
  for (var attempt = 0; attempt < 10; attempt++) {
    await Future<void>.delayed(Duration.zero);
    final value = container.read(provider);
    if (value.hasValue) return value.requireValue;
    if (value.hasError) throw value.error!;
  }
  throw StateError('readiness provider did not settle');
}

void main() {
  final external = externalReadinessTarget(ToolId.parse('shell.rg'));

  ProviderContainer container({
    required FakeReadinessAttachClient client,
    required LocalReadinessInspector local,
    HostAttachConfig config = HostAttachConfig.empty,
    bool wasm = false,
  }) {
    final store = _MemoryStore(config);
    final result = ProviderContainer(
      overrides: [
        isWasmRuntimeProvider.overrideWithValue(wasm),
        localReadinessInspectorProvider.overrideWithValue(local),
        attachClientProvider.overrideWithValue(client),
        hostAttachStoreProvider.overrideWithValue(store),
      ],
    );
    addTearDown(result.dispose);
    return result;
  }

  test('non_external_tools_never_call_local_or_network', () async {
    var localCalls = 0;
    final client = FakeReadinessAttachClient();
    final target = (
      toolId: ToolId.parse('num.hex'),
      isExternal: false,
      boardKey: null,
    );
    final c = container(
      client: client,
      local: ({required toolId, boardKey}) async {
        localCalls++;
        return readiness(ExternalReadinessStatusDto.ready);
      },
    );
    expect(await resolve(c, target), isA<ExternalReadinessNotApplicable>());
    expect(localCalls, 0);
    expect(client.inspectCalls, 0);
  });

  test('native_null_local_result_is_not_blocked', () async {
    final c = container(
      client: FakeReadinessAttachClient(),
      local: ({required toolId, boardKey}) async => null,
    );
    expect(await resolve(c, external), isA<ExternalReadinessNotApplicable>());
  });

  test(
    'native_missing_then_invalidate_then_ready_rechecks_once_per_cache',
    () async {
      var calls = 0;
      var current = readiness(ExternalReadinessStatusDto.missingExecutable);
      final c = container(
        client: FakeReadinessAttachClient(),
        local: ({required toolId, boardKey}) async {
          calls++;
          return current;
        },
      );
      final provider = externalReadinessProvider(external);
      expect(
        (await resolve(c, external) as ExternalReadinessInspected)
            .readiness
            .status,
        ExternalReadinessStatusDto.missingExecutable,
      );
      expect(
        (await resolve(c, external) as ExternalReadinessInspected)
            .readiness
            .status,
        ExternalReadinessStatusDto.missingExecutable,
      );
      expect(calls, 1);
      current = readiness(ExternalReadinessStatusDto.ready);
      c.invalidate(provider);
      expect(
        (await resolve(c, external) as ExternalReadinessInspected)
            .readiness
            .status,
        ExternalReadinessStatusDto.ready,
      );
      expect(calls, 2);
    },
  );

  test('web_unpaired_does_not_call_local_inspector', () async {
    var localCalls = 0;
    final c = container(
      wasm: true,
      client: FakeReadinessAttachClient(),
      local: ({required toolId, boardKey}) async {
        localCalls++;
        return readiness(ExternalReadinessStatusDto.ready);
      },
    );
    expect(await resolve(c, external), isA<ExternalReadinessHostUnpaired>());
    expect(localCalls, 0);
  });

  test('web_paired_uses_host_result_and_passes_board_key', () async {
    final client = FakeReadinessAttachClient(
      readinessResult: AttachReadinessOk(
        readiness(ExternalReadinessStatusDto.ready),
      ),
    );
    final c = container(
      wasm: true,
      config: const HostAttachConfig(baseUrl: 'http://host', token: 't'),
      client: client,
      local: ({required toolId, boardKey}) async => null,
    );
    final target = (
      toolId: external.toolId,
      isExternal: true,
      boardKey: 'board-a',
    );
    expect(await resolve(c, target), isA<ExternalReadinessInspected>());
    expect(client.lastBoardKey, 'board-a');
  });

  test('separate_board_keys_are_separate_cache_entries', () async {
    final client = FakeReadinessAttachClient(
      readinessResult: AttachReadinessOk(
        readiness(ExternalReadinessStatusDto.ready),
      ),
    );
    final c = container(
      wasm: true,
      config: const HostAttachConfig(baseUrl: 'http://host', token: 't'),
      client: client,
      local: ({required toolId, boardKey}) async => null,
    );
    await resolve(c, (
      toolId: external.toolId,
      isExternal: true,
      boardKey: 'a',
    ));
    await resolve(c, (
      toolId: external.toolId,
      isExternal: true,
      boardKey: 'b',
    ));
    expect(client.inspectCalls, 2);
  });

  test('config_change_invalidates_cached_host_readiness', () async {
    final client = FakeReadinessAttachClient(
      readinessResult: AttachReadinessOk(
        readiness(ExternalReadinessStatusDto.missingExecutable),
      ),
    );
    final c = container(
      wasm: true,
      config: const HostAttachConfig(baseUrl: 'http://host', token: 't'),
      client: client,
      local: ({required toolId, boardKey}) async => null,
    );
    await resolve(c, external);
    expect(client.inspectCalls, 1);
    client.readinessResult = AttachReadinessOk(
      readiness(ExternalReadinessStatusDto.ready),
    );
    c
        .read(hostAttachConfigProvider.notifier)
        .save(const HostAttachConfig(baseUrl: 'http://other-host', token: 't'));
    final result = await resolve(c, external) as ExternalReadinessInspected;
    expect(result.readiness.status, ExternalReadinessStatusDto.ready);
    expect(client.inspectCalls, 2);
  });

  test('host_unauthorized_and_malformed_results_are_errors', () async {
    for (final result in <AttachReadinessResult>[
      const AttachReadinessUnauthorized(),
      const AttachReadinessMalformed(),
    ]) {
      final c = container(
        wasm: true,
        config: const HostAttachConfig(baseUrl: 'http://host', token: 't'),
        client: FakeReadinessAttachClient(readinessResult: result),
        local: ({required toolId, boardKey}) async => null,
      );
      final provider = externalReadinessProvider(external);
      final sub = c.listen(provider, (_, _) {});
      addTearDown(sub.close);
      await Future<void>.delayed(Duration.zero);
      final value = c.read(provider);
      expect(value.hasError, isTrue);
      expect(value.error, isA<Exception>());
    }
  });
}

final class _MemoryStore implements HostAttachStore {
  _MemoryStore(this.value);
  HostAttachConfig value;
  @override
  HostAttachConfig read() => value;
  @override
  void write(HostAttachConfig config) => value = config;
}
