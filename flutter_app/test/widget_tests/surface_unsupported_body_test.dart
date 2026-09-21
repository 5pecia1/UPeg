import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/capability.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/capability_provider.dart';
import 'package:upeg/src/widgets/board_canvas.dart';
import 'package:upeg/src/widgets/surface_unsupported_body.dart';

import '../test_helpers/i18n_test_catalog.dart';

/// A native-only built-in (e.g. `media.image_to_pdf`) modelled as a plain
/// runnable Function pin. Its real capability on wasm is decided in Rust;
/// here the verdict is injected via [toolCapabilityFnProvider].
ToolDto _mediaTool() => ToolDto(
  id: 'media.image_to_pdf',
  toolkit: 'media',
  label: 'Image to PDF',
  description: 'Combine images into a PDF.',
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

void main() {
  group('hostAttachCanSolve', () {
    test('every_unsupported_reason_is_solvable_by_host_attach', () {
      // Every wasm/PWA-unsupported reason — including nativeOnlyTool — is
      // attach-solvable now that a paired daemon links the native-gated
      // dispatchers the browser lacks.
      for (final reason in UnsupportedReasonDto.values) {
        expect(
          hostAttachCanSolve(reason),
          isTrue,
          reason: '$reason should be runnable via host attach',
        );
      }
    });
  });

  group('SurfaceUnsupportedBody', () {
    testWidgets('renders_a_unique_hint_for_each_unsupported_reason', (
      tester,
    ) async {
      for (final reason in UnsupportedReasonDto.values) {
        await tester.pumpWidget(
          ProviderScope(
            overrides: [...i18nTestOverrides],
            child: MaterialApp(
              home: Scaffold(body: SurfaceUnsupportedBody(reason: reason)),
            ),
          ),
        );
        expect(find.byKey(surfaceUnsupportedBodyKey), findsOneWidget);
        expect(find.text(i18nEn(surfaceUnsupportedLabelKey)), findsOneWidget);
        expect(
          find.text(i18nEn(surfaceUnsupportedHintKey(reason))),
          findsOneWidget,
        );
      }
    });
  });

  group('board_canvas capability', () {
    testWidgets(
      'media_tool_renders_the_unsupported_state_on_the_wasm_runtime',
      (tester) async {
        final media = _mediaTool();
        await tester.pumpWidget(
          ProviderScope(
            overrides: [
              ...i18nTestOverrides,
              toolsLoaderProvider.overrideWith(
                (ref) =>
                    () => [media],
              ),
              isWasmRuntimeProvider.overrideWithValue(true),
              toolCapabilityFnProvider.overrideWithValue(
                (toolId) => const DispatchCapabilityDto.unsupported(
                  reason: UnsupportedReasonDto.nativeOnlyTool,
                ),
              ),
            ],
            child: MaterialApp(
              home: Scaffold(
                body: debugBoardCanvasGrid(
                  snapshot: const LayoutSnapshotDto(
                    boardKey: 'dev',
                    boardCols: 6,
                    placements: [
                      PlacementDto(
                        toolId: 'media.image_to_pdf',
                        x: 0,
                        y: 0,
                        w: 1,
                        h: 1,
                      ),
                    ],
                  ),
                  onPinTap: (_) {},
                ),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();

        expect(find.byKey(surfaceUnsupportedBodyKey), findsOneWidget);
        expect(
          find.text(
            i18nEn(
              surfaceUnsupportedHintKey(UnsupportedReasonDto.nativeOnlyTool),
            ),
          ),
          findsOneWidget,
        );
      },
    );

    testWidgets('supported_tools_remain_runnable_as_before', (tester) async {
      final media = _mediaTool();
      var consulted = false;
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            ...i18nTestOverrides,
            toolsLoaderProvider.overrideWith(
              (ref) =>
                  () => [media],
            ),
            isWasmRuntimeProvider.overrideWithValue(true),
            toolCapabilityFnProvider.overrideWithValue((toolId) {
              consulted = true;
              return const DispatchCapabilityDto.supported();
            }),
          ],
          child: MaterialApp(
            home: Scaffold(
              body: debugBoardCanvasGrid(
                snapshot: const LayoutSnapshotDto(
                  boardKey: 'dev',
                  boardCols: 6,
                  placements: [
                    PlacementDto(
                      toolId: 'media.image_to_pdf',
                      x: 0,
                      y: 0,
                      w: 1,
                      h: 1,
                    ),
                  ],
                ),
                onPinTap: (_) {},
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(
        consulted,
        isTrue,
        reason: 'capability must be consulted on the wasm runtime',
      );
      // Supported tools keep their normal pin body — no unsupported notice.
      expect(find.byKey(surfaceUnsupportedBodyKey), findsNothing);
    });

    testWidgets('capability_is_not_consulted_on_the_native_runtime', (
      tester,
    ) async {
      final media = _mediaTool();
      var consulted = false;
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            ...i18nTestOverrides,
            toolsLoaderProvider.overrideWith(
              (ref) =>
                  () => [media],
            ),
            // Default isWasmRuntimeProvider is kIsWeb == false on the test host.
            toolCapabilityFnProvider.overrideWithValue((toolId) {
              consulted = true;
              return const DispatchCapabilityDto.supported();
            }),
          ],
          child: MaterialApp(
            home: Scaffold(
              body: debugBoardCanvasGrid(
                snapshot: const LayoutSnapshotDto(
                  boardKey: 'dev',
                  boardCols: 6,
                  placements: [
                    PlacementDto(
                      toolId: 'media.image_to_pdf',
                      x: 0,
                      y: 0,
                      w: 1,
                      h: 1,
                    ),
                  ],
                ),
                onPinTap: (_) {},
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(
        consulted,
        isFalse,
        reason:
            'the native runtime supports every tool, so capability is never consulted',
      );
      expect(find.byKey(surfaceUnsupportedBodyKey), findsNothing);
    });
  });
}
