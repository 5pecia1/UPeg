/// PWA host-attach routing tests (Task B3).
///
/// On the wasm runtime an in-process-unsupported tool routes through the
/// paired daemon when a host is configured, and falls back to the honest
/// "unsupported" state (with a pairing nudge) when it is not. A fake
/// [AttachClient] + provider overrides keep every case off the network and
/// off the native dylib.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/features/host_attach/attach_client.dart';
import 'package:upeg/src/features/host_attach/host_attach_config.dart';
import 'package:upeg/src/features/host_attach/host_attach_config_provider.dart';
import 'package:upeg/src/features/host_attach/host_attach_dispatch_provider.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/capability.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/capability_provider.dart';
import 'package:upeg/src/state/last_outcome_provider.dart';
import 'package:upeg/src/widgets/board_canvas.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';
import 'package:upeg/src/widgets/host_attach_notice_body.dart';
import 'package:upeg/src/widgets/pin.dart';
import 'package:upeg/src/widgets/surface_unsupported_body.dart';

import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/dispatch_stream_fixture.dart';

/// A plain runnable Function pin whose capability is injected as
/// unsupported on the wasm runtime.
ToolDto _remoteTool() => ToolDto(
  id: 'net.fetch',
  toolkit: 'net',
  label: 'Fetch URL',
  description: 'Fetch a URL over HTTP.',
  tags: const [],
  inputFields: const [],
  outputFields: const [],
  pinKind: PinKindDto.inline,
  invoker: InvokerDto.function,
  pegboardUnits: PegboardUnitsDto.u1,
  source: const SourceDto.userInput(),
  requiresApproval: false,
  approvalSurfaces: const <String>[],
  effect: ToolEffectDto.unknown,
);

const LayoutSnapshotDto _snapshot = LayoutSnapshotDto(
  boardKey: 'dev',
  boardCols: 6,
  placements: [
    PlacementDto(
      toolId: 'net.fetch',
      pinId: 'net.fetch',
      x: 0,
      y: 0,
      w: 1,
      h: 1,
    ),
  ],
);

class _FakeStore implements HostAttachStore {
  _FakeStore(this.config);
  HostAttachConfig config;
  @override
  HostAttachConfig read() => config;
  @override
  void write(HostAttachConfig next) => config = next;
}

class _FakeAttachClient implements AttachClient {
  _FakeAttachClient({
    this.dispatchResult = const AttachDispatchUnreachable(),
    this.resultForArgs,
  });

  final AttachDispatchResult dispatchResult;
  final AttachDispatchResult Function(ToolArgs)? resultForArgs;
  int dispatchCalls = 0;
  final List<ToolArgs> dispatchedArgs = [];

  @override
  Future<HealthzResult> checkHealth() async => const HealthzUnreachable();

  @override
  Future<AttachListResult> listTools() async => const AttachListOk([]);

  @override
  Future<AttachReadinessResult> inspectReadiness({
    required ToolId toolId,
    String? boardKey,
  }) async => const AttachReadinessUnreachable();

  @override
  Future<AttachDispatchResult> dispatch({
    required ToolId toolId,
    required ToolArgs args,
    String? boardKey,
  }) async {
    dispatchCalls += 1;
    dispatchedArgs.add(args);
    return resultForArgs?.call(args) ?? dispatchResult;
  }
}

CanonicalToolResult _okResult(String value) => CanonicalToolResult(
  ok: true,
  primaryOutputId: 'answer',
  outputs: [
    CanonicalOutputEntry(
      id: 'answer',
      label: 'answer',
      kind: 'string',
      value: CanonicalOutputValue.string(value: value),
    ),
  ],
);

