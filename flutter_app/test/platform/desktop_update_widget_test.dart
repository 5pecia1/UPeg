import 'dart:async';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/platform/app_lifecycle.dart';
import 'package:upeg/src/platform/desktop_update.dart';
import 'package:upeg/src/platform/tray.dart';
import 'package:upeg/src/rust/api/i18n.dart';
import 'package:upeg/src/state/desktop_update_provider.dart';
import 'package:upeg/src/state/locale_provider.dart';
import 'package:upeg/src/widgets/desktop_update_button.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  tearDown(() => debugDefaultTargetPlatformOverride = null);

  testWidgets(
    'approval runs once, blocks duplicate dialogs, and quits after install',
    (tester) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.linux;
      final gate = Completer<String?>();
      var calls = 0;
      var quits = 0;
      final container = ProviderContainer(
        overrides: [
          localeProvider.overrideWithValue(LocaleDto.en),
          i18nTranslateOverride.overrideWithValue((key, _) => key),
          i18nTranslateArgsOverride.overrideWithValue(
            (key, _, keys, values) => key,
          ),
          updateDismissalReaderProvider.overrideWithValue(() async => null),
          desktopUpdateServiceProvider.overrideWithValue(
            DesktopUpdateService(
              platform: TargetPlatform.linux,
              resolveLatest: () async => Uri.parse(
                'https://github.com/5pecia1/UPeg/releases/tag/v2.0.0',
              ),
              nativeUpdate: (_) {
                calls++;
                return gate.future;
              },
            ),
          ),
          quitAppProvider.overrideWithValue(() async {
            quits++;
          }),
        ],
      );
      addTearDown(container.dispose);
      await container.read(desktopUpdateProvider.notifier).check('1.0.0');
      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(home: Scaffold(body: DesktopUpdateButton())),
        ),
      );
      await tester.tap(find.byType(IconButton));
      await tester.pumpAndSettle();
      await tester.tap(find.text('desktop.update.approve'));
      await tester.pump();
      expect(calls, 1);
      expect(find.text('desktop.update.busy'), findsOneWidget);
      final context = tester.element(find.byType(DesktopUpdateButton));
      await showDesktopUpdateDialog(context);
      expect(find.byType(AlertDialog), findsOneWidget);
      gate.complete('installed');
      await tester.pumpAndSettle();
      expect(quits, 1);
      Navigator.of(tester.element(find.byType(AlertDialog))).pop();
      await tester.pumpAndSettle();
      await tester.pumpWidget(const SizedBox.shrink());
      await tester.pumpAndSettle();
      debugDefaultTargetPlatformOverride = null;
    },
  );

  testWidgets('Later dismisses without download, Windows approval opens page', (
    tester,
  ) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.windows;
    var opens = 0;
    String? dismissed;
    final container = ProviderContainer(
      overrides: [
        localeProvider.overrideWithValue(LocaleDto.en),
        i18nTranslateOverride.overrideWithValue((key, _) => key),
        i18nTranslateArgsOverride.overrideWithValue(
          (key, _, keys, values) => key,
        ),
        updateDismissalReaderProvider.overrideWithValue(() async => null),
        updateDismissalWriterProvider.overrideWithValue((value) async {
          dismissed = value;
        }),
        desktopUpdateServiceProvider.overrideWithValue(
          DesktopUpdateService(
            platform: TargetPlatform.windows,
            resolveLatest: () async => Uri.parse(
              'https://github.com/5pecia1/UPeg/releases/tag/v2.0.0',
            ),
            openUrl: (_) async {
              opens++;
              return true;
            },
          ),
        ),
      ],
    );
    addTearDown(container.dispose);
    await container.read(desktopUpdateProvider.notifier).check('1.0.0');
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const MaterialApp(home: Scaffold(body: DesktopUpdateButton())),
      ),
    );
    await tester.tap(find.byType(IconButton));
    await tester.pumpAndSettle();
    await tester.tap(find.text('desktop.update.later'));
    await tester.pumpAndSettle();
    expect(dismissed, '2.0.0');
    expect(opens, 0);
    await tester.tap(find.byType(IconButton));
    await tester.pumpAndSettle();
    await tester.tap(find.text('desktop.update.open_download'));
    await tester.pumpAndSettle();
    expect(opens, 1);
    debugDefaultTargetPlatformOverride = null;
  });

  test('tray keeps tool unread and adds update menu dynamically', () {
    expect(trayHasUnread(toolResults: true, update: false), isTrue);
    expect(trayHasUnread(toolResults: false, update: false), isFalse);
    expect(
      buildTrayMenu().items!.where((item) => item.key == 'open_update'),
      isEmpty,
    );
    expect(
      buildTrayMenu(
        updateLabel: 'Update',
      ).items!.where((item) => item.key == 'open_update'),
      hasLength(1),
    );
  });
}
