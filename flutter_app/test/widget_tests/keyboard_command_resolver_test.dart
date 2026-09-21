import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/keyboard/keyboard_command_resolver.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('EmbedBodyFocusScope marker', () {
    testWidgets('focus_inside_the_scope_is_detected_as_embed_body', (
      tester,
    ) async {
      final node = FocusNode();
      addTearDown(node.dispose);

      await tester.pumpWidget(
        MaterialApp(
          home: EmbedBodyFocusScope(
            child: Focus(
              focusNode: node,
              child: const SizedBox(width: 10, height: 10),
            ),
          ),
        ),
      );

      node.requestFocus();
      await tester.pump();

      expect(FocusManager.instance.primaryFocus, node);
      expect(primaryFocusIsInsideEmbedBody(), isTrue);
    });

    testWidgets('focus_outside_the_scope_is_not_embed_body', (tester) async {
      final node = FocusNode();
      addTearDown(node.dispose);

      await tester.pumpWidget(
        MaterialApp(
          home: Focus(
            focusNode: node,
            child: const SizedBox(width: 10, height: 10),
          ),
        ),
      );

      node.requestFocus();
      await tester.pump();

      expect(FocusManager.instance.primaryFocus, node);
      expect(primaryFocusIsInsideEmbedBody(), isFalse);
    });

    testWidgets(
      'the_scope_wrapper_is_a_traversal_skip_node_that_never_requests_focus',
      (tester) async {
        await tester.pumpWidget(
          const MaterialApp(
            home: EmbedBodyFocusScope(child: SizedBox(width: 10, height: 10)),
          ),
        );

        final focus = tester.widget<Focus>(
          find.byWidgetPredicate(
            (widget) =>
                widget is Focus &&
                widget.debugLabel == EmbedBodyFocusScope.debugLabel,
          ),
        );
        expect(focus.canRequestFocus, isFalse);
        expect(focus.skipTraversal, isTrue);
      },
    );
  });

  group('keyboard command resolver', () {
    test(
      'keyboardlabelforevent_resolves_a_characterless_letter_key_to_lowercase',
      () {
        final event = KeyDownEvent(
          physicalKey: PhysicalKeyboardKey.keyK,
          logicalKey: LogicalKeyboardKey.keyK,
          timeStamp: Duration.zero,
        );

        expect(keyboardLabelForEvent(event), 'k');
      },
    );

    test(
      'isplaintextentrykeyevent_treats_a_characterless_letter_key_as_text',
      () {
        final event = KeyDownEvent(
          physicalKey: PhysicalKeyboardKey.keyQ,
          logicalKey: LogicalKeyboardKey.keyQ,
          timeStamp: Duration.zero,
        );

        expect(isPlainTextEntryKeyEvent(event), isTrue);
      },
    );
  });

  KeyDownEvent keyEvent(
    LogicalKeyboardKey logical,
    PhysicalKeyboardKey physical, {
    String? character,
  }) {
    return KeyDownEvent(
      physicalKey: physical,
      logicalKey: logical,
      timeStamp: Duration.zero,
      character: character,
    );
  }

  group('isGlobalShortcutKeyEvent', () {
    test('f1_is_a_global_shortcut_even_while_editing', () {
      expect(
        isGlobalShortcutKeyEvent(
          keyEvent(LogicalKeyboardKey.f1, PhysicalKeyboardKey.f1),
        ),
        isTrue,
      );
    });

    test('escape_is_global', () {
      expect(
        isGlobalShortcutKeyEvent(
          keyEvent(LogicalKeyboardKey.escape, PhysicalKeyboardKey.escape),
        ),
        isTrue,
      );
    });

    test('arrow_keys_are_not_global_and_yield_to_text', () {
      expect(
        isGlobalShortcutKeyEvent(
          keyEvent(LogicalKeyboardKey.arrowLeft, PhysicalKeyboardKey.arrowLeft),
        ),
        isFalse,
      );
    });

    test('backspace_is_not_global', () {
      expect(
        isGlobalShortcutKeyEvent(
          keyEvent(LogicalKeyboardKey.backspace, PhysicalKeyboardKey.backspace),
        ),
        isFalse,
      );
    });

    testWidgets('a_ctrl_combination_is_global', (tester) async {
      await tester.pumpWidget(const SizedBox());
      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      expect(
        isGlobalShortcutKeyEvent(
          keyEvent(
            LogicalKeyboardKey.keyK,
            PhysicalKeyboardKey.keyK,
            character: 'k',
          ),
        ),
        isTrue,
      );
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    });
  });

  group('shortcutKeysMatchEvent', () {
    testWidgets('cmd_shift_n_matches_a_ctrl_shift_n_event', (tester) async {
      await tester.pumpWidget(const SizedBox());
      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
      expect(
        shortcutKeysMatchEvent(
          'Cmd+Shift+N',
          keyEvent(
            LogicalKeyboardKey.keyN,
            PhysicalKeyboardKey.keyN,
            character: 'N',
          ),
        ),
        isTrue,
      );
      await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    });

    testWidgets('without_shift_the_event_does_not_match_cmd_shift_n', (
      tester,
    ) async {
      await tester.pumpWidget(const SizedBox());
      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      expect(
        shortcutKeysMatchEvent(
          'Cmd+Shift+N',
          keyEvent(
            LogicalKeyboardKey.keyN,
            PhysicalKeyboardKey.keyN,
            character: 'n',
          ),
        ),
        isFalse,
      );
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    });
  });
}
