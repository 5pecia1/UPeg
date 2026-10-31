import 'package:flutter/material.dart';
import 'dart:ui' show Tristate, SemanticsAction;
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter/semantics.dart' show SemanticsNode;
import 'package:upeg/src/widgets/board_details_keys.dart';
import 'package:upeg/src/widgets/board_details_dialog.dart';

class UiWorkflowRobot {
  UiWorkflowRobot(this.tester);

  final WidgetTester tester;

  Future<void> pumpScreen(Widget widget) async {
    await tester.pumpWidget(widget);
    await tester.pumpAndSettle();
  }

  Future<void> openSearch() async {
    await tester.tap(find.byKey(const Key('open-palette-btn')));
    await tester.pumpAndSettle();
  }

  Future<void> openMore() async {
    await tester.tap(find.byKey(const Key('board-more-actions-btn')));
    await tester.pumpAndSettle();
  }

  Future<void> openDetailsFromMenu() async {
    await tester.tap(find.byKey(BoardDetailsKeys.open));
    await tester.pumpAndSettle();
  }

  void expectDetailsForBoard(String key) {
    expect(tester.takeException(), isNull);
    expect(find.byKey(BoardDetailsKeys.dialog), findsOneWidget);
    expect(
      tester
          .widget<BoardDetailsDialog>(find.byType(BoardDetailsDialog))
          .boardKey,
      key,
    );
  }

  Future<void> openSettingsFromMore() async {
    await tester.tap(find.byKey(const Key('board-more-actions-btn')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('open-settings-btn')));
    await tester.pumpAndSettle();
  }

  Future<void> requestCreateBoard(String title) async {
    await tester.tap(find.byKey(const Key('add-board-btn')));
    await tester.pumpAndSettle();
    await tester.enterText(find.byKey(const Key('create-board-field')), title);
    await tester.tap(find.byKey(const Key('create-board-submit')));
    await tester.pumpAndSettle();
  }

  Future<void> requestRenameBoard(String boardKey, String title) async {
    await tester.longPress(find.byKey(ValueKey('board-tab-$boardKey')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(Key('board-menu-rename-$boardKey')));
    await tester.pumpAndSettle();
    await tester.enterText(find.byKey(const Key('rename-board-field')), title);
    await tester.tap(find.byKey(const Key('rename-board-submit')));
    await tester.pumpAndSettle();
  }

  Future<void> requestDeleteBoard(String boardKey) async {
    await tester.longPress(find.byKey(ValueKey('board-tab-$boardKey')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(Key('board-menu-delete-$boardKey')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Delete board'));
    await tester.pumpAndSettle();
  }

  void expectBoardFailureVisible(String action) {
    expect(find.text('Could not $action board. Try again.'), findsOneWidget);
    expect(tester.takeException(), isNull);
  }

  void expectBoardFailureNotVisible(String action) =>
      expect(find.text('Could not $action board. Try again.'), findsNothing);

  void expectPrimaryActionsReachable(double viewportWidth) {
    for (final key in ['open-palette-btn', 'board-more-actions-btn']) {
      final finder = find.byKey(Key(key));
      final rect = tester.getRect(finder);
      expect(rect.left, greaterThanOrEqualTo(0));
      expect(rect.right, lessThanOrEqualTo(viewportWidth));
      expect(rect.height, greaterThanOrEqualTo(44));
      expect(finder.hitTestable(), findsOneWidget);
    }
    expect(tester.takeException(), isNull);
  }

  void expectSelectedBoard(
    String selectedKey,
    String unselectedKey, {
    String selectedLabel = 'Dev',
  }) {
    final selected = tester.getSemantics(
      find.byKey(ValueKey('board-tab-$selectedKey')),
    );
    final unselected = tester.getSemantics(
      find.byKey(ValueKey('board-tab-$unselectedKey')),
    );
    expect(selected.flagsCollection.isSelected, Tristate.isTrue);
    expect(unselected.flagsCollection.isSelected, Tristate.isFalse);
    final buttons = <SemanticsNode>[];
    void collect(SemanticsNode node) {
      if (node.flagsCollection.isButton) buttons.add(node);
      node.visitChildren((child) {
        collect(child);
        return true;
      });
    }

    collect(selected);
    expect(buttons, hasLength(1));
    expect(buttons.single.getSemanticsData().label, selectedLabel);
  }

  Future<void> tapBoardThroughSemantics(String boardKey) async {
    final node = tester.getSemantics(
      find.byKey(ValueKey('board-tab-$boardKey')),
    );
    expect(node.getSemanticsData().hasAction(SemanticsAction.tap), isTrue);
    node.owner!.performAction(node.id, SemanticsAction.tap);
    await tester.pumpAndSettle();
  }

  Future<void> tapCopy() async {
    await tester.tap(find.byIcon(Icons.copy_outlined));
    await tester.pump();
  }

  void expectCopiedVisible() =>
      expect(find.byIcon(Icons.check), findsOneWidget);
  void expectCopiedNotVisible() =>
      expect(find.byIcon(Icons.check), findsNothing);

  void expectCopyFailureVisible() {
    expect(find.byIcon(Icons.error_outline), findsOneWidget);
    expect(find.byTooltip('Could not copy. Try again.'), findsOneWidget);
    expect(find.text('Could not copy. Try again.'), findsOneWidget);
  }

  void expectCopyFailureNotVisible() =>
      expect(find.byIcon(Icons.error_outline), findsNothing);

  void expectCopyDisabled() {
    expect(
      tester.widget<IconButton>(find.byType(IconButton)).onPressed,
      isNull,
    );
  }

  Future<void> toggleCurrentTagSearch() async {
    await tester.tap(find.byKey(const Key('palette-current-tag-chip')));
    await tester.pumpAndSettle();
  }

  Future<void> searchPalette(String query) async {
    await tester.enterText(find.byKey(const Key('palette-query-field')), query);
    await tester.pumpAndSettle();
  }

  void expectCurrentTagSearchVisible(String tag, {required bool selected}) {
    expect(find.text('Search current tag: $tag'), findsOneWidget);
    final chip = tester.widget<FilterChip>(
      find.byKey(const Key('palette-current-tag-chip')),
    );
    expect(chip.selected, selected);
  }

  void expectCurrentTagSearchNotVisible() {
    expect(find.byKey(const Key('palette-current-tag-chip')), findsNothing);
  }

  void expectPaletteToolVisible(String id) =>
      expect(find.byKey(ValueKey('palette-hit-$id')), findsOneWidget);

  void expectPaletteToolNotVisible(String id) =>
      expect(find.byKey(ValueKey('palette-hit-$id')), findsNothing);
}
