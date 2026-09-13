import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/keyboard/keyboard_command_resolver.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('EmbedBodyFocusScope marker', () {
    testWidgets('scope_안쪽에_focus가_있으면_embed_body로_감지한다', (tester) async {
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

    testWidgets('scope_바깥의_focus는_embed_body가_아니다', (tester) async {
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

    testWidgets('scope_wrapper는_focus를_요청하지_않는_traversal_skip_노드다', (
      tester,
    ) async {
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
    });
  });

  group('keyboard command resolver', () {
    test('keyboardLabelForEvent는_character가_없는_letter_key를_소문자로_해석한다', () {
      final event = KeyDownEvent(
        physicalKey: PhysicalKeyboardKey.keyK,
        logicalKey: LogicalKeyboardKey.keyK,
        timeStamp: Duration.zero,
      );

      expect(keyboardLabelForEvent(event), 'k');
    });

    test('isPlainTextEntryKeyEvent는_character가_없는_일반_letter_key를_text로_본다', () {
      final event = KeyDownEvent(
        physicalKey: PhysicalKeyboardKey.keyQ,
        logicalKey: LogicalKeyboardKey.keyQ,
        timeStamp: Duration.zero,
      );

      expect(isPlainTextEntryKeyEvent(event), isTrue);
    });
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
    test('F1은_편집중에도_글로벌_단축키다', () {
      expect(
        isGlobalShortcutKeyEvent(
          keyEvent(LogicalKeyboardKey.f1, PhysicalKeyboardKey.f1),
        ),
        isTrue,
      );
    });

    test('Escape는_글로벌이다', () {
      expect(
        isGlobalShortcutKeyEvent(
          keyEvent(LogicalKeyboardKey.escape, PhysicalKeyboardKey.escape),
        ),
        isTrue,
      );
    });

    test('방향키는_글로벌이_아니라_텍스트로_양보된다', () {
      expect(
        isGlobalShortcutKeyEvent(
          keyEvent(LogicalKeyboardKey.arrowLeft, PhysicalKeyboardKey.arrowLeft),
        ),
        isFalse,
      );
    });

    test('Backspace는_글로벌이_아니다', () {
      expect(
        isGlobalShortcutKeyEvent(
          keyEvent(LogicalKeyboardKey.backspace, PhysicalKeyboardKey.backspace),
        ),
        isFalse,
      );
    });

    testWidgets('Ctrl_조합은_글로벌이다', (tester) async {
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
    testWidgets('Cmd_Shift_N을_Ctrl_Shift_N_이벤트와_매치한다', (tester) async {
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

    testWidgets('Shift가_빠지면_Cmd_Shift_N에_매치하지_않는다', (tester) async {
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
