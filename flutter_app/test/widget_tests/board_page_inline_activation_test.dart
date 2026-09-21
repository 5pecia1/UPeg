/// Widget tests for the BoardPage keyboard wiring (G5).
///
/// The keyboard FRB itself is exercised in upeg-frb Rust unit tests
/// (`keyboard_command_for_maps_*`). Here we verify that the Dart side
/// translates Flutter `KeyEvent`s into the right FRB call, and that
/// the resulting [`KeyboardCommandDto`] drives the right page-level
/// side effect (open palette / open settings / cycle tag / …).
///
/// The resolver provider is overridden so a stub can return a
/// pre-baked [`KeyboardCommandDto`] without touching the Rust dylib.
library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/pages/expanded_modal_page.dart';
import 'package:upeg/src/rust/api/keyboard.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/pin_activation.dart';
import 'package:upeg/src/rust/api/memos.dart' show MemoEntry;
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/features/memos/memos_provider.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart' show ToolArgs;
import 'package:upeg/src/widgets/pin.dart';

import '../test_helpers/tool_fixture.dart';
import '../test_helpers/board_page_harness.dart';
import '../test_helpers/dispatch_stream_fixture.dart';

void main() {
  group('inline-first activation', () {
    testWidgets(
      'a_zero_arg_inline_pin_renders_the_inline_result_on_a_single_tap_without_opening_the_modal',
      (tester) async {
        const toolId = 'demo.inline';
        await tester.pumpWidget(
          boardPageHarness(
            currentBoardKey: 'dev',
            tools: [fixtureToolDto(id: toolId, label: 'Inline demo')],
            layoutLoader: (query) => LayoutSnapshotDto(
              boardKey: query.boardKey.value,
              boardCols: 6,
              placements: const <PlacementDto>[
                PlacementDto(toolId: toolId, x: 0, y: 0, w: 1, h: 1),
              ],
            ),
            pinActivation: ({required toolId, required argsJson}) =>
                PinActivationDto.dispatchImmediate(toolId: toolId.value),
            resolver: noKeyboardCommand,
            extraOverrides: [
              ...dispatchOverrides(
                ({required ToolId toolId, required ToolArgs args}) async =>
                    const CanonicalToolResult(
                      ok: true,
                      primaryOutputId: 'out',
                      outputs: [
                        CanonicalOutputEntry(
                          id: 'out',
                          kind: 'string',
                          value: CanonicalOutputValue.string(
                            value: 'INLINE-42',
                          ),
                        ),
                      ],
                    ),
              ),
            ],
          ),
        );
        await tester.pumpAndSettle();

        await tester.tap(find.byType(Pin));
        await tester.pumpAndSettle();

        // Result is rendered inline in the pin body, no modal opened.
        expect(find.text('INLINE-42'), findsOneWidget);
        expect(find.byType(ExpandedModalPage), findsNothing);
      },
    );

    testWidgets('the_modal_opens_only_on_an_explicit_Open_gesture', (
      tester,
    ) async {
      await tester.pumpWidget(
        boardPageHarness(
          currentBoardKey: 'dev',
          focusedToolId: 'num.hex_to_decimal',
          tools: [
            fixtureToolDto(id: 'num.hex_to_decimal', label: 'Hex to Dec'),
          ],
          layoutLoader: (query) => LayoutSnapshotDto(
            boardKey: query.boardKey.value,
            boardCols: 6,
            placements: const <PlacementDto>[
              PlacementDto(
                toolId: 'num.hex_to_decimal',
                x: 0,
                y: 0,
                w: 1,
                h: 1,
              ),
            ],
          ),
          pinActivation: ({required toolId, required argsJson}) =>
              PinActivationDto.openModal(toolId: toolId.value),
          resolver:
              ({
                required String key,
                required bool ctrl,
                required bool meta,
                required bool shift,
                required bool alt,
                required bool hasToolFocus,
                required KeyboardScopeDto scope,
              }) {
                if (key == 'o') return const KeyboardCommandDto.open();
                return null;
              },
        ),
      );
      await tester.pumpAndSettle();
      expect(find.byType(ExpandedModalPage), findsNothing);

      await tester.sendKeyEvent(LogicalKeyboardKey.keyO);
      await tester.pumpAndSettle();

      expect(find.byType(ExpandedModalPage), findsOneWidget);
    });

    testWidgets('memo_create_creates_a_new_memo_on_Cmd_Shift_N', (
      tester,
    ) async {
      final saved = <List<MemoEntry>>[];
      await tester.pumpWidget(
        boardPageHarness(
          currentBoardKey: 'dev',
          tools: [
            fixtureToolDto(
              id: 'memo.create',
              toolkit: 'memo',
              label: 'New memo',
              pinKind: PinKindDto.action,
              source: const SourceDto.shortcut(keys: 'Cmd+Shift+N'),
            ),
          ],
          pinActivation: ({required toolId, required argsJson}) =>
              PinActivationDto.dispatchImmediate(toolId: toolId.value),
          resolver: noKeyboardCommand,
          extraOverrides: [
            memosLoaderProvider.overrideWithValue(() => const <MemoEntry>[]),
            memosSaverProvider.overrideWithValue(
              (entries) => saved.add(entries),
            ),
          ],
        ),
      );
      await tester.pumpAndSettle();

      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
      final result = dispatchBoardPageKey(
        tester,
        logicalKey: LogicalKeyboardKey.keyN,
        physicalKey: PhysicalKeyboardKey.keyN,
        character: 'N',
      );
      await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      await tester.pumpAndSettle();

      expect(result, KeyEventResult.handled);
      expect(saved, isNotEmpty);
      expect(saved.last, hasLength(1));
    });
  });
}
