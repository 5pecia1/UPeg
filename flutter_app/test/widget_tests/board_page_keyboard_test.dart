/// Widget tests for the BoardPage keyboard wiring (G5).
///
/// The keyboard FRB itself is exercised in upeg-frb Rust unit tests
/// (`keyboard_command_for_가_*`). Here we verify that the Dart side
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
    testWidgets('BoardPage는_F1_누르면_Run_command를_dispatch한다', (tester) async {
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

    testWidgets('BoardPage는_Cmd_K_누르면_Search_command를_dispatch한다', (
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

    testWidgets('BoardPage는_Esc_누르면_Close_command를_dispatch한다', (tester) async {
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

    testWidgets('BoardPage는_Close_command로_popup_mode로_돌아간다', (tester) async {
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

    testWidgets('q_누르면_quit_confirm_dialog가_열리고_즉시_종료하지_않는다', (tester) async {
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
    });

    testWidgets('quit_confirm에서_F1으로_확인하면_shutdown_경로가_한번_호출된다', (
      tester,
    ) async {
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
    });

    testWidgets('quit_confirm에서_Enter로_확인해도_shutdown_경로가_한번_호출된다', (
      tester,
    ) async {
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
    });

    testWidgets('quit_confirm에서_Esc는_취소하고_종료하지_않는다', (tester) async {
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

    testWidgets('quit_confirm이_열린_동안_q를_다시_눌러도_dialog는_하나만_뜬다', (tester) async {
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
    });

    testWidgets('move_mode에서_Enter는_moving_scope로_commit한다', (tester) async {
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

    testWidgets('move_mode에서_Escape는_popup_close가_아니라_cancel한다', (
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

    testWidgets('BoardPage는_NewBoard_command로_create_board_dialog를_연다', (
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
                if (key == 'n') return const KeyboardCommandDto.newBoard();
                return null;
              },
        ),
      );

      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.keyN);
      await tester.pumpAndSettle();

      expect(find.byKey(const Key('create-board-dialog')), findsOneWidget);
    });

    testWidgets('모드리스에서_e는_바인딩없는_자유키고_n은_토글없이_동작한다', (tester) async {
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
    });

    testWidgets('BoardPage는_RenameBoard_command로_현재_board_rename_dialog를_연다', (
      tester,
    ) async {
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

      expect(find.byKey(const Key('rename-board-dialog-dev')), findsOneWidget);
    });

    testWidgets('BoardPage는_DeleteBoard_command로_현재_board_delete_dialog를_연다', (
      tester,
    ) async {
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

      expect(find.byKey(const Key('delete-board-dialog-dev')), findsOneWidget);
    });

    testWidgets('BoardPage는_OpenToolPicker_command로_palette를_연다', (
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

    testWidgets('BoardPage는_물음표로_cheatsheet_overlay를_열고_Esc로_닫는다', (
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

      // Shift+/ 는 character '?'로 도착한다 — label 파서가 그대로 넘긴다.
      dispatchBoardPageKey(
        tester,
        logicalKey: LogicalKeyboardKey.slash,
        physicalKey: PhysicalKeyboardKey.slash,
        character: '?',
      );
      await tester.pumpAndSettle();
      expect(find.byKey(CheatsheetOverlay.overlayKey), findsOneWidget);

      // 오버레이는 공유 confirm scope를 재사용해 Esc로 닫힌다.
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(find.byKey(CheatsheetOverlay.overlayKey), findsNothing);
    });

    testWidgets('BoardPage는_ClearBoardFilter_command로_첫번째_board를_선택한다', (
      tester,
    ) async {
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
    });

    testWidgets('BoardPage는_CycleBoardFilter_command로_다음_board를_선택한다', (
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
    });

    testWidgets('BoardPage는_SwitchBoard_command로_slot_board를_선택한다', (
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

    testWidgets('BoardPage는_CycleTagFilter_command로_다음_tag를_선택한다', (
      tester,
    ) async {
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
      expect(tester.widget<Text>(find.byKey(selectedTagProbeKey)).data, 'all');

      await tester.sendKeyEvent(LogicalKeyboardKey.keyT);
      await tester.pumpAndSettle();

      expect(
        tester.widget<Text>(find.byKey(selectedTagProbeKey)).data,
        'convert',
      );
    });
  });
}
