/// Widget tests for [BoardTabs].
///
/// `boardsLoaderProvider` is overridden so the FRB call is never
/// invoked. Tap behavior writes to `currentBoardKeyProvider`; we
/// observe that via a fresh `ProviderContainer`-equivalent read on
/// the test `WidgetRef`.
library;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/platform/keyboard_label.dart';
import 'package:upeg/src/rust/api/i18n.dart' show LocaleDto;
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/focused_pin_provider.dart';
import 'package:upeg/src/state/locale_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/board_tabs.dart';

import '../test_helpers/fake_keyboard_resolver.dart';
import '../test_helpers/pegboard_selection_overrides.dart';
import '../test_helpers/i18n_test_catalog.dart';

Widget _harness({
  required List<BoardDto> boards,
  ProviderContainer? container,
}) {
  return UncontrolledProviderScope(
    container: container ?? _container(boards),
    child: MaterialApp(
      theme: UpegTheme.darkTheme(),
      home: Scaffold(
        body: BoardTabs(
          onOpenPalette: () {},
          onOpenSettings: () {},
          onEditPinColor: () {},
        ),
      ),
    ),
  );
}

Widget _sizedHarness({
  required List<BoardDto> boards,
  required Size size,
  ProviderContainer? container,
}) {
  return UncontrolledProviderScope(
    container: container ?? _container(boards, locale: LocaleDto.ko),
    child: MaterialApp(
      theme: UpegTheme.darkTheme(),
      home: Scaffold(
        body: SizedBox(
          width: size.width,
          height: size.height,
          child: BoardTabs(
            onOpenPalette: () {},
            onOpenSettings: () {},
            onEditPinColor: () {},
          ),
        ),
      ),
    ),
  );
}

ProviderContainer _container(
  List<BoardDto> boards, {
  LocaleDto locale = LocaleDto.en,
  TargetPlatform keyboardPlatform = TargetPlatform.macOS,
  String Function(String title)? boardCreator,
  void Function(String boardKey, String newTitle)? boardRenamer,
  void Function(String boardKey)? boardDeleter,
}) {
  final container = ProviderContainer(
    overrides: [
      ...pegboardSelectionOverrides(),
      boardsLoaderProvider.overrideWith(
        (ref) =>
            () => boards,
      ),
      localeProvider.overrideWithValue(locale),
      ...i18nTestOverrides,
      fakeKeyboardResolverOverride,
      // Pin the default so label-focused tests do not vary by host.
      // Overflow-specific coverage overrides this with Linux below.
      keyboardPlatformProvider.overrideWithValue(keyboardPlatform),
      if (boardCreator != null)
        boardCreatorProvider.overrideWith((ref) => boardCreator),
      if (boardRenamer != null)
        boardRenamerProvider.overrideWith((ref) => boardRenamer),
      if (boardDeleter != null)
        boardDeleterProvider.overrideWith((ref) => boardDeleter),
    ],
  );
  return container;
}

Future<void> _openBoardMenu(WidgetTester tester, String boardKey) async {
  final tab = find.byKey(ValueKey('board-tab-$boardKey'));
  final gesture = await tester.startGesture(
    tester.getCenter(tab),
    kind: PointerDeviceKind.mouse,
    buttons: kSecondaryMouseButton,
  );
  await gesture.up();
  await tester.pumpAndSettle();
}

