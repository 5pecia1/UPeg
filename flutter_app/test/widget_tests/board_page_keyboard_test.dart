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
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/pages/board_page.dart';
import 'package:upeg/src/rust/api/keyboard.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/state/window_mode_provider.dart';
import 'package:upeg/src/widgets/cheatsheet_overlay.dart';
import 'package:upeg/src/widgets/palette_overlay.dart';

import '../test_helpers/board_page_harness.dart';

void main() {
  group('BoardPage keyboard wiring', () {
    testWidgets('BoardPage_dispatches_the_Run_command_on_F1', (tester) async {
      KeyboardCommandDto? received;
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            keyboardCommandResolverProvider.overrideWith(
              (_) =>
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
          ],
          child: MaterialApp(
            home: KeyboardHarness(onCommand: (c) => received = c),
          ),
        ),
      );

      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.f1);
      await tester.pump();

      expect(received, const KeyboardCommandDto.run());
    });

    testWidgets('BoardPage_dispatches_the_Search_command_on_Cmd_K', (
      tester,
    ) async {
      KeyboardCommandDto? received;
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            keyboardCommandResolverProvider.overrideWith(
              (_) =>
                  ({
                    required String key,
                    required bool ctrl,
                    required bool meta,
                    required bool shift,
                    required bool alt,
                    required bool hasToolFocus,
                    required KeyboardScopeDto scope,
                  }) {
                    if ((ctrl || meta) && key == 'k') {
                      return const KeyboardCommandDto.search();
                    }
                    return null;
                  },
            ),
          ],
          child: MaterialApp(
            home: KeyboardHarness(onCommand: (c) => received = c),
          ),
        ),
      );

      await tester.pump();
      // Hold Ctrl, press K, release.
      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.keyK);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      await tester.pump();

      expect(received, const KeyboardCommandDto.search());
    });

    testWidgets('BoardPage_dispatches_the_Close_command_on_Esc', (
      tester,
    ) async {
      KeyboardCommandDto? received;
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            keyboardCommandResolverProvider.overrideWith(
              (_) =>
                  ({
                    required String key,
                    required bool ctrl,
                    required bool meta,
                    required bool shift,
                    required bool alt,
                    required bool hasToolFocus,
                    required KeyboardScopeDto scope,
                  }) {
                    if (key == 'Escape') {
                      return const KeyboardCommandDto.close();
                    }
                    return null;
                  },
            ),
          ],
          child: MaterialApp(
            home: KeyboardHarness(onCommand: (c) => received = c),
          ),
        ),
      );

      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump();

      expect(received, const KeyboardCommandDto.close());
    });

    testWidgets('BoardPage_returns_to_popup_mode_on_the_Close_command', (
      tester,
    ) async {
      WindowMode? observed;
      await tester.pumpWidget(
        boardPageHarness(
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
                if (key == 'Escape') return const KeyboardCommandDto.close();
                return null;
              },
          windowModeNotifier: () => RecordingWindowModeNotifier(
            initial: WindowMode.full,
            onChange: (mode) => observed = mode,
          ),
        ),
      );

      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump();

      expect(observed, WindowMode.popup);
    });

    testWidgets(
      'pressing_q_opens_the_quit_confirm_dialog_without_quitting_immediately',
      (tester) async {
        var quitCalls = 0;
        await tester.pumpWidget(
          boardPageHarness(
            quitApp: () async {
              quitCalls += 1;
            },
            resolver: quitFlowResolver,
          ),
        );

        await tester.pump();
        await tester.sendKeyEvent(LogicalKeyboardKey.keyQ);
        await tester.pumpAndSettle();

        expect(find.byKey(QuitConfirmDialog.dialogKey), findsOneWidget);
        expect(quitCalls, 0);
      },
    );

    testWidgets(
      'confirming_quit_confirm_with_F1_invokes_the_shutdown_path_once',
      (tester) async {
        var quitCalls = 0;
        await tester.pumpWidget(
          boardPageHarness(
            quitApp: () async {
              quitCalls += 1;
            },
            resolver: quitFlowResolver,
          ),
        );

        await tester.pump();
        await tester.sendKeyEvent(LogicalKeyboardKey.keyQ);
        await tester.pumpAndSettle();
        await tester.sendKeyEvent(LogicalKeyboardKey.f1);
        await tester.pumpAndSettle();

        expect(find.byKey(QuitConfirmDialog.dialogKey), findsNothing);
        expect(quitCalls, 1);
      },
    );

    testWidgets(
      'confirming_quit_confirm_with_Enter_also_invokes_the_shutdown_path_once',
      (tester) async {
        var quitCalls = 0;
        await tester.pumpWidget(
          boardPageHarness(
            quitApp: () async {
              quitCalls += 1;
            },
            resolver: quitFlowResolver,
          ),
        );

        await tester.pump();
        await tester.sendKeyEvent(LogicalKeyboardKey.keyQ);
        await tester.pumpAndSettle();
        await tester.sendKeyEvent(LogicalKeyboardKey.enter);
        await tester.pumpAndSettle();

        expect(find.byKey(QuitConfirmDialog.dialogKey), findsNothing);
        expect(quitCalls, 1);
      },
    );

    testWidgets('Esc_in_quit_confirm_cancels_without_quitting', (tester) async {
      var quitCalls = 0;
      await tester.pumpWidget(
        boardPageHarness(
          quitApp: () async {
            quitCalls += 1;
          },
          resolver: quitFlowResolver,
        ),
      );

      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.keyQ);
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();

      expect(find.byKey(QuitConfirmDialog.dialogKey), findsNothing);
      expect(quitCalls, 0);
    });

    testWidgets(
      'pressing_q_again_while_quit_confirm_is_open_keeps_a_single_dialog',
      (tester) async {
        var quitCalls = 0;
        await tester.pumpWidget(
          boardPageHarness(
            quitApp: () async {
              quitCalls += 1;
            },
            resolver: quitFlowResolver,
          ),
        );

        await tester.pump();
        await tester.sendKeyEvent(LogicalKeyboardKey.keyQ);
        await tester.pumpAndSettle();
        // In the confirm scope `q` resolves to Cancel (shared confirm
        // resolver) — never to a second Quit → no stacked dialog.
        await tester.sendKeyEvent(LogicalKeyboardKey.keyQ);
        await tester.pumpAndSettle();

        expect(find.byKey(QuitConfirmDialog.dialogKey), findsNothing);
        expect(quitCalls, 0);
      },
    );

    testWidgets('Enter_in_move_mode_commits_in_the_moving_scope', (
      tester,
    ) async {
      KeyboardScopeDto? enterScope;
      ToolId? observedTool;
      int? observedX;
      int? observedY;

      await tester.pumpWidget(
        boardPageHarness(
          focusedToolId: 'num.hex_to_decimal',
          layoutLoader: (query) => LayoutSnapshotDto(
            boardKey: query.boardKey.value,
            boardCols: 6,
            placements: const [
              PlacementDto(
                toolId: 'num.hex_to_decimal',
                x: 0,
                y: 0,
                w: 1,
                h: 1,
              ),
            ],
          ),
          movePinCommit: (toolId, anchorX, anchorY) async {
            observedTool = toolId;
            observedX = anchorX;
            observedY = anchorY;
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
                if (key == 'm') return const KeyboardCommandDto.startMove();
                if (key == 'Enter') {
                  enterScope = scope;
                  return scope == KeyboardScopeDto.moving
                      ? const KeyboardCommandDto.commit()
                      : null;
                }
                return null;
              },
        ),
      );

      await tester.pump();
      expect(
        dispatchBoardPageKey(
          tester,
          logicalKey: LogicalKeyboardKey.keyM,
          physicalKey: PhysicalKeyboardKey.keyM,
          character: 'm',
        ),
        KeyEventResult.handled,
      );
      await tester.pump();
      expect(
        dispatchBoardPageKey(
          tester,
          logicalKey: LogicalKeyboardKey.enter,
          physicalKey: PhysicalKeyboardKey.enter,
        ),
        KeyEventResult.handled,
      );
      await tester.pump();
      await tester.pump();

      expect(enterScope, KeyboardScopeDto.moving);
      expect(observedTool, ToolId.parse('num.hex_to_decimal'));
      expect(observedX, 0);
      expect(observedY, 0);
    });

    testWidgets('Escape_in_move_mode_cancels_instead_of_closing_the_popup', (
      tester,
    ) async {
      KeyboardScopeDto? escapeScope;
      WindowMode? observedMode;

      await tester.pumpWidget(
        boardPageHarness(
          focusedToolId: 'num.hex_to_decimal',
          layoutLoader: (query) => LayoutSnapshotDto(
            boardKey: query.boardKey.value,
            boardCols: 6,
            placements: const [
              PlacementDto(
                toolId: 'num.hex_to_decimal',
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
                if (key == 'm') return const KeyboardCommandDto.startMove();
                if (key == 'Escape') {
                  escapeScope = scope;
                  return scope == KeyboardScopeDto.moving
                      ? const KeyboardCommandDto.cancel()
                      : const KeyboardCommandDto.close();
                }
                return null;
              },
          windowModeNotifier: () => RecordingWindowModeNotifier(
            initial: WindowMode.full,
            onChange: (mode) => observedMode = mode,
          ),
        ),
      );

      await tester.pump();
      expect(
        dispatchBoardPageKey(
          tester,
          logicalKey: LogicalKeyboardKey.keyM,
          physicalKey: PhysicalKeyboardKey.keyM,
          character: 'm',
        ),
        KeyEventResult.handled,
      );
      await tester.pump();
      expect(
        dispatchBoardPageKey(
          tester,
          logicalKey: LogicalKeyboardKey.escape,
          physicalKey: PhysicalKeyboardKey.escape,
        ),
        KeyEventResult.handled,
      );
      await tester.pump();

      expect(escapeScope, KeyboardScopeDto.moving);
      expect(observedMode, isNull);
    });

    testWidgets(
      'BoardPage_opens_the_create_board_dialog_on_the_NewBoard_command',
      (tester) async {
        await tester.pumpWidget(
          boardPageHarness(
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
                  if (key == 'n') return const KeyboardCommandDto.newBoard();
                  return null;
                },
          ),
        );

        await tester.pump();
        await tester.sendKeyEvent(LogicalKeyboardKey.keyN);
        await tester.pumpAndSettle();

        expect(find.byKey(const Key('create-board-dialog')), findsOneWidget);
      },
    );

    testWidgets(
      'in_modeless_e_is_an_unbound_free_key_and_n_acts_without_a_toggle',
      (tester) async {
        var newBoardRequested = false;
        await tester.pumpWidget(
          boardPageHarness(
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
                  // Modeless: 'e' no longer toggles edit mode — it resolves to
                  // nothing. Board-management keys stay live without any toggle.
                  if (key == 'n') {
                    newBoardRequested = true;
                    return const KeyboardCommandDto.newBoard();
                  }
                  return null;
                },
          ),
        );
        await tester.pump();

        // 'e' is a free, unbound key → the board ignores it.
        expect(
          dispatchBoardPageKey(
            tester,
            logicalKey: LogicalKeyboardKey.keyE,
            physicalKey: PhysicalKeyboardKey.keyE,
            character: 'e',
          ),
          KeyEventResult.ignored,
        );

        // 'n' still opens the create-board dialog with no edit toggle first.
        await tester.sendKeyEvent(LogicalKeyboardKey.keyN);
        await tester.pumpAndSettle();
        expect(newBoardRequested, isTrue);
        expect(find.byKey(const Key('create-board-dialog')), findsOneWidget);
      },
    );

    testWidgets(
      'BoardPage_opens_the_rename_dialog_for_the_current_board_on_the_RenameBoard_command',
      (tester) async {
        await tester.pumpWidget(
          boardPageHarness(
            currentBoardKey: 'dev',
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
                  if (key == 'r') return const KeyboardCommandDto.renameBoard();
                  return null;
                },
          ),
        );

        await tester.pump();
        await tester.sendKeyEvent(LogicalKeyboardKey.keyR);
        await tester.pumpAndSettle();

        expect(
          find.byKey(const Key('rename-board-dialog-dev')),
          findsOneWidget,
        );
      },
    );

    testWidgets(
      'BoardPage_opens_the_delete_dialog_for_the_current_board_on_the_DeleteBoard_command',
      (tester) async {
        await tester.pumpWidget(
          boardPageHarness(
            currentBoardKey: 'dev',
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
                  if (key == 'd') return const KeyboardCommandDto.deleteBoard();
                  return null;
                },
          ),
        );

        await tester.pump();
        await tester.sendKeyEvent(LogicalKeyboardKey.keyD);
        await tester.pumpAndSettle();

        expect(
          find.byKey(const Key('delete-board-dialog-dev')),
          findsOneWidget,
        );
      },
    );

    testWidgets('BoardPage_opens_the_palette_on_the_OpenToolPicker_command', (
      tester,
    ) async {
      await tester.pumpWidget(
        boardPageHarness(
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
                if (key == 'a') {
                  return const KeyboardCommandDto.openToolPicker();
                }
                return null;
              },
        ),
      );

      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
      await tester.pumpAndSettle();

      expect(find.byType(PaletteOverlay), findsOneWidget);
    });

    testWidgets(
      'BoardPage_opens_the_cheatsheet_overlay_on_question_mark_and_closes_it_on_Esc',
      (tester) async {
        await tester.pumpWidget(
          boardPageHarness(
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
                  if (scope == KeyboardScopeDto.confirmDelete &&
                      key == 'Escape') {
                    return const KeyboardCommandDto.cancel();
                  }
                  if (scope == KeyboardScopeDto.board && key == '?') {
                    return const KeyboardCommandDto.showCheatsheet();
                  }
                  return null;
                },
          ),
        );
        await tester.pump();

        // Shift+/ arrives as character '?' — the label parser passes it through.
        dispatchBoardPageKey(
          tester,
          logicalKey: LogicalKeyboardKey.slash,
          physicalKey: PhysicalKeyboardKey.slash,
          character: '?',
        );
        await tester.pumpAndSettle();
        expect(find.byKey(CheatsheetOverlay.overlayKey), findsOneWidget);

        // The overlay reuses the shared confirm scope and closes on Esc.
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await tester.pumpAndSettle();
        expect(find.byKey(CheatsheetOverlay.overlayKey), findsNothing);
      },
    );

    testWidgets(
      'BoardPage_selects_the_first_board_on_the_ClearBoardFilter_command',
      (tester) async {
        final loadedBoards = <String>[];
        await tester.pumpWidget(
          boardPageHarness(
            boards: const <BoardDto>[
              BoardDto(key: 'dev', title: 'Dev'),
              BoardDto(key: 'media', title: 'Media'),
            ],
            currentBoardKey: 'media',
            layoutLoader: (query) {
              loadedBoards.add(query.boardKey.value);
              return LayoutSnapshotDto(
                boardKey: query.boardKey.value,
                boardCols: 6,
                placements: const <PlacementDto>[],
              );
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
                  if (key == '0') {
                    return const KeyboardCommandDto.clearBoardFilter();
                  }
                  return null;
                },
          ),
        );

        await tester.pumpAndSettle();
        expect(loadedBoards, contains('media'));

        await tester.sendKeyEvent(LogicalKeyboardKey.digit0);
        await tester.pumpAndSettle();

        expect(loadedBoards, contains('dev'));
      },
    );

    testWidgets(
      'BoardPage_selects_the_next_board_on_the_CycleBoardFilter_command',
      (tester) async {
        final loadedBoards = <String>[];
        await tester.pumpWidget(
          boardPageHarness(
            boards: const <BoardDto>[
              BoardDto(key: 'dev', title: 'Dev'),
              BoardDto(key: 'media', title: 'Media'),
              BoardDto(key: 'ops', title: 'Ops'),
            ],
            currentBoardKey: 'media',
            layoutLoader: (query) {
              loadedBoards.add(query.boardKey.value);
              return LayoutSnapshotDto(
                boardKey: query.boardKey.value,
                boardCols: 6,
                placements: const <PlacementDto>[],
              );
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
                  if (key == 'b') {
                    return const KeyboardCommandDto.cycleBoardFilter();
                  }
                  return null;
                },
          ),
        );

        await tester.pumpAndSettle();
        expect(loadedBoards, contains('media'));

        await tester.sendKeyEvent(LogicalKeyboardKey.keyB);
        await tester.pumpAndSettle();

        expect(loadedBoards, contains('ops'));
      },
    );

    testWidgets('BoardPage_selects_the_slot_board_on_the_SwitchBoard_command', (
      tester,
    ) async {
      final loadedBoards = <String>[];
      await tester.pumpWidget(
        boardPageHarness(
          boards: const <BoardDto>[
            BoardDto(key: 'dev', title: 'Dev'),
            BoardDto(key: 'media', title: 'Media'),
            BoardDto(key: 'ops', title: 'Ops'),
          ],
          currentBoardKey: 'dev',
          layoutLoader: (query) {
            loadedBoards.add(query.boardKey.value);
            return LayoutSnapshotDto(
              boardKey: query.boardKey.value,
              boardCols: 6,
              placements: const <PlacementDto>[],
            );
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
                if (key == '2') {
                  return const KeyboardCommandDto.switchBoard(slot: 2);
                }
                return null;
              },
        ),
      );

      await tester.pumpAndSettle();
      expect(loadedBoards, contains('dev'));

      await tester.sendKeyEvent(LogicalKeyboardKey.digit2);
      await tester.pumpAndSettle();

      expect(loadedBoards, contains('media'));
    });

    testWidgets(
      'BoardPage_selects_the_next_tag_on_the_CycleTagFilter_command',
      (tester) async {
        await tester.pumpWidget(
          boardPageHarness(
            tagOptions: const <String>['all', 'convert', 'uuid'],
            includeSelectedTagProbe: true,
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
                  if (key == 't') {
                    return const KeyboardCommandDto.cycleTagFilter();
                  }
                  return null;
                },
          ),
        );

        await tester.pumpAndSettle();
        expect(
          tester.widget<Text>(find.byKey(selectedTagProbeKey)).data,
          'all',
        );

        await tester.sendKeyEvent(LogicalKeyboardKey.keyT);
        await tester.pumpAndSettle();

        expect(
          tester.widget<Text>(find.byKey(selectedTagProbeKey)).data,
          'convert',
        );
      },
    );
  });
}