Future<void> _pump(
  WidgetTester tester, {
  required List<Override> overrides,
  PinTapCallback? onPinTap,
}) async {
  await tester.pumpWidget(
    ProviderScope(
      overrides: [
        ...i18nTestOverrides,
        toolsLoaderProvider.overrideWith(
          (ref) =>
              () => [_remoteTool()],
        ),
        layoutLoaderProvider.overrideWithValue((query) => _snapshot),
        ...overrides,
      ],
      child: MaterialApp(
        home: Scaffold(
          body: debugBoardCanvasGrid(
            snapshot: _snapshot,
            onPinTap: onPinTap ?? (_) {},
          ),
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();
}

void main() {
  test(
    'pinned host dispatch uses each placement preset and caller overrides',
    () async {
      final client = _FakeAttachClient(
        resultForArgs: (args) =>
            AttachDispatchOk(_okResult(args['input'] as String)),
      );
      final container = ProviderContainer(
        overrides: [
          attachClientProvider.overrideWithValue(client),
          layoutLoaderProvider.overrideWithValue(
            (_) => const LayoutSnapshotDto(
              boardKey: 'dev',
              boardCols: 6,
              placements: [
                PlacementDto(
                  toolId: 'net.fetch',
                  pinId: 'pin-a',
                  x: 0,
                  y: 0,
                  w: 1,
                  h: 1,
                  argsPresetJson: '{"input":"0x10"}',
                ),
                PlacementDto(
                  toolId: 'net.fetch',
                  pinId: 'pin-b',
                  x: 1,
                  y: 0,
                  w: 1,
                  h: 1,
                  argsPresetJson: '{"input":"0x20"}',
                ),
              ],
            ),
          ),
          lastOutcomePersistProvider.overrideWithValue(
            ({
              required boardKey,
              required pinId,
              required toolId,
              required result,
            }) {},
          ),
        ],
      );
      addTearDown(container.dispose);
      final board = BoardKey.parse('dev');
      final pinA = (board, PinId.parse('pin-a'));
      final pinB = (board, PinId.parse('pin-b'));
      final tool = ToolId.parse('net.fetch');
      final dispatch = container.read(hostAttachDispatchProvider.notifier);

      await dispatch.run(tool, ToolArgs.empty, pinKey: pinA);
      await dispatch.run(tool, ToolArgs.empty, pinKey: pinB);

      expect(client.dispatchedArgs.map((args) => args['input']), [
        '0x10',
        '0x20',
      ]);
      expect(
        container.read(pinLastOutcomeProvider(pinA))?.result.primaryOutputText,
        '0x10',
      );
      expect(
        container.read(pinLastOutcomeProvider(pinB))?.result.primaryOutputText,
        '0x20',
      );
      expect(
        container.read(hostAttachDispatchProvider).keys,
        containsAll([pinA, pinB]),
      );

      await dispatch.run(
        tool,
        ToolArgs.fromJsonObject(const {'input': '0x30'}),
        pinKey: pinB,
      );

      expect(client.dispatchedArgs.last['input'], '0x30');
      expect(
        container.read(pinLastOutcomeProvider(pinA))?.result.primaryOutputText,
        '0x10',
      );
      expect(
        container.read(pinLastOutcomeProvider(pinB))?.result.primaryOutputText,
        '0x30',
      );
      expect(client.dispatchCalls, 3);

      final missingPin = await dispatch.run(
        tool,
        ToolArgs.empty,
        pinKey: (board, PinId.parse('missing-pin')),
      );
      final wrongTool = await dispatch.run(
        ToolId.parse('net.other'),
        ToolArgs.empty,
        pinKey: pinA,
      );

      expect(
        missingPin,
        isA<AttachDispatchToolError>().having(
          (result) => result.error.code,
          'code',
          kAttachInvalidPinErrorCode,
        ),
      );
      expect(
        wrongTool,
        isA<AttachDispatchToolError>().having(
          (result) => result.error.code,
          'code',
          kAttachInvalidPinErrorCode,
        ),
      );
      expect(client.dispatchCalls, 3);
    },
  );

  group('host attach routing', () {
    testWidgets(
      'an_unsupported_tool_runs_remotely_against_the_configured_host',
      (tester) async {
        final client = _FakeAttachClient(
          dispatchResult: AttachDispatchOk(_okResult('done')),
        );
        await _pump(
          tester,
          overrides: [
            isWasmRuntimeProvider.overrideWithValue(true),
            toolCapabilityFnProvider.overrideWithValue(
              (_) => const DispatchCapabilityDto.unsupported(
                reason: UnsupportedReasonDto.noProcessSpawn,
              ),
            ),
            hostAttachStoreProvider.overrideWithValue(
              _FakeStore(
                const HostAttachConfig(
                  baseUrl: 'http://127.0.0.1:7173',
                  token: 'tok',
                ),
              ),
            ),
            attachClientProvider.overrideWithValue(client),
          ],
        );

        // Configured host: no unsupported notice, and tapping dispatches
        // remotely instead of opening the in-process path.
        expect(find.byKey(surfaceUnsupportedBodyKey), findsNothing);
        await tester.tap(find.byType(Pin));
        await tester.pumpAndSettle();
        expect(client.dispatchCalls, 1);
      },
    );

    testWidgets('a_remote_result_renders_inline_in_the_pin_body', (
      tester,
    ) async {
      final client = _FakeAttachClient(
        dispatchResult: AttachDispatchOk(_okResult('42')),
      );
      await _pump(
        tester,
        overrides: [
          isWasmRuntimeProvider.overrideWithValue(true),
          toolCapabilityFnProvider.overrideWithValue(
            (_) => const DispatchCapabilityDto.unsupported(
              reason: UnsupportedReasonDto.noProcessSpawn,
            ),
          ),
          hostAttachStoreProvider.overrideWithValue(
            _FakeStore(const HostAttachConfig(baseUrl: 'http://h', token: 't')),
          ),
          attachClientProvider.overrideWithValue(client),
        ],
      );

      expect(find.text('42'), findsNothing);
      await tester.tap(find.byType(Pin));
      await tester.pumpAndSettle();
      // The canonical remote output renders through the normal pin body.
      expect(find.text('42'), findsOneWidget);
      expect(find.byKey(hostAttachNoticeBodyKey), findsNothing);
    });

    testWidgets('without_a_host_the_unsupported_notice_shows_a_connect_hint', (
      tester,
    ) async {
      await _pump(
        tester,
        overrides: [
          isWasmRuntimeProvider.overrideWithValue(true),
          toolCapabilityFnProvider.overrideWithValue(
            (_) => const DispatchCapabilityDto.unsupported(
              reason: UnsupportedReasonDto.noProcessSpawn,
            ),
          ),
          hostAttachStoreProvider.overrideWithValue(
            _FakeStore(HostAttachConfig.empty),
          ),
        ],
      );

      expect(find.byKey(surfaceUnsupportedBodyKey), findsOneWidget);
      expect(
        find.text(i18nEn(surfaceUnsupportedAttachHintKey)),
        findsOneWidget,
      );
    });

    testWidgets('an_unauthorized_result_shows_the_token_guidance', (
      tester,
    ) async {
      final client = _FakeAttachClient(
        dispatchResult: const AttachDispatchUnauthorized(),
      );
      await _pump(
        tester,
        overrides: [
          isWasmRuntimeProvider.overrideWithValue(true),
          toolCapabilityFnProvider.overrideWithValue(
            (_) => const DispatchCapabilityDto.unsupported(
              reason: UnsupportedReasonDto.noProcessSpawn,
            ),
          ),
          hostAttachStoreProvider.overrideWithValue(
            _FakeStore(
              const HostAttachConfig(baseUrl: 'http://h', token: 'bad'),
            ),
          ),
          attachClientProvider.overrideWithValue(client),
        ],
      );

      await tester.tap(find.byType(Pin));
      await tester.pumpAndSettle();
      expect(find.byKey(hostAttachNoticeBodyKey), findsOneWidget);
      expect(find.text(i18nEn(kHostAttachUnauthorizedHintKey)), findsOneWidget);
    });

    testWidgets('a_host_503_response_shows_its_hint', (tester) async {
      // Daemon-authored hint text passes through verbatim; only the
      // client's own kHostUnavailableDefaultHint fallback localizes.
      const hint = 'daemon is restarting, retry shortly';
      final client = _FakeAttachClient(
        dispatchResult: const AttachDispatchUnavailable(hint),
      );
      await _pump(
        tester,
        overrides: [
          isWasmRuntimeProvider.overrideWithValue(true),
          toolCapabilityFnProvider.overrideWithValue(
            (_) => const DispatchCapabilityDto.unsupported(
              reason: UnsupportedReasonDto.noProcessSpawn,
            ),
          ),
          hostAttachStoreProvider.overrideWithValue(
            _FakeStore(const HostAttachConfig(baseUrl: 'http://h', token: 't')),
          ),
          attachClientProvider.overrideWithValue(client),
        ],
      );

      await tester.tap(find.byType(Pin));
      await tester.pumpAndSettle();
      expect(find.byKey(hostAttachNoticeBodyKey), findsOneWidget);
      expect(find.text(hint), findsOneWidget);
    });

    testWidgets('a_native_only_tool_runs_remotely_once_the_host_is_connected', (
      tester,
    ) async {
      // A native-only built-in (eth.gas-style): the browser has no
      // dispatcher, but a paired daemon links the native one. NativeOnlyTool
      // is now attach-solvable, so a configured host routes it remotely.
      final client = _FakeAttachClient(
        dispatchResult: AttachDispatchOk(_okResult('12')),
      );
      await _pump(
        tester,
        overrides: [
          isWasmRuntimeProvider.overrideWithValue(true),
          toolCapabilityFnProvider.overrideWithValue(
            (_) => const DispatchCapabilityDto.unsupported(
              reason: UnsupportedReasonDto.nativeOnlyTool,
            ),
          ),
          hostAttachStoreProvider.overrideWithValue(
            _FakeStore(const HostAttachConfig(baseUrl: 'http://h', token: 't')),
          ),
          attachClientProvider.overrideWithValue(client),
        ],
      );

      // Configured host: no unsupported notice, and the canonical remote
      // output renders inline through the normal pin body.
      expect(find.byKey(surfaceUnsupportedBodyKey), findsNothing);
      expect(find.text('12'), findsNothing);
      await tester.tap(find.byType(Pin));
      await tester.pumpAndSettle();
      expect(client.dispatchCalls, 1);
      expect(find.text('12'), findsOneWidget);
    });

    testWidgets('a_host_reporting_unsupported_is_surfaced_honestly', (
      tester,
    ) async {
      // The daemon answered but reports the tool isn't runnable on its
      // surface — surface the tool error honestly instead of crashing.
      final client = _FakeAttachClient(
        dispatchResult: const AttachDispatchToolError(
          CanonicalToolError(
            code: 'tool_not_on_surface',
            message: 'tool not available on host surface',
          ),
        ),
      );
      await _pump(
        tester,
        overrides: [
          isWasmRuntimeProvider.overrideWithValue(true),
          toolCapabilityFnProvider.overrideWithValue(
            (_) => const DispatchCapabilityDto.unsupported(
              reason: UnsupportedReasonDto.nativeOnlyTool,
            ),
          ),
          hostAttachStoreProvider.overrideWithValue(
            _FakeStore(const HostAttachConfig(baseUrl: 'http://h', token: 't')),
          ),
          attachClientProvider.overrideWithValue(client),
        ],
      );

      await tester.tap(find.byType(Pin));
      await tester.pumpAndSettle();
      expect(find.byKey(hostAttachNoticeBodyKey), findsOneWidget);
      expect(find.text('tool not available on host surface'), findsOneWidget);
    });

    testWidgets('the_native_runtime_never_uses_attach', (tester) async {
      final client = _FakeAttachClient(
        dispatchResult: AttachDispatchOk(_okResult('x')),
      );
      var consulted = false;
      await _pump(
        tester,
        overrides: [
          // Default isWasmRuntimeProvider is kIsWeb == false on the VM.
          toolCapabilityFnProvider.overrideWithValue((_) {
            consulted = true;
            return const DispatchCapabilityDto.supported();
          }),
          hostAttachStoreProvider.overrideWithValue(
            _FakeStore(const HostAttachConfig(baseUrl: 'http://h', token: 't')),
          ),
          attachClientProvider.overrideWithValue(client),
          // `_remoteTool` is Inline/Function with no inputs, so on native it
          // is now a generic-inline-body candidate (item 7 fix) and mounts
          // its own Run button instead of the old plain body. Tapping the
          // Pin's bounding box lands on that Run button, so the in-process
          // live dispatcher needs a fake too — same "off the native dylib"
          // guarantee this file promises, just reached via a different seam.
          ...dispatchOverrides(
            ({pinKey, required toolId, required args}) async =>
                _okResult('local'),
          ),
        ],
      );

      expect(
        consulted,
        isFalse,
        reason: 'the native runtime must not consult capabilities',
      );
      expect(find.byKey(surfaceUnsupportedBodyKey), findsNothing);
      await tester.tap(find.byType(Pin));
      await tester.pumpAndSettle();
      expect(
        client.dispatchCalls,
        0,
        reason: 'native must not route through attach',
      );
    });
  });
}