void main() {
  // Pin the keyboard platform via the Riverpod provider so the
  // `⌘K` (mac) / `Ctrl K` (others) modifier label (CC, see
  // `lib/src/platform/keyboard_label.dart`) renders deterministically.
  // Overriding via the provider keeps tests off the foundation
  // debug var (`debugDefaultTargetPlatformOverride`), which trips
  // `_verifyInvariants` on every widget-test pump.

  group('BoardTabs', () {
    testWidgets('BoardTabs_renders_each_board_title', (tester) async {
      const boards = <BoardDto>[
        BoardDto(key: 'dev', title: 'Dev'),
        BoardDto(key: 'media', title: 'Media'),
        BoardDto(key: 'misc', title: 'Misc'),
      ];
      await tester.pumpWidget(_harness(boards: boards));
      await tester.pumpAndSettle();

      expect(find.text('Dev'), findsOneWidget);
      expect(find.text('Media'), findsOneWidget);
      expect(find.text('Misc'), findsOneWidget);
    });

    testWidgets('tapping_a_BoardTabs_tab_updates_currentBoardProvider', (
      tester,
    ) async {
      const boards = <BoardDto>[
        BoardDto(key: 'dev', title: 'Dev'),
        BoardDto(key: 'media', title: 'Media'),
      ];
      final container = _container(boards);
      addTearDown(container.dispose);

      await tester.pumpWidget(_harness(boards: boards, container: container));
      await tester.pumpAndSettle();

      // Initially nothing is selected.
      expect(container.read(currentBoardKeyProvider), isNull);

      await tester.tap(find.byKey(const ValueKey('board-tab-media')));
      await tester.pumpAndSettle();

      expect(container.read(currentBoardKeyProvider), BoardKey.parse('media'));
    });

    testWidgets('BoardTabs_renders_even_with_an_empty_board_list', (
      tester,
    ) async {
      // Post-parity: BoardTabs no longer prints a "(no boards registered)"
      // placeholder — the empty-state copy moved to the BoardCanvas. The
      // tab strip just renders its right-aligned action buttons.
      await tester.pumpWidget(_harness(boards: const <BoardDto>[]));
      await tester.pumpAndSettle();

      expect(find.byKey(const Key('open-palette-btn')), findsOneWidget);
      expect(find.byKey(const Key('open-settings-btn')), findsOneWidget);
    });

    testWidgets('BoardTabs_switches_the_search_label_to_Korean_in_locale_Ko', (
      tester,
    ) async {
      const boards = <BoardDto>[BoardDto(key: 'dev', title: 'Dev')];
      final container = _container(boards, locale: LocaleDto.ko);
      addTearDown(container.dispose);

      await tester.pumpWidget(_harness(boards: boards, container: container));
      await tester.pumpAndSettle();

      // EN catalog: 'search ' (trailing space). KO catalog: '검색 '.
      expect(find.text('search'), findsNothing);
      expect(find.textContaining('검색'), findsOneWidget);
      expect(find.text('설정'), findsOneWidget);
      expect(find.text('+ 보드'), findsOneWidget);
      expect(find.text('+ 핀'), findsOneWidget);
    });

    testWidgets('BoardTabs_renders_the_UpegPopover_on_right_click', (
      tester,
    ) async {
      // Right-click should open the custom UpegPopover (not Material
      // showMenu). Verifies the Rename/Delete entries appear with the
      // ghost-styled labels.
      const boards = <BoardDto>[BoardDto(key: 'dev', title: 'Dev')];
      await tester.pumpWidget(_harness(boards: boards));
      await tester.pumpAndSettle();

      final tab = find.byKey(const ValueKey('board-tab-dev'));
      final gesture = await tester.startGesture(
        tester.getCenter(tab),
        kind: PointerDeviceKind.mouse,
        buttons: kSecondaryMouseButton,
      );
      await gesture.up();
      await tester.pumpAndSettle();

      expect(find.byKey(const Key('board-menu-rename-dev')), findsOneWidget);
      expect(find.byKey(const Key('board-menu-delete-dev')), findsOneWidget);
      expect(find.text('Rename…'), findsOneWidget);
      expect(find.text('Delete'), findsOneWidget);
    });

    testWidgets(
      'BoardTabs_opens_the_board_menu_and_selects_items_by_keyboard',
      (tester) async {
        const boards = <BoardDto>[BoardDto(key: 'dev', title: 'Dev')];
        await tester.pumpWidget(_harness(boards: boards));
        await tester.pumpAndSettle();

        await tester.sendKeyEvent(LogicalKeyboardKey.tab);
        await tester.pump();
        await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
        await tester.sendKeyEvent(LogicalKeyboardKey.f10);
        await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
        await tester.pumpAndSettle();

        expect(find.byKey(const Key('board-menu-rename-dev')), findsOneWidget);
        expect(find.byKey(const Key('board-menu-delete-dev')), findsOneWidget);

        await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
        await tester.sendKeyEvent(LogicalKeyboardKey.enter);
        await tester.pumpAndSettle();

        expect(
          find.byKey(const Key('delete-board-dialog-dev')),
          findsOneWidget,
        );
      },
    );

    testWidgets(
      'BoardTabs_does_not_overflow_at_narrow_widths_with_long_Korean_labels',
      (tester) async {
        const boards = <BoardDto>[
          BoardDto(key: 'dev', title: '개발도구모음보드'),
          BoardDto(key: 'media', title: '미디어변환작업보드'),
          BoardDto(key: 'ops', title: '운영자동화긴보드'),
        ];
        final container = _container(boards, locale: LocaleDto.ko);
        addTearDown(container.dispose);

        await tester.pumpWidget(
          _sizedHarness(
            boards: boards,
            size: const Size(320, UpegSizing.tabBarHeight),
            container: container,
          ),
        );
        await tester.pumpAndSettle();

        expect(tester.takeException(), isNull);
        expect(find.byType(SingleChildScrollView), findsOneWidget);
        expect(find.text('설정'), findsOneWidget);
      },
    );

    testWidgets('BoardTabs_does_not_overflow_with_linux_CtrlK_and_3_boards', (
      tester,
    ) async {
      const boards = <BoardDto>[
        BoardDto(key: 'dev', title: 'Dev'),
        BoardDto(key: 'media', title: 'Media'),
        BoardDto(key: 'ops', title: 'Ops'),
      ];
      final container = _container(
        boards,
        keyboardPlatform: TargetPlatform.linux,
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(
        _sizedHarness(
          boards: boards,
          size: const Size(800, UpegSizing.tabBarHeight),
          container: container,
        ),
      );
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
      expect(find.byKey(const Key('open-settings-btn')), findsOneWidget);
    });

    testWidgets('BoardTabs_BoardEditor_commits_create_on_Enter', (
      tester,
    ) async {
      final created = <String>[];
      const boards = <BoardDto>[BoardDto(key: 'dev', title: 'Dev')];
      final container = _container(
        boards,
        boardCreator: (title) {
          created.add(title);
          return 'ops';
        },
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(_harness(boards: boards, container: container));
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('add-board-btn')));
      await tester.pump();
      await tester.enterText(
        find.byKey(const Key('create-board-field')),
        ' Ops ',
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();

      expect(created, <String>['Ops']);
      expect(container.read(currentBoardKeyProvider), BoardKey.parse('ops'));
      expect(find.byKey(const Key('create-board-dialog')), findsNothing);
    });

    testWidgets(
      'BoardTabs_BoardEditor_cancels_create_on_Escape_without_a_transition',
      (tester) async {
        final created = <String>[];
        const boards = <BoardDto>[BoardDto(key: 'dev', title: 'Dev')];
        final container = _container(
          boards,
          boardCreator: (title) {
            created.add(title);
            return 'ops';
          },
        );
        addTearDown(container.dispose);

        await tester.pumpWidget(_harness(boards: boards, container: container));
        await tester.pumpAndSettle();

        await tester.tap(find.byKey(const Key('add-board-btn')));
        await tester.pump();
        await tester.enterText(
          find.byKey(const Key('create-board-field')),
          'Ops',
        );
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await tester.pump();

        expect(created, isEmpty);
        expect(find.byKey(const Key('create-board-dialog')), findsNothing);
      },
    );

    testWidgets('BoardTabs_BoardEditor_commits_rename_on_Enter', (
      tester,
    ) async {
      final renamed = <(String, String)>[];
      const boards = <BoardDto>[BoardDto(key: 'dev', title: 'Dev')];
      final container = _container(
        boards,
        boardRenamer: (boardKey, newTitle) => renamed.add((boardKey, newTitle)),
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(_harness(boards: boards, container: container));
      await tester.pumpAndSettle();

      await _openBoardMenu(tester, 'dev');
      await tester.tap(find.byKey(const Key('board-menu-rename-dev')));
      await tester.pumpAndSettle();
      await tester.enterText(
        find.byKey(const Key('rename-board-field')),
        'Development',
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();

      expect(renamed, <(String, String)>[('dev', 'Development')]);
      expect(find.byKey(const Key('rename-board-dialog-dev')), findsNothing);
    });

    testWidgets('BoardTabs_ConfirmDelete_confirms_on_y_and_Enter', (
      tester,
    ) async {
      final deleted = <String>[];
      const boards = <BoardDto>[
        BoardDto(key: 'dev', title: 'Dev'),
        BoardDto(key: 'ops', title: 'Ops'),
      ];
      final container = _container(boards, boardDeleter: deleted.add);
      addTearDown(container.dispose);

      await tester.pumpWidget(_harness(boards: boards, container: container));
      await tester.pumpAndSettle();

      await _openBoardMenu(tester, 'dev');
      await tester.tap(find.byKey(const Key('board-menu-delete-dev')));
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.keyY);
      await tester.pumpAndSettle();

      await _openBoardMenu(tester, 'ops');
      await tester.tap(find.byKey(const Key('board-menu-delete-ops')));
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();

      expect(deleted, <String>['dev', 'ops']);
      expect(find.byKey(const Key('delete-board-dialog-dev')), findsNothing);
      expect(find.byKey(const Key('delete-board-dialog-ops')), findsNothing);
    });

    testWidgets(
      'BoardTabs_selects_a_remaining_board_after_deleting_the_current_one',
      (tester) async {
        final deleted = <String>[];
        const boards = <BoardDto>[
          BoardDto(key: 'dev', title: 'Dev'),
          BoardDto(key: 'ops', title: 'Ops'),
        ];
        final container = _container(boards, boardDeleter: deleted.add);
        addTearDown(container.dispose);
        container
            .read(currentBoardKeyProvider.notifier)
            .select(BoardKey.parse('dev'));

        await tester.pumpWidget(_harness(boards: boards, container: container));
        await tester.pumpAndSettle();

        await _openBoardMenu(tester, 'dev');
        await tester.tap(find.byKey(const Key('board-menu-delete-dev')));
        await tester.pumpAndSettle();
        await tester.sendKeyEvent(LogicalKeyboardKey.keyY);
        await tester.pumpAndSettle();

        expect(deleted, <String>['dev']);
        expect(container.read(currentBoardKeyProvider), BoardKey.parse('ops'));
      },
    );

    testWidgets('BoardTabs_ConfirmDelete_cancels_on_n_q_and_Escape', (
      tester,
    ) async {
      final deleted = <String>[];
      const boards = <BoardDto>[BoardDto(key: 'dev', title: 'Dev')];
      final container = _container(boards, boardDeleter: deleted.add);
      addTearDown(container.dispose);

      await tester.pumpWidget(_harness(boards: boards, container: container));
      await tester.pumpAndSettle();

      for (final key in <LogicalKeyboardKey>[
        LogicalKeyboardKey.keyN,
        LogicalKeyboardKey.keyQ,
        LogicalKeyboardKey.escape,
      ]) {
        await _openBoardMenu(tester, 'dev');
        await tester.tap(find.byKey(const Key('board-menu-delete-dev')));
        await tester.pumpAndSettle();
        await tester.sendKeyEvent(key);
        await tester.pumpAndSettle();
        expect(find.byKey(const Key('delete-board-dialog-dev')), findsNothing);
      }

      expect(deleted, isEmpty);
    });

    testWidgets('the_color_button_is_enabled_when_a_pin_is_focused', (
      tester,
    ) async {
      var opened = false;
      const boards = <BoardDto>[BoardDto(key: 'dev', title: 'Dev')];
      final container = _container(boards);
      container
          .read(focusedPinProvider.notifier)
          .focus(PinId.parse('num.hex_to_decimal'));
      addTearDown(container.dispose);

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: MaterialApp(
            theme: UpegTheme.darkTheme(),
            home: Scaffold(
              body: BoardTabs(
                onOpenPalette: () {},
                onOpenSettings: () {},
                onEditPinColor: () => opened = true,
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('edit-pin-color-btn')));
      await tester.pumpAndSettle();
      expect(opened, isTrue);
    });

    testWidgets('the_color_button_is_disabled_without_a_focused_pin', (
      tester,
    ) async {
      var opened = false;
      const boards = <BoardDto>[BoardDto(key: 'dev', title: 'Dev')];
      final container = _container(boards);
      addTearDown(container.dispose);

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: MaterialApp(
            theme: UpegTheme.darkTheme(),
            home: Scaffold(
              body: BoardTabs(
                onOpenPalette: () {},
                onOpenSettings: () {},
                onEditPinColor: () => opened = true,
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      // Disabled button is wrapped in Opacity with 0.45
      final buttonFinder = find.byKey(const Key('edit-pin-color-btn'));
      expect(buttonFinder, findsOneWidget);
      final opacity = tester.widget<Opacity>(
        find.descendant(of: buttonFinder, matching: find.byType(Opacity)).first,
      );
      expect(opacity.opacity, 0.45);

      // Tapping a disabled button does nothing
      await tester.tap(buttonFinder);
      await tester.pumpAndSettle();
      expect(opened, isFalse);
    });

    testWidgets('BoardTabs_tracks_the_color_button_disabled_state_to_focus', (
      tester,
    ) async {
      var opened = false;
      const boards = <BoardDto>[BoardDto(key: 'dev', title: 'Dev')];
      final container = _container(boards);
      addTearDown(container.dispose);

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: MaterialApp(
            theme: UpegTheme.darkTheme(),
            home: Scaffold(
              body: BoardTabs(
                onOpenPalette: () {},
                onOpenSettings: () {},
                onEditPinColor: () => opened = true,
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      // Initially no focused pin → disabled
      final btnFinder = find.byKey(const Key('edit-pin-color-btn'));
      var opacity = tester.widget<Opacity>(
        find.descendant(of: btnFinder, matching: find.byType(Opacity)).first,
      );
      expect(opacity.opacity, 0.45);

      // Focus a pin → becomes enabled
      container
          .read(focusedPinProvider.notifier)
          .focus(PinId.parse('num.hex_to_decimal'));
      await tester.pumpAndSettle();

      // Now tapping should work
      await tester.tap(btnFinder);
      await tester.pumpAndSettle();
      expect(opened, isTrue);
    });

    testWidgets('the_edit_toggle_button_was_removed_in_modeless', (
      tester,
    ) async {
      const boards = <BoardDto>[BoardDto(key: 'dev', title: 'Dev')];
      await tester.pumpWidget(_harness(boards: boards));
      await tester.pumpAndSettle();

      // Modeless: there is no edit/done toggle in the tab bar anymore.
      expect(find.byIcon(Icons.edit), findsNothing);
      expect(find.text('edit'), findsNothing);
      expect(find.text('done'), findsNothing);
      // The rest of the action cluster still renders.
      expect(find.byKey(const Key('open-settings-btn')), findsOneWidget);
      expect(find.byKey(const Key('open-palette-btn')), findsOneWidget);
    });
  });
}
