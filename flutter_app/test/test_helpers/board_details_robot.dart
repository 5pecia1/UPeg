import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/i18n.dart';
import 'package:upeg/src/rust/api/pegboard.dart' show BoardDto;
import 'package:upeg/src/state/board_details_provider.dart';
import 'package:upeg/src/state/boards_provider.dart';
import 'package:upeg/src/state/locale_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/board_details_dialog.dart';
import 'package:upeg/src/widgets/board_details_keys.dart';
import 'package:upeg/src/widgets/board_tabs.dart';
import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';

import '../shared/fake_board_details.dart';
import 'i18n_test_catalog.dart';
import 'fake_keyboard_resolver.dart';
import 'pegboard_selection_overrides.dart';

class BoardDetailsRobot {
  BoardDetailsRobot(this.tester);
  final WidgetTester tester;
  late FakeBoardDetails store;
  final clipboard = BoardDetailsClipboard();

  Future<void> open({
    bool project = false,
    bool native = true,
    bool saveFails = false,
    bool cliAvailable = true,
    bool copyFails = false,
    bool throughTabs = false,
    bool projectChanged = false,
    bool unresolvedPins = false,
  }) async {
    store = FakeBoardDetails(
      project: project,
      native: native,
      saveFails: saveFails,
      cliAvailable: cliAvailable,
      projectChanged: projectChanged,
      unresolvedPins: unresolvedPins,
    );
    clipboard.fails = copyFails;
    tester.view.physicalSize = const Size(1200, 1800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          localeProvider.overrideWithValue(LocaleDto.ko),
          ...i18nTestOverrides,
          ...pegboardSelectionOverrides(boardKey: boardDetailsTestKey),
          fakeKeyboardResolverOverride,
          boardsLoaderProvider.overrideWithValue(
            () => const [BoardDto(key: boardDetailsTestKey, title: 'Review')],
          ),
          boardDetailsLoaderProvider.overrideWithValue(store.load),
          boardGuidanceSaverProvider.overrideWithValue(store.save),
          boardConnectionLoaderProvider.overrideWithValue(store.preview),
          clipboardWriterProvider.overrideWithValue(clipboard),
        ],
        child: MaterialApp(
          theme: UpegTheme.darkTheme(),
          home: Scaffold(
            body: throughTabs
                ? BoardTabs(
                    onOpenPalette: () {},
                    onOpenSettings: () {},
                    onEditPinColor: () {},
                  )
                : const BoardDetailsDialog(boardKey: boardDetailsTestKey),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    if (throughTabs) {
      await tester.tap(find.text('Review'));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(BoardDetailsKeys.open));
      await tester.pumpAndSettle();
    }
    expect(find.byKey(BoardDetailsKeys.dialog), findsOneWidget);
    expect(tester.takeException(), isNull);
  }

  Future<void> editGuidance(String description, String instructions) async {
    await tester.enterText(
      find.byKey(BoardDetailsKeys.description),
      description,
    );
    await tester.enterText(
      find.byKey(BoardDetailsKeys.instructions),
      instructions,
    );
  }

  Future<void> save() async {
    await tester.ensureVisible(find.byKey(BoardDetailsKeys.save));
    await tester.tap(find.byKey(BoardDetailsKeys.save));
    await tester.pumpAndSettle();
  }

  void expectSavedGuidance(String description, String instructions) {
    expect(store.description, description);
    expect(store.instructions, instructions);
    expect(find.byKey(BoardDetailsKeys.saved), findsOneWidget);
    expect(find.byKey(BoardDetailsKeys.saveError), findsNothing);
  }

  void expectReconnectNotice() {
    expect(find.textContaining('기존 에이전트 세션을 다시 연결'), findsOneWidget);
  }

  void expectProjectGuidance() {
    expect(find.text(boardDetailsTestManifest), findsOneWidget);
    expectEditorText(store.description, store.instructions);
    expect(
      tester
          .widget<TextField>(find.byKey(BoardDetailsKeys.description))
          .readOnly,
      isTrue,
    );
    expect(
      tester
          .widget<TextField>(find.byKey(BoardDetailsKeys.instructions))
          .readOnly,
      isTrue,
    );
    expect(find.textContaining('[[boards]]'), findsOneWidget);
  }

  void expectSaveUnavailable() {
    expect(find.byKey(BoardDetailsKeys.save), findsNothing);
  }

  Future<void> openConnection() async {
    await tester.ensureVisible(find.byKey(BoardDetailsKeys.connection));
    await tester.tap(find.byKey(BoardDetailsKeys.connection));
    await tester.pumpAndSettle();
  }

  Future<void> expectEffectiveTools() async {
    expect(find.textContaining('upeg.board_context'), findsOneWidget);
    expect(find.text('git.status'), findsOneWidget);
    expect(find.text('remote.review'), findsOneWidget);
    expect(find.text('사용 가능 여부 미확인'), findsOneWidget);
    await tester.ensureVisible(find.text('git.status'));
    await tester.tap(find.text('git.status'));
    await tester.pumpAndSettle();
    expect(find.text('{"short":true}'), findsOneWidget);
    expect(find.text(boardDetailsTestDirectory), findsWidgets);
  }

  Future<void> copyConfiguration() async {
    await tester.ensureVisible(find.byKey(BoardDetailsKeys.copy));
    await tester.tap(find.byKey(BoardDetailsKeys.copy));
    await tester.pumpAndSettle();
  }

  void expectBoundConfigurationCopied() {
    expect(clipboard.text, boardDetailsTestConfig);
    final config = jsonDecode(clipboard.text!) as Map<String, dynamic>;
    final servers = config['mcpServers'] as Map<String, dynamic>;
    final server = servers['upeg-review'] as Map<String, dynamic>;
    expect(
      server['args'],
      containsAllInOrder([
        '--working-directory',
        boardDetailsTestDirectory,
        '--board',
        boardDetailsTestKey,
      ]),
    );
    expect(find.byKey(BoardDetailsKeys.copied), findsOneWidget);
  }

  void expectConnectionNotConfirmed() {
    expect(find.textContaining('에이전트에 추가하여 연결하세요'), findsOneWidget);
    expect(find.textContaining('실제 연결 상태를 확인하지 않습니다'), findsOneWidget);
    expect(find.text('연결 완료'), findsNothing);
  }

  void expectSaveFailure() {
    expect(find.byKey(BoardDetailsKeys.saveError), findsOneWidget);
    expect(find.byKey(BoardDetailsKeys.saved), findsNothing);
    expect(store.description, '검토용 보드');
  }

  void expectEditorText(String description, String instructions) {
    expect(
      tester
          .widget<TextField>(find.byKey(BoardDetailsKeys.description))
          .controller!
          .text,
      description,
    );
    expect(
      tester
          .widget<TextField>(find.byKey(BoardDetailsKeys.instructions))
          .controller!
          .text,
      instructions,
    );
  }

  void expectNativeConnectionUnavailable() {
    expect(find.byKey(BoardDetailsKeys.webNotice), findsOneWidget);
    expect(find.byKey(BoardDetailsKeys.connection), findsNothing);
    expect(find.byKey(BoardDetailsKeys.copy), findsNothing);
  }

  void expectCliUnavailable() {
    expect(find.textContaining('upeg CLI가 설치되어 있지 않거나'), findsOneWidget);
    expect(find.text('연결 완료'), findsNothing);
  }

  void expectCopyFailure() {
    expect(clipboard.text, isNull);
    expect(find.byKey(BoardDetailsKeys.copied), findsNothing);
    expect(find.textContaining('설정을 복사하지 못했습니다'), findsOneWidget);
  }

  void expectProjectRestartRequired() {
    expect(find.textContaining('프로젝트 파일이 변경되었습니다'), findsOneWidget);
    expect(find.textContaining('UPeg를 다시 시작하여'), findsOneWidget);
    expect(find.textContaining(boardDetailsTestManifest), findsOneWidget);
    expect(find.byKey(BoardDetailsKeys.instructions), findsNothing);
    expect(find.byKey(BoardDetailsKeys.save), findsNothing);
  }

  void expectBrowserSave() {
    expect(find.text('이 브라우저에 지침을 저장했습니다.'), findsOneWidget);
    expect(find.textContaining('기존 에이전트 세션을 다시 연결'), findsNothing);
  }

  void expectUnresolvedPinsNotice() {
    expect(find.textContaining('준비 상태 안내를 확인'), findsOneWidget);
    expect(find.textContaining('remote.review'), findsOneWidget);
    expect(find.textContaining('MCP를 지원하는 도구를 추가'), findsNothing);
  }
}
