/// Geometry re-apply on every WindowMode flip (inventory row C03).
///
/// The retired Dioxus surface re-applied window geometry via an effect
/// on every `mode` change.
/// The Flutter equivalent must (a) call applyWindowMode at boot, AND
/// (b) call it again on every subsequent flip — otherwise transitions
/// driven from outside the tray dispatch path (e.g. popup_page's
/// "open desktop" tap, the LaunchIntentApplier forcing full on deep
/// link) leave the OS window stuck at the old geometry.
///
/// The C03 row was ⚠️ "no integration test" because the live path
/// depends on `window_manager`. This test exercises the new
/// `WindowModeApplier` observer with an injected `ApplyWindowMode`
/// seam — same shape as `PopupAutoHideObserver`'s `WindowHider`.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/state/window_mode_provider.dart';
import 'package:upeg/src/widgets/window_mode_applier.dart';

import '../test_helpers/i18n_test_catalog.dart';

void main() {
  group('WindowModeApplier', () {
    testWidgets('App은_windowMode가_바뀌면_geometry를_재적용한다', (tester) async {
      // Start in popup; flip to full; assert the apply seam was called
      // for the flip. The boot-time call is observed too so the test
      // also catches a regression that drops the initial application.
      final applied = <WindowMode>[];
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            ...i18nTestOverrides,
            windowModeProvider.overrideWith(
              () => WindowModeNotifier(initial: WindowMode.popup),
            ),
          ],
          child: MaterialApp(
            home: WindowModeApplier(
              applyWindowMode: (mode) async {
                applied.add(mode);
              },
              child: const SizedBox.shrink(),
            ),
          ),
        ),
      );
      await tester.pump();
      // Boot-time application observed once for the initial mode.
      expect(applied, [WindowMode.popup]);

      // Flip the provider; the observer must call apply with the new mode.
      final container = ProviderScope.containerOf(
        tester.element(find.byType(WindowModeApplier)),
      );
      container.read(windowModeProvider.notifier).set(WindowMode.full);
      await tester.pump();

      expect(
        applied,
        [WindowMode.popup, WindowMode.full],
        reason: 'WindowModeApplier must re-apply geometry on every mode flip',
      );
    });

    testWidgets('WindowModeApplier는_같은_모드_재설정시_재적용하지_않는다', (tester) async {
      // Idempotency guard: setting the same mode twice in a row must
      // not re-fire the platform call. The Riverpod listen contract
      // already swallows equal states, but pin the behaviour at the
      // observer level so a future refactor that switches to a manual
      // `addListener` (where equal-state semantics differ) is forced
      // to re-add the dedupe.
      final applied = <WindowMode>[];
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            ...i18nTestOverrides,
            windowModeProvider.overrideWith(
              () => WindowModeNotifier(initial: WindowMode.full),
            ),
          ],
          child: MaterialApp(
            home: WindowModeApplier(
              applyWindowMode: (mode) async {
                applied.add(mode);
              },
              child: const SizedBox.shrink(),
            ),
          ),
        ),
      );
      await tester.pump();
      expect(applied, [WindowMode.full]);

      final container = ProviderScope.containerOf(
        tester.element(find.byType(WindowModeApplier)),
      );
      container.read(windowModeProvider.notifier).set(WindowMode.full);
      await tester.pump();

      expect(applied, [
        WindowMode.full,
      ], reason: 'Equal-state re-set must not re-fire applyWindowMode');
    });

    testWidgets('WindowModeApplier는_platform_call_예외를_격리한다', (tester) async {
      final originalDebugPrint = debugPrint;
      final logs = <String>[];
      debugPrint = (String? message, {int? wrapWidth}) {
        if (message != null) logs.add(message);
      };

      try {
        await tester.pumpWidget(
          ProviderScope(
            overrides: [
              ...i18nTestOverrides,
              windowModeProvider.overrideWith(
                () => WindowModeNotifier(initial: WindowMode.full),
              ),
            ],
            child: MaterialApp(
              home: WindowModeApplier(
                applyWindowMode: (_) async {
                  throw StateError('missing platform method');
                },
                child: const Text('board'),
              ),
            ),
          ),
        );
        await tester.pump();
      } finally {
        debugPrint = originalDebugPrint;
      }

      expect(find.text('board'), findsOneWidget);
      expect(tester.takeException(), isNull);
      expect(
        logs,
        contains(contains('upeg: applyWindowMode(WindowMode.full) failed')),
      );
    });
  });
}
