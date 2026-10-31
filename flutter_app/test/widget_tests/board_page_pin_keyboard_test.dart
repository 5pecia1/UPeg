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
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/pages/board_page.dart';
import 'package:upeg/src/pages/expanded_modal_page.dart';
import 'package:upeg/src/rust/api/keyboard.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/pin_activation.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/state/last_outcome_provider.dart';
import 'package:upeg/src/state/focused_pin_provider.dart';
import 'package:upeg/src/state/dispatch_stream_provider.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

import '../test_helpers/tool_fixture.dart';
import '../test_helpers/board_page_harness.dart';
import '../test_helpers/dispatch_stream_fixture.dart';

class _RecordingWriter extends ClipboardWriter {
  final writes = <String>[];

  @override
  Future<void> write(String text) async => writes.add(text);
}

void main() {
  group('BoardPage keyboard wiring', () {
    testWidgets(
      'new modal pin remains the board F2 target after layout reload',
      (tester) async {
        final writer = _RecordingWriter();
        var added = false;
        final board = BoardKey.parse('dev');
        final oldPin = PinId.parse('old-pin');
        final newPin = PinId.parse('new-pin');
        final tool = fixtureToolDto(
          id: 'fixture.modal_copy',
          label: 'Modal copy',
          source: const SourceDto.manual(),
          inputFields: const [],
        );
        await tester.pumpWidget(
          boardPageHarness(
            currentBoardKey: board.value,
            focusedToolId: oldPin.value,
            tools: [tool],
            layoutLoader: (query) => LayoutSnapshotDto(
              boardKey: query.boardKey.value,
              boardCols: 6,
              placements: [
                PlacementDto(
                  toolId: tool.id,
                  pinId: oldPin.value,
                  x: 0,
                  y: 0,
                  w: 1,
                  h: 1,
                ),
                if (added)
                  PlacementDto(
                    toolId: tool.id,
                    pinId: newPin.value,
                    x: 1,
                    y: 0,
                    w: 1,
                    h: 1,
                  ),
              ],
            ),
            extraOverrides: [
              clipboardWriterProvider.overrideWithValue(writer),
              addPinMutatorProvider.overrideWithValue((_, _) {
                added = true;
                return newPin.value;
              }),
              dispatchStreamFnProvider.overrideWithValue(
                stubDispatchStream(
                  ({required toolId, required args, required approve}) async =>
                      const CanonicalToolResult(
                        ok: true,
                        primaryOutputId: 'result',
                        outputs: [
                          CanonicalOutputEntry(
                            id: 'result',
                            kind: 'string',
                            value: CanonicalOutputValue.string(
                              value: 'new pin output',
                            ),
                          ),
                        ],
                      ),
                ),
              ),
            ],
            resolver:
                ({
                  required key,
                  required ctrl,
                  required meta,
                  required shift,
                  required alt,
                  required scope,
                  required hasToolFocus,
                }) => switch (key) {
                  'o' => const KeyboardCommandDto.open(),
                  'F2' => const KeyboardCommandDto.copy(),
                  _ => null,
                },
          ),
        );
        await tester.pumpAndSettle();
        final container = ProviderScope.containerOf(
          tester.element(find.byType(BoardPage)),
        );
        container
            .read(lastOutcomeProvider.notifier)
            .record(
              (board, oldPin),
              ToolId.parse(tool.id),
              const CanonicalToolResult(
                ok: true,
                primaryOutputId: 'result',
                outputs: [
                  CanonicalOutputEntry(
                    id: 'result',
                    kind: 'string',
                    value: CanonicalOutputValue.string(value: 'old pin output'),
                  ),
                ],
              ),
            );

        await tester.sendKeyEvent(LogicalKeyboardKey.keyO);
        await tester.pumpAndSettle();
        expect(find.byType(ExpandedModalPage), findsOneWidget);
        await tester.tap(find.byKey(const Key('expanded-modal-pin-btn')));
        await tester.pumpAndSettle();
        expect(container.read(focusedPinProvider), newPin);
        await tester.tap(find.byKey(const Key('expanded-modal-run-btn')));
        await tester.pumpAndSettle();
        await tester.tap(find.byKey(const Key('expanded-modal-close-btn')));
        await tester.pumpAndSettle();
        expect(container.read(focusedPinProvider), newPin);

        await tester.sendKeyEvent(LogicalKeyboardKey.f2);
        await tester.pump();
        expect(writer.writes, ['new pin output']);
      },
    );

    testWidgets('F2 copies the focused pin result while its input has focus', (
      tester,
    ) async {
      final writer = _RecordingWriter();
      const pinId = 'fixture.manual';
      final tool = fixtureToolDto(
        id: pinId,
        label: 'Manual',
        source: const SourceDto.manual(),
        inputFields: const [
          InputFieldDto(
            key: 'input',
            label: 'Input',
            fieldType: InputFieldType.text(),
            required_: true,
          ),
        ],
      );
      await tester.pumpWidget(
        boardPageHarness(
          currentBoardKey: 'dev',
          focusedToolId: pinId,
          tools: [tool],
          layoutLoader: (query) => LayoutSnapshotDto(
            boardKey: query.boardKey.value,
            boardCols: 6,
            placements: const [
              PlacementDto(toolId: pinId, pinId: pinId, x: 0, y: 0, w: 2, h: 2),
            ],
          ),
          extraOverrides: [clipboardWriterProvider.overrideWithValue(writer)],
          resolver:
              ({
                required key,
                required ctrl,
                required meta,
                required shift,
                required alt,
                required scope,
                required hasToolFocus,
              }) => switch (key) {
                'F2' => const KeyboardCommandDto.copy(),
                'Backspace' => const KeyboardCommandDto.backspace(),
                'PageDown' => const KeyboardCommandDto.page(
                  direction: PageDirectionDto.down,
                ),
                _ => null,
              },
        ),
      );
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.backspace);
      await tester.sendKeyEvent(LogicalKeyboardKey.pageDown);
      expect(writer.writes, isEmpty);
      final container = ProviderScope.containerOf(
        tester.element(find.byType(BoardPage)),
      );
      container
          .read(lastOutcomeProvider.notifier)
          .record(
            (BoardKey.parse('dev'), PinId.parse(pinId)),
            ToolId.parse(pinId),
            const CanonicalToolResult(
              ok: true,
              primaryOutputId: 'result',
              outputs: [
                CanonicalOutputEntry(
                  id: 'result',
                  kind: 'string',
                  value: CanonicalOutputValue.string(value: 'saved result'),
                ),
              ],
            ),
            args: ToolArgs.fromJsonObject(const {'input': 'old input'}),
          );
      await tester.enterText(find.byKey(const Key('field-input')), 'new draft');
      await tester.pump();
      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.keyC);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      expect(writer.writes, isEmpty);
      await tester.sendKeyEvent(LogicalKeyboardKey.f2);
      await tester.pump();
      expect(writer.writes, ['saved result']);
      expect(find.text('Copied previous result'), findsOneWidget);
      expect(find.byType(ExpandedModalPage), findsNothing);

      container
          .read(lastOutcomeProvider.notifier)
          .record(
            (BoardKey.parse('dev'), PinId.parse(pinId)),
            ToolId.parse(pinId),
            const CanonicalToolResult(
              ok: true,
              primaryOutputId: 'result',
              outputs: [
                CanonicalOutputEntry(
                  id: 'result',
                  kind: 'string',
                  value: CanonicalOutputValue.string(value: 'legacy result'),
                ),
              ],
            ),
          );
      await tester.sendKeyEvent(LogicalKeyboardKey.f2);
      await tester.pump();
      expect(writer.writes, ['saved result', 'legacy result']);
      expect(
        find.text('Copied result; its input history is unavailable'),
        findsOneWidget,
      );

      container
          .read(lastOutcomeProvider.notifier)
          .record(
            (BoardKey.parse('dev'), PinId.parse(pinId)),
            ToolId.parse(pinId),
            const CanonicalToolResult(
              ok: false,
              outputs: [],
              error: CanonicalToolError(
                code: 'failed',
                message: '/private/hidden-error.txt',
              ),
            ),
          );
      await tester.sendKeyEvent(LogicalKeyboardKey.f2);
      await tester.pump();
      expect(writer.writes, ['saved result', 'legacy result']);
      expect(
        find.text(
          'No text result to copy. Run the tool or use its file action.',
        ),
        findsWidgets,
      );

      await tester.enterText(find.byKey(const Key('field-input')), ' 0xff ');
      await tester.pump();
      container
          .read(lastOutcomeProvider.notifier)
          .record(
            (BoardKey.parse('dev'), PinId.parse(pinId)),
            ToolId.parse(pinId),
            const CanonicalToolResult(
              ok: true,
              primaryOutputId: 'result',
              outputs: [
                CanonicalOutputEntry(
                  id: 'result',
                  kind: 'string',
                  value: CanonicalOutputValue.string(value: '16'),
                ),
              ],
            ),
            args: ToolArgs.fromJsonObject(const {'input': ' 0xff '}),
          );
      await tester.sendKeyEvent(LogicalKeyboardKey.f2);
      await tester.pump();
      expect(writer.writes, ['saved result', 'legacy result', '16']);
      expect(find.text('Copied'), findsWidgets);
    });

    testWidgets('BoardPage_unpins_the_focused_pin_on_the_TogglePin_command', (
      tester,
    ) async {
      PinKey? observed;
      await tester.pumpWidget(
        boardPageHarness(
          currentBoardKey: 'dev',
          focusedToolId: 'num.hex_to_decimal',
          isPinnedLoader: (boardKey, toolId) => true,
          removePinMutator: (boardKey, pinId) {
            observed = (boardKey, pinId);
          },
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
                if (key == 'p') return const KeyboardCommandDto.togglePin();
                return null;
              },
        ),
      );

      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.keyP);
      await tester.pump();

      expect(observed, (
        BoardKey.parse('dev'),
        PinId.parse('num.hex_to_decimal'),
      ));
    });

    testWidgets('BoardPage_does_not_remove_a_pin_when_none_is_focused', (
      tester,
    ) async {
      var removeCalls = 0;
      await tester.pumpWidget(
        boardPageHarness(
          currentBoardKey: 'dev',
          removePinMutator: (_, _) => removeCalls += 1,
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
                if (key == 'p') return const KeyboardCommandDto.togglePin();
                return null;
              },
        ),
      );

      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.keyP);
      await tester.pump();

      expect(removeCalls, 0);
    });

    testWidgets(
      'BoardPage_routes_the_Reorder_previous_command_to_the_reorder_provider',
      (tester) async {
        PinId? observedTool;
        OrderDirectionDto? observedDirection;
        await tester.pumpWidget(
          boardPageHarness(
            currentBoardKey: 'dev',
            focusedToolId: 'num.hex_to_decimal',
            reorderPin: (toolId, direction) async {
              observedTool = toolId;
              observedDirection = direction;
            },
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
                  if (key == '[') {
                    return const KeyboardCommandDto.reorder(
                      direction: OrderDirectionDto.previous,
                    );
                  }
                  return null;
                },
          ),
        );

        await tester.pumpAndSettle();
        await tester.sendKeyEvent(LogicalKeyboardKey.bracketLeft);
        await tester.pump();

        expect(observedTool, PinId.parse('num.hex_to_decimal'));
        expect(observedDirection, OrderDirectionDto.previous);
      },
    );

    testWidgets('BoardPage_activates_the_focused_pin_on_the_Run_command', (
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
            placements: const [
              PlacementDto(
                toolId: 'num.hex_to_decimal',
                pinId: 'num.hex_to_decimal',
                x: 0,
                y: 0,
                w: 1,
                h: 1,
              ),
            ],
          ),
          pinActivation: ({required toolId, required argsJson}) {
            return PinActivationDto.openModal(toolId: toolId.value);
          },
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
                if (key == 'F1') return const KeyboardCommandDto.run();
                return null;
              },
        ),
      );

      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.f1);
      await tester.pumpAndSettle();

      expect(find.byType(ExpandedModalPage), findsOneWidget);
    });

    testWidgets('BoardPage_can_focus_the_first_pin_on_Move_command_then_Run', (
      tester,
    ) async {
      await tester.pumpWidget(
        boardPageHarness(
          currentBoardKey: 'dev',
          layoutLoader: (query) => LayoutSnapshotDto(
            boardKey: query.boardKey.value,
            boardCols: 6,
            placements: const <PlacementDto>[
              PlacementDto(
                toolId: 'num.hex_to_decimal',
                pinId: 'num.hex_to_decimal',
                x: 0,
                y: 0,
                w: 1,
                h: 1,
              ),
            ],
          ),
          tools: [
            fixtureToolDto(id: 'num.hex_to_decimal', label: 'Hex to Dec'),
          ],
          pinActivation: ({required toolId, required argsJson}) {
            return PinActivationDto.openModal(toolId: toolId.value);
          },
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
                if (key == 'ArrowRight') {
                  return const KeyboardCommandDto.move(
                    direction: DirectionDto.right,
                  );
                }
                if (key == 'F1') return const KeyboardCommandDto.run();
                return null;
              },
        ),
      );

      await tester.pumpAndSettle();
      expect(find.byType(ExpandedModalPage), findsNothing);

      await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.f1);
      await tester.pumpAndSettle();

      expect(find.byType(ExpandedModalPage), findsOneWidget);
    });

    testWidgets('BoardPage_focuses_the_spatially_right_pin_on_arrow_keys', (
      tester,
    ) async {
      ToolId? activated;
      await tester.pumpWidget(
        boardPageHarness(
          currentBoardKey: 'dev',
          focusedToolId: 'fixture.left',
          layoutLoader: (query) => LayoutSnapshotDto(
            boardKey: query.boardKey.value,
            boardCols: 12,
            placements: const <PlacementDto>[
              PlacementDto(
                toolId: 'fixture.left',
                pinId: 'fixture.left',
                x: 0,
                y: 0,
                w: 1,
                h: 1,
              ),
              PlacementDto(
                toolId: 'fixture.right',
                pinId: 'fixture.right',
                x: 4,
                y: 0,
                w: 1,
                h: 1,
              ),
              PlacementDto(
                toolId: 'fixture.down',
                pinId: 'fixture.down',
                x: 0,
                y: 1,
                w: 1,
                h: 1,
              ),
            ],
          ),
          tools: [
            fixtureToolDto(id: 'fixture.left', label: 'Left'),
            fixtureToolDto(id: 'fixture.right', label: 'Right'),
            fixtureToolDto(id: 'fixture.down', label: 'Down'),
          ],
          pinActivation: ({required toolId, required argsJson}) {
            activated = toolId;
            return PinActivationDto.openModal(toolId: toolId.value);
          },
          resolver:
              ({
                required String key,
                required bool ctrl,
                required bool meta,
                required bool shift,
                required bool alt,
                required KeyboardScopeDto scope,
                required bool hasToolFocus,
              }) {
                expect(scope, KeyboardScopeDto.board);
                if (key == 'ArrowRight') {
                  return const KeyboardCommandDto.move(
                    direction: DirectionDto.right,
                  );
                }
                if (key == 'F1') return const KeyboardCommandDto.run();
                return null;
              },
        ),
      );

      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.f1);
      await tester.pumpAndSettle();

      expect(activated, ToolId.parse('fixture.right'));
    });

    testWidgets('BoardPage_moves_GUI_focus_with_Tab_without_pin_focus', (
      tester,
    ) async {
      var resolverCalls = 0;
      await tester.pumpWidget(
        boardPageHarness(
          resolver:
              ({
                required String key,
                required bool ctrl,
                required bool meta,
                required bool shift,
                required bool alt,
                required KeyboardScopeDto scope,
                required bool hasToolFocus,
              }) {
                resolverCalls += 1;
                return const KeyboardCommandDto.focusNext();
              },
        ),
      );

      await tester.pumpAndSettle();
      final boardFocus = tester.widget<Focus>(boardPageFocusFinder()).focusNode;
      expect(FocusManager.instance.primaryFocus, boardFocus);
      final result = dispatchBoardPageKey(
        tester,
        logicalKey: LogicalKeyboardKey.tab,
        physicalKey: PhysicalKeyboardKey.tab,
      );
      await tester.pump();

      expect(result, KeyEventResult.handled);
      expect(resolverCalls, 0);
      expect(FocusManager.instance.primaryFocus, isNot(boardFocus));
    });

    testWidgets('BoardPage_uses_Tab_as_a_board_focus_command_with_pin_focus', (
      tester,
    ) async {
      var resolverCalls = 0;
      await tester.pumpWidget(
        boardPageHarness(
          focusedToolId: 'num.hex_to_decimal',
          resolver:
              ({
                required String key,
                required bool ctrl,
                required bool meta,
                required bool shift,
                required bool alt,
                required KeyboardScopeDto scope,
                required bool hasToolFocus,
              }) {
                resolverCalls += 1;
                expect(key, 'Tab');
                expect(scope, KeyboardScopeDto.board);
                expect(hasToolFocus, isTrue);
                return const KeyboardCommandDto.focusNext();
              },
        ),
      );

      await tester.pumpAndSettle();
      final result = dispatchBoardPageKey(
        tester,
        logicalKey: LogicalKeyboardKey.tab,
        physicalKey: PhysicalKeyboardKey.tab,
      );

      expect(result, KeyEventResult.handled);
      expect(resolverCalls, 1);
    });

    testWidgets('the_right_click_pin_menu_has_a_color_edit_entry', (
      tester,
    ) async {
      await pumpBoardPageWithPinnedTool(tester);

      await openPinContextMenu(tester);

      expect(find.byKey(pinContextEditColorKey), findsOneWidget);
      expect(find.text(editColorMenuText), findsOneWidget);
      expect(find.text(unpinMenuText), findsOneWidget);
      expect(
        tester.getTopLeft(find.text(editColorMenuText)).dy,
        lessThan(tester.getTopLeft(find.text(unpinMenuText)).dy),
      );
    });

    testWidgets('the_right_click_color_edit_opens_a_dialog', (tester) async {
      await pumpBoardPageWithPinnedTool(tester);

      await openPinContextMenu(tester);
      await tester.tap(find.byKey(pinContextEditColorKey));
      await tester.pumpAndSettle();

      expect(find.byKey(pinColorDialogKey), findsOneWidget);
    });

    testWidgets('pressing_c_opens_the_focused_pin_color_dialog', (
      tester,
    ) async {
      await tester.pumpWidget(
        boardPageHarness(
          currentBoardKey: 'dev',
          focusedToolId: 'num.hex_to_decimal',
          layoutLoader: (_) => LayoutSnapshotDto(
            boardKey: 'dev',
            boardCols: 6,
            placements: const [
              PlacementDto(
                toolId: 'num.hex_to_decimal',
                pinId: 'num.hex_to_decimal',
                x: 0,
                y: 0,
                w: 1,
                h: 1,
              ),
            ],
          ),
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
                // Modeless: EditPinColor gates on a focused pin only — no
                // edit-mode toggle is required.
                if (key == 'c' && hasToolFocus) {
                  return const KeyboardCommandDto.editPinColor();
                }
                return null;
              },
        ),
      );

      // Press 'c' with a focused pin — should open PinColorDialog directly.
      expect(
        dispatchBoardPageKey(
          tester,
          logicalKey: LogicalKeyboardKey.keyC,
          physicalKey: PhysicalKeyboardKey.keyC,
          character: 'c',
        ),
        KeyEventResult.handled,
      );
      await tester.pumpAndSettle();

      expect(find.byKey(const Key('pin-color-dialog')), findsOneWidget);
    });

    testWidgets('the_color_command_does_nothing_without_a_focused_pin', (
      tester,
    ) async {
      await tester.pumpWidget(
        boardPageHarness(
          currentBoardKey: 'dev',
          focusedToolId: null,
          layoutLoader: (_) => LayoutSnapshotDto(
            boardKey: 'dev',
            boardCols: 6,
            placements: const <PlacementDto>[],
          ),
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
                // Resolver returns editPinColor, but since there's no focused pin
                // in BoardPage, _dispatchCommand will return false (ignored).
                // In real usage, the Rust resolver only returns EditPinColor when
                // hasToolFocus is true.
                if (hasToolFocus && key == 'c') {
                  return const KeyboardCommandDto.editPinColor();
                }
                return null;
              },
        ),
      );

      // 'c' without edit mode → resolver returns null → no command → ignored
      expect(
        dispatchBoardPageKey(
          tester,
          logicalKey: LogicalKeyboardKey.keyC,
          physicalKey: PhysicalKeyboardKey.keyC,
          character: 'c',
        ),
        KeyEventResult.ignored,
      );
      await tester.pumpAndSettle();

      expect(find.byKey(const Key('pin-color-dialog')), findsNothing);
    });
  });
}
