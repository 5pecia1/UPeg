/// `?` keyboard-cheatsheet overlay widget tests.
///
/// The overlay renders the shared binding catalog
/// (`keyboardBindingCatalogProvider`), overridden here with the shared
/// fixture catalog so no native dylib is needed. Catalog↔resolver
/// parity itself is pinned by the Rust tests in
/// `upeg-core/src/keyboard_catalog.rs`.
library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/platform/keyboard_label.dart';
import 'package:upeg/src/rust/api/keyboard.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/cheatsheet_overlay.dart';

import '../test_helpers/fake_binding_catalog.dart';
import '../test_helpers/fake_keyboard_resolver.dart';
import '../test_helpers/i18n_test_catalog.dart';

Widget _overlayHarness({
  KeyboardScopeDto currentScope = KeyboardScopeDto.board,
  TargetPlatform platform = TargetPlatform.linux,
}) {
  return ProviderScope(
    overrides: [
      ...i18nTestOverrides,
      fakeKeyboardResolverOverride,
      fakeBindingCatalogOverride,
      keyboardPlatformProvider.overrideWithValue(platform),
    ],
    child: MaterialApp(
      theme: UpegTheme.darkTheme(),
      home: Scaffold(body: CheatsheetOverlay(currentScope: currentScope)),
    ),
  );
}

/// Opens the overlay through the real dialog route so Esc dismissal
/// exercises the same path as the board page.
Widget _dialogHarness() {
  return ProviderScope(
    overrides: [
      ...i18nTestOverrides,
      fakeKeyboardResolverOverride,
      fakeBindingCatalogOverride,
    ],
    child: MaterialApp(
      theme: UpegTheme.darkTheme(),
      home: Scaffold(
        body: Builder(
          builder: (context) => TextButton(
            onPressed: () => showCheatsheetOverlay(context),
            child: const Text('open-cheatsheet'),
          ),
        ),
      ),
    ),
  );
}

void main() {
  group('CheatsheetOverlay', () {
    testWidgets('카탈로그의_모든_스코프_섹션을_렌더한다', (tester) async {
      await tester.pumpWidget(_overlayHarness());
      await tester.pump();

      for (final section in fakeBindingCatalog) {
        expect(
          find.byKey(CheatsheetOverlay.scopeSectionKey(section.scope)),
          findsOneWidget,
        );
      }
      expect(find.text(i18nEn('keys.title')), findsOneWidget);
      expect(find.text(i18nEn('keys.scope.board')), findsOneWidget);
      expect(find.text(i18nEn('keys.cmd.open')), findsOneWidget);
    });

    testWidgets('현재_컨텍스트_스코프_섹션이_상단에_온다', (tester) async {
      await tester.pumpWidget(
        _overlayHarness(currentScope: KeyboardScopeDto.resize),
      );
      await tester.pump();

      final resizeTop = tester
          .getTopLeft(
            find.byKey(
              CheatsheetOverlay.scopeSectionKey(KeyboardScopeDto.resize),
            ),
          )
          .dy;
      final boardTop = tester
          .getTopLeft(
            find.byKey(
              CheatsheetOverlay.scopeSectionKey(KeyboardScopeDto.board),
            ),
          )
          .dy;
      expect(resizeTop, lessThan(boardTop));
    });

    testWidgets('연속_숫자_키캡은_범위_캡으로_압축된다', (tester) async {
      await tester.pumpWidget(_overlayHarness());
      await tester.pump();

      // 보드 슬롯 1..9는 아홉 개의 캡이 아니라 하나의 `1–9` 캡으로.
      expect(find.text('1–9'), findsOneWidget);
      expect(find.text('1'), findsNothing);
      expect(find.text('9'), findsNothing);
    });

    testWidgets('primary_chord는_OS_관용_라벨로_렌더된다', (tester) async {
      await tester.pumpWidget(_overlayHarness(platform: TargetPlatform.macOS));
      await tester.pump();

      // macOS에서는 Cmd+K가 ⌘k 캡으로 (keyboard_label.dart 재사용).
      expect(find.text('⌘k'), findsOneWidget);
    });

    testWidgets('포커스_게이트_엔트리는_focused_pin_배지를_단다', (tester) async {
      await tester.pumpWidget(_overlayHarness());
      await tester.pump();

      expect(
        find.textContaining(i18nEn('keys.requires_focus'), findRichText: true),
        findsWidgets,
      );
    });

    testWidgets('Esc로_닫힌다', (tester) async {
      await tester.pumpWidget(_dialogHarness());
      await tester.tap(find.text('open-cheatsheet'));
      await tester.pumpAndSettle();
      expect(find.byKey(CheatsheetOverlay.overlayKey), findsOneWidget);

      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(find.byKey(CheatsheetOverlay.overlayKey), findsNothing);
    });
  });
}
