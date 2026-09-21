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

import 'package:upeg/src/pages/board_page.dart';
import 'package:upeg/src/state/window_mode_provider.dart';
import 'package:upeg/src/widgets/controlled_embed/tile.dart'
    show ControlledEmbedTile;
import 'package:upeg/src/widgets/palette_overlay.dart';
import 'package:upeg/src/widgets/pin.dart';

import '../test_helpers/board_page_harness.dart';

void main() {
  group('inline embed keyboard coexistence', () {
    testWidgets(
      'while_typing_in_an_embed_form_a_plain_key_is_text_not_a_board_command',
      (tester) async {
        final resolverCalls = <String>[];
        await pumpBoardWithControlledEmbed(
          tester,
          onResolverKey: resolverCalls.add,
        );

        // The inline embed body is marked so the board can detect it.
        expect(
          find.descendant(
            of: find.byType(Pin),
            matching: find.byType(EmbedBodyFocusScope),
          ),
          findsOneWidget,
        );
        expect(find.byType(ControlledEmbedTile), findsOneWidget);

        // Focus the embed's form field: primary focus is now editable text.
        focusEmbedField(tester);
        await tester.pump();
        expect(primaryFocusIsEditableText(), isTrue);
        expect(primaryFocusIsInsideEmbedBody(), isTrue);

        for (final probe in plainKeyProbes) {
          final result = dispatchBoardPageKey(
            tester,
            logicalKey: probe.logical,
            physicalKey: probe.physical,
            character: probe.character,
          );
          expect(
            result,
            KeyEventResult.ignored,
            reason:
                "plain '${probe.character}' must reach the field, not a "
                'board command',
          );
        }
        await tester.pump();

        // The guard short-circuits before resolution, so the board resolver
        // was never consulted for the plain keys.
        expect(resolverCalls, isEmpty);
        // No board side effects fired: an empty resolverCalls already
        // proves no command (togglePin / switchBoard / startMove …) was
        // produced. Palette stayed closed and focus stayed in the field.
        expect(find.byType(PaletteOverlay), findsNothing);
        expect(primaryFocusIsEditableText(), isTrue);
      },
    );

    testWidgets('cmd_k_still_opens_the_palette_while_typing_in_an_embed_form', (
      tester,
    ) async {
      final resolverCalls = <String>[];
      await pumpBoardWithControlledEmbed(
        tester,
        onResolverKey: resolverCalls.add,
      );

      focusEmbedField(tester);
      await tester.pump();
      expect(primaryFocusIsInsideEmbedBody(), isTrue);

      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      final result = dispatchBoardPageKey(
        tester,
        logicalKey: LogicalKeyboardKey.keyK,
        physicalKey: PhysicalKeyboardKey.keyK,
        character: 'k',
      );
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      await tester.pumpAndSettle();

      expect(result, KeyEventResult.handled);
      expect(resolverCalls, contains('k'));
      expect(find.byType(PaletteOverlay), findsOneWidget);
    });

    testWidgets(
      'when_the_embed_webview_body_holds_focus_a_plain_key_is_text_and_cmd_k_opens_the_palette',
      (tester) async {
        // Simulate the webview platform view holding focus: a non-editable
        // focus node inside the embed-body marker scope. primary focus is
        // NOT an EditableText, yet the board must still yield plain keys.
        final probeNode = FocusNode(debugLabel: 'embed-webview-probe');
        addTearDown(probeNode.dispose);
        final resolverCalls = <String>[];

        await tester.pumpWidget(
          boardPageHarness(
            resolver: embedProbeResolver(resolverCalls.add),
            extraOverlays: [
              Positioned(
                left: 0,
                top: 0,
                child: EmbedBodyFocusScope(
                  child: Focus(
                    focusNode: probeNode,
                    child: const SizedBox(width: 10, height: 10),
                  ),
                ),
              ),
            ],
          ),
        );
        await tester.pumpAndSettle();

        probeNode.requestFocus();
        await tester.pump();
        expect(primaryFocusIsInsideEmbedBody(), isTrue);
        expect(primaryFocusIsEditableText(), isFalse);

        for (final probe in plainKeyProbes) {
          final result = dispatchBoardPageKey(
            tester,
            logicalKey: probe.logical,
            physicalKey: probe.physical,
            character: probe.character,
          );
          expect(result, KeyEventResult.ignored);
        }
        expect(resolverCalls, isEmpty);

        await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
        final result = dispatchBoardPageKey(
          tester,
          logicalKey: LogicalKeyboardKey.keyK,
          physicalKey: PhysicalKeyboardKey.keyK,
          character: 'k',
        );
        await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
        await tester.pumpAndSettle();

        expect(result, KeyEventResult.handled);
        expect(find.byType(PaletteOverlay), findsOneWidget);
      },
    );

    testWidgets('esc_in_an_embed_form_returns_focus_to_the_board', (
      tester,
    ) async {
      WindowMode? observedMode;
      final resolverCalls = <String>[];
      await pumpBoardWithControlledEmbed(
        tester,
        onResolverKey: resolverCalls.add,
        windowModeNotifier: () => RecordingWindowModeNotifier(
          initial: WindowMode.full,
          onChange: (mode) => observedMode = mode,
        ),
      );

      focusEmbedField(tester);
      await tester.pump();
      expect(primaryFocusIsInsideEmbedBody(), isTrue);

      final boardFocus = tester.widget<Focus>(boardPageFocusFinder()).focusNode;

      // First Esc: return focus to the board root, do NOT close.
      final first = dispatchBoardPageKey(
        tester,
        logicalKey: LogicalKeyboardKey.escape,
        physicalKey: PhysicalKeyboardKey.escape,
      );
      await tester.pump();
      expect(first, KeyEventResult.handled);
      expect(observedMode, isNull);
      expect(FocusManager.instance.primaryFocus, boardFocus);
      expect(primaryFocusIsInsideEmbedBody(), isFalse);

      // Second Esc: normal board Esc → collapse to popup.
      final second = dispatchBoardPageKey(
        tester,
        logicalKey: LogicalKeyboardKey.escape,
        physicalKey: PhysicalKeyboardKey.escape,
      );
      await tester.pump();
      expect(second, KeyEventResult.handled);
      expect(observedMode, WindowMode.popup);
    });

    testWidgets(
      'arrow_keys_move_the_text_caret_in_an_embed_form_instead_of_triggering_board_commands',
      (tester) async {
        // Issue 6 gap 1: while an inline embed field is focused, navigation
        // keys (arrows / Home / End / Page / Backspace) must reach the field
        // as caret motion, not resolve into board Move/Page/Home/End. If the
        // guard yields, the board resolver is never consulted for them.
        final resolverCalls = <String>[];
        await pumpBoardWithControlledEmbed(
          tester,
          onResolverKey: resolverCalls.add,
        );

        focusEmbedField(tester);
        await tester.pump();
        expect(primaryFocusIsEditableText(), isTrue);

        const navKeys =
            <({LogicalKeyboardKey logical, PhysicalKeyboardKey physical})>[
              (
                logical: LogicalKeyboardKey.arrowLeft,
                physical: PhysicalKeyboardKey.arrowLeft,
              ),
              (
                logical: LogicalKeyboardKey.arrowRight,
                physical: PhysicalKeyboardKey.arrowRight,
              ),
              (
                logical: LogicalKeyboardKey.arrowUp,
                physical: PhysicalKeyboardKey.arrowUp,
              ),
              (
                logical: LogicalKeyboardKey.arrowDown,
                physical: PhysicalKeyboardKey.arrowDown,
              ),
              (
                logical: LogicalKeyboardKey.home,
                physical: PhysicalKeyboardKey.home,
              ),
              (
                logical: LogicalKeyboardKey.end,
                physical: PhysicalKeyboardKey.end,
              ),
              (
                logical: LogicalKeyboardKey.pageUp,
                physical: PhysicalKeyboardKey.pageUp,
              ),
              (
                logical: LogicalKeyboardKey.pageDown,
                physical: PhysicalKeyboardKey.pageDown,
              ),
              (
                logical: LogicalKeyboardKey.backspace,
                physical: PhysicalKeyboardKey.backspace,
              ),
            ];
        for (final key in navKeys) {
          final result = dispatchBoardPageKey(
            tester,
            logicalKey: key.logical,
            physicalKey: key.physical,
          );
          expect(
            result,
            KeyEventResult.ignored,
            reason: '${key.logical} must reach the field, not a board command',
          );
        }
        await tester.pump();
        expect(resolverCalls, isEmpty);
      },
    );

    testWidgets(
      'when_a_passive_embed_pin_is_focused_a_plain_key_is_not_a_board_shortcut',
      (tester) async {
        // Issue 6 gap 2: the platform webview never propagated DOM focus into
        // the Flutter tree (primary focus stays on the board root), yet the
        // focused pin IS an embed-bodied pin. Plain keys must still be gated
        // off board shortcuts.
        final resolverCalls = <String>[];
        await pumpBoardWithControlledEmbed(
          tester,
          onResolverKey: resolverCalls.add,
          focusedToolId: embedToolId,
        );

        // Do NOT focus the embed field — primary focus is the board root.
        expect(primaryFocusIsInsideEmbedBody(), isFalse);
        expect(primaryFocusIsEditableText(), isFalse);

        for (final probe in plainKeyProbes) {
          final result = dispatchBoardPageKey(
            tester,
            logicalKey: probe.logical,
            physicalKey: probe.physical,
            character: probe.character,
          );
          expect(result, KeyEventResult.ignored);
        }
        expect(resolverCalls, isEmpty);
        expect(find.byType(PaletteOverlay), findsNothing);

        // Cmd+K is still global even for a passive embed pin.
        await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
        final result = dispatchBoardPageKey(
          tester,
          logicalKey: LogicalKeyboardKey.keyK,
          physicalKey: PhysicalKeyboardKey.keyK,
          character: 'k',
        );
        await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
        await tester.pumpAndSettle();
        expect(result, KeyEventResult.handled);
        expect(find.byType(PaletteOverlay), findsOneWidget);
      },
    );
  });
}
