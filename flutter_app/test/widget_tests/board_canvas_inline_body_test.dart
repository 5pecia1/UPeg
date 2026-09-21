import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/capability.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/capability_provider.dart';
import 'package:upeg/src/state/last_outcome_provider.dart';
import 'package:upeg/src/state/pin_provider.dart' show PinKey;
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/board_canvas.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';
import 'package:upeg/src/widgets/inline/generic_inline_pin_body.dart';
import 'package:upeg/src/widgets/surface_unsupported_body.dart';

import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/tool_fixture.dart';
import '../test_helpers/board_canvas_harness.dart';
import '../test_helpers/dispatch_stream_fixture.dart';

void main() {
  group('BoardCanvas', () {
    testWidgets(
      'an_argumentless_inline_pin_renders_a_Run_button_instead_of_a_recorded_result',
      (tester) async {
        final inlineTool = ToolDto(
          id: 'demo.inline',
          toolkit: 'demo',
          label: 'Inline demo',
          description: '',
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
        final container = ProviderContainer(
          overrides: [
            ...i18nTestOverrides,
            toolsLoaderProvider.overrideWith(
              (ref) =>
                  () => [inlineTool],
            ),
            // Recorded-result rendering is independent of the web capability
            // bridge, so keep this fixture on the native contract in Chrome.
            isWasmRuntimeProvider.overrideWithValue(false),
          ],
        );
        addTearDown(container.dispose);
        container
            .read(lastOutcomeProvider.notifier)
            .record(
              ToolId.parse('demo.inline'),
              const CanonicalToolResult(
                ok: true,
                primaryOutputId: 'out',
                outputs: [
                  CanonicalOutputEntry(
                    id: 'out',
                    kind: 'string',
                    value: CanonicalOutputValue.string(value: 'INLINE-99'),
                  ),
                ],
              ),
            );

        await tester.pumpWidget(
          UncontrolledProviderScope(
            container: container,
            child: MaterialApp(
              home: Scaffold(
                body: debugBoardCanvasGrid(
                  snapshot: const LayoutSnapshotDto(
                    boardKey: 'dev',
                    boardCols: 6,
                    placements: [
                      PlacementDto(
                        toolId: 'demo.inline',
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

        // Item 7 fix: an Inline/Function tool with no input fields is now a
        // generic-inline-body candidate regardless of input count, so it
        // mounts `GenericInlinePinBody` (Run button) instead of the plain
        // default body that used to surface `lastOutcomeProvider`'s recorded
        // value directly. `GenericInlinePinBody` owns its own result state
        // and never reads a stale recorded outcome on mount.
        expect(find.byType(GenericInlinePinBody), findsOneWidget);
        expect(find.byKey(inlineRunButtonKey), findsOneWidget);
        expect(find.text('INLINE-99'), findsNothing);
      },
    );

    testWidgets(
      'only_an_Inline_Function_with_inputs_renders_the_generic_body_Launcher_is_excluded',
      (tester) async {
        final inlineTool = genericInputTool(id: 'demo.inline');
        final launcherTool = genericInputTool(
          id: 'demo.launcher',
          pinKind: PinKindDto.launcher,
          pegboardUnits: PegboardUnitsDto.u2,
        );
        await tester.pumpWidget(
          genericInlineHarness(
            tools: <ToolDto>[inlineTool, launcherTool],
            snapshot: const LayoutSnapshotDto(
              boardKey: 'dev',
              boardCols: 6,
              placements: <PlacementDto>[
                PlacementDto(toolId: 'demo.inline', x: 0, y: 0, w: 1, h: 1),
                PlacementDto(toolId: 'demo.launcher', x: 1, y: 0, w: 2, h: 1),
              ],
            ),
            dispatch: ({required toolId, required args}) async =>
                const CanonicalToolResult(
                  ok: true,
                  outputs: <CanonicalOutputEntry>[],
                ),
          ),
        );
        await tester.pump();
        await tester.pump();

        expect(find.byType(GenericInlinePinBody), findsOneWidget);
        expect(
          tester
              .widget<GenericInlinePinBody>(find.byType(GenericInlinePinBody))
              .tool
              .id,
          inlineTool.id,
        );
      },
    );

    testWidgets(
      'an_inputless_Inline_Function_tool_still_exposes_a_Run_button_on_BoardCanvas',
      (tester) async {
        // Regression for the zero-input inline Run affordance: the pin must
        // mount `GenericInlinePinBody` (and its Run button) through the real
        // BoardCanvas render path, not only when mounted directly — a tool
        // declaring no input fields used to fall through to the plain body.
        final zeroInputTool = fixtureToolDto(
          id: 'demo.zero_input',
          label: 'demo.zero_input',
        );
        await tester.pumpWidget(
          genericInlineHarness(
            tools: <ToolDto>[zeroInputTool],
            snapshot: const LayoutSnapshotDto(
              boardKey: 'dev',
              boardCols: 6,
              placements: <PlacementDto>[
                PlacementDto(toolId: 'demo.zero_input', x: 0, y: 0, w: 1, h: 1),
              ],
            ),
            dispatch: ({required toolId, required args}) async =>
                const CanonicalToolResult(
                  ok: true,
                  outputs: <CanonicalOutputEntry>[],
                ),
          ),
        );
        await tester.pump();
        await tester.pump();

        expect(find.byType(GenericInlinePinBody), findsOneWidget);
        expect(find.byKey(inlineRunButtonKey), findsOneWidget);
      },
    );

    testWidgets(
      'a_generic_Timer_pin_does_not_start_a_live_poller_for_empty_arguments',
      (tester) async {
        final calls = <ToolArgs>[];
        final timerTool = genericInputTool(
          id: 'demo.timer',
          source: SourceDto.timer(intervalMs: BigInt.from(1000)),
        );
        await tester.pumpWidget(
          genericInlineHarness(
            tools: <ToolDto>[timerTool],
            snapshot: const LayoutSnapshotDto(
              boardKey: 'dev',
              boardCols: 6,
              placements: <PlacementDto>[
                PlacementDto(toolId: 'demo.timer', x: 0, y: 0, w: 1, h: 1),
              ],
            ),
            dispatch: ({required toolId, required args}) async {
              calls.add(args);
              return const CanonicalToolResult(
                ok: true,
                outputs: <CanonicalOutputEntry>[],
              );
            },
          ),
        );
        await tester.pump();
        await tester.pump();

        expect(find.byType(GenericInlinePinBody), findsOneWidget);
        expect(calls, isEmpty);
      },
    );

    testWidgets(
      'an_input_aware_Timer_unsupported_on_Wasm_blocks_both_the_generic_body_and_empty_argument_polling',
      (tester) async {
        final calls = <ToolArgs>[];
        final timerTool = genericInputTool(
          id: 'demo.unsupported_timer',
          source: SourceDto.timer(intervalMs: BigInt.from(1000)),
        );
        await tester.pumpWidget(
          ProviderScope(
            overrides: [
              ...i18nTestOverrides,
              toolsLoaderProvider.overrideWith(
                (ref) =>
                    () => <ToolDto>[timerTool],
              ),
              ...dispatchOverrides(({required toolId, required args}) async {
                calls.add(args);
                return const CanonicalToolResult(
                  ok: true,
                  outputs: <CanonicalOutputEntry>[],
                );
              }),
              isWasmRuntimeProvider.overrideWithValue(true),
              toolCapabilityFnProvider.overrideWithValue(
                (_) => const DispatchCapabilityDto.unsupported(
                  reason: UnsupportedReasonDto.nativeOnlyTool,
                ),
              ),
            ],
            child: MaterialApp(
              theme: UpegTheme.darkTheme(),
              home: Scaffold(
                body: SizedBox(
                  width: 1200,
                  height: 600,
                  child: debugBoardCanvasGrid(
                    snapshot: const LayoutSnapshotDto(
                      boardKey: 'dev',
                      boardCols: 6,
                      placements: <PlacementDto>[
                        PlacementDto(
                          toolId: 'demo.unsupported_timer',
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
          ),
        );
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 1));

        expect(find.byType(GenericInlinePinBody), findsNothing);
        expect(find.byType(SurfaceUnsupportedBody), findsOneWidget);
        expect(calls, isEmpty);
      },
    );

    testWidgets(
      'input_state_follows_tool_identity_even_when_the_placement_order_changes',
      (tester) async {
        final firstTool = genericInputTool(id: 'demo.first');
        final secondTool = genericInputTool(id: 'demo.second');
        final snapshot = ValueNotifier<LayoutSnapshotDto>(
          const LayoutSnapshotDto(
            boardKey: 'dev',
            boardCols: 6,
            placements: <PlacementDto>[
              PlacementDto(toolId: 'demo.first', x: 0, y: 0, w: 1, h: 1),
              PlacementDto(toolId: 'demo.second', x: 1, y: 0, w: 1, h: 1),
            ],
          ),
        );
        addTearDown(snapshot.dispose);
        await tester.pumpWidget(
          ProviderScope(
            overrides: [
              ...i18nTestOverrides,
              toolsLoaderProvider.overrideWith(
                (ref) =>
                    () => <ToolDto>[firstTool, secondTool],
              ),
              ...dispatchOverrides(
                ({required toolId, required args}) async =>
                    const CanonicalToolResult(
                      ok: true,
                      outputs: <CanonicalOutputEntry>[],
                    ),
              ),
              // Pin identity is the behavior under test, not wasm capability.
              isWasmRuntimeProvider.overrideWithValue(false),
            ],
            child: MaterialApp(
              theme: UpegTheme.darkTheme(),
              home: Scaffold(
                body: SizedBox(
                  width: 1200,
                  height: 600,
                  child: ValueListenableBuilder<LayoutSnapshotDto>(
                    valueListenable: snapshot,
                    builder: (context, value, _) =>
                        debugBoardCanvasGrid(snapshot: value, onPinTap: (_) {}),
                  ),
                ),
              ),
            ),
          ),
        );
        await tester.pump();
        await tester.pump();

        final fields = find.byKey(const Key('field-value'));
        expect(fields, findsNWidgets(2));
        await tester.enterText(fields.at(0), 'first draft');
        await tester.pump();

        snapshot.value = const LayoutSnapshotDto(
          boardKey: 'dev',
          boardCols: 6,
          placements: <PlacementDto>[
            PlacementDto(toolId: 'demo.second', x: 0, y: 0, w: 1, h: 1),
            PlacementDto(toolId: 'demo.first', x: 1, y: 0, w: 1, h: 1),
          ],
        );
        await tester.pump();

        final reorderedFields = find.byKey(const Key('field-value'));
        final firstField = find.descendant(
          of: reorderedFields.at(0),
          matching: find.byType(EditableText),
        );
        final secondField = find.descendant(
          of: reorderedFields.at(1),
          matching: find.byType(EditableText),
        );
        expect(
          tester.widget<EditableText>(firstField).controller.text,
          isEmpty,
        );
        expect(
          tester.widget<EditableText>(secondField).controller.text,
          'first draft',
        );

        final secondBody = find.byWidgetPredicate(
          (widget) =>
              widget is GenericInlinePinBody && widget.tool.id == 'demo.second',
        );
        final positioned = tester.widget<Positioned>(
          find
              .ancestor(of: secondBody, matching: find.byType(Positioned))
              .first,
        );
        final PinKey expectedKey = (
          BoardKey.parse('dev'),
          ToolId.parse('demo.second'),
        );
        expect(positioned.key, ValueKey<PinKey>(expectedKey));
      },
    );

    testWidgets(
      'taps_on_interactive_fields_and_Run_only_trigger_pin_chrome_taps_without_opening_the_modal',
      (tester) async {
        var modalOpenCount = 0;
        final tool = genericInputTool(id: 'demo.interactive');
        await tester.pumpWidget(
          genericInlineHarness(
            tools: <ToolDto>[tool],
            snapshot: const LayoutSnapshotDto(
              boardKey: 'dev',
              boardCols: 6,
              placements: <PlacementDto>[
                PlacementDto(
                  toolId: 'demo.interactive',
                  x: 0,
                  y: 0,
                  w: 1,
                  h: 1,
                ),
              ],
            ),
            dispatch: ({required toolId, required args}) async =>
                const CanonicalToolResult(
                  ok: true,
                  outputs: <CanonicalOutputEntry>[],
                ),
            onOpenModal: (_) => modalOpenCount++,
          ),
        );
        await tester.pump();
        await tester.pump();

        await tester.tap(find.byKey(const Key('field-value')));
        await tester.enterText(find.byKey(const Key('field-value')), 'value');
        await tester.pump();
        expect(modalOpenCount, 0);

        await tester.tap(find.byKey(inlineRunButtonKey));
        await tester.pump();
        expect(modalOpenCount, 0);

        await tester.tap(find.text('DEMO.INTERACTIVE'));
        await tester.pump();
        expect(modalOpenCount, 1);
      },
    );
  });
}
