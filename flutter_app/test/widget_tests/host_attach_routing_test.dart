/// PWA host-attach routing tests (Task B3).
///
/// On the wasm runtime an in-process-unsupported tool routes through the
/// paired daemon when a host is configured, and falls back to the honest
/// "미지원" state (with a pairing nudge) when it is not. A fake
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
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/capability_provider.dart';
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
);

const LayoutSnapshotDto _snapshot = LayoutSnapshotDto(
  boardKey: 'dev',
  boardCols: 6,
  placements: [PlacementDto(toolId: 'net.fetch', x: 0, y: 0, w: 1, h: 1)],
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
  _FakeAttachClient({this.dispatchResult = const AttachDispatchUnreachable()});

  final AttachDispatchResult dispatchResult;
  int dispatchCalls = 0;

  @override
  Future<HealthzResult> checkHealth() async => const HealthzUnreachable();

  @override
  Future<AttachListResult> listTools() async => const AttachListOk([]);

  @override
  Future<AttachDispatchResult> dispatch({
    required ToolId toolId,
    required ToolArgs args,
    String? boardKey,
  }) async {
    dispatchCalls += 1;
    return dispatchResult;
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
  group('host attach routing', () {
    testWidgets('설정된_host로_미지원_도구를_원격_실행한다', (tester) async {
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
    });

    testWidgets('원격_결과는_핀_바디에_인라인_렌더된다', (tester) async {
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

    testWidgets('host_미설정시_미지원_안내에_연결_힌트를_보여준다', (tester) async {
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

    testWidgets('unauthorized는_토큰_안내를_보여준다', (tester) async {
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

    testWidgets('호스트_503_응답은_힌트를_보여준다', (tester) async {
      const hint = 'host temporarily unavailable';
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

    testWidgets('native_only_도구도_host_연결시_원격_실행된다', (tester) async {
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

    testWidgets('호스트가_미지원_보고시_정직하게_알린다', (tester) async {
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

    testWidgets('native_런타임은_attach를_사용하지_않는다', (tester) async {
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
            ({required toolId, required args}) async => _okResult('local'),
          ),
        ],
      );

      expect(consulted, isFalse, reason: 'native 런타임은 capability 를 조회하지 않는다');
      expect(find.byKey(surfaceUnsupportedBodyKey), findsNothing);
      await tester.tap(find.byType(Pin));
      await tester.pumpAndSettle();
      expect(client.dispatchCalls, 0, reason: 'native 는 attach 로 라우팅하지 않는다');
    });
  });
}
