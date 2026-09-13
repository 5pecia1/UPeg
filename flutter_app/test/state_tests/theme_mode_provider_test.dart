/// Unit tests for [themeModeProvider].
///
/// `tweaksLoaderProvider` is overridden so the FRB call is never
/// invoked. We drive `MaterialApp.themeMode` through a real widget
/// build to assert that flipping the Tweaks value re-renders with
/// the new mode (no restart required).
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/rust/api/tweaks.dart';
import 'package:upeg/src/state/theme_mode_provider.dart';
import 'package:upeg/src/state/tweaks_provider.dart';

const TweaksDto _lightTweaks = TweaksDto(
  theme: 'Light',
  accent: 'Green',
  showHoles: true,
  locale: 'En',
  localHttpHost: false,
);

const TweaksDto _darkTweaks = TweaksDto(
  theme: 'Dark',
  accent: 'Green',
  showHoles: true,
  locale: 'En',
  localHttpHost: false,
);

class _ThemeModeHarness extends ConsumerWidget {
  const _ThemeModeHarness();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final mode = ref.watch(themeModeProvider);
    return MaterialApp(
      key: const Key('harness-app'),
      themeMode: mode,
      theme: ThemeData.light(),
      darkTheme: ThemeData.dark(),
      home: const Scaffold(body: SizedBox.shrink()),
    );
  }
}

MaterialApp _findApp(WidgetTester tester) =>
    tester.widget<MaterialApp>(find.byKey(const Key('harness-app')));

void main() {
  group('themeModeFromTweakValue', () {
    test('Light_을_ThemeMode_light로_매핑한다', () {
      expect(themeModeFromTweakValue('Light'), ThemeMode.light);
    });

    test('Dark_를_ThemeMode_dark로_매핑한다', () {
      expect(themeModeFromTweakValue('Dark'), ThemeMode.dark);
    });

    test('System_을_ThemeMode_system으로_매핑한다', () {
      expect(themeModeFromTweakValue('System'), ThemeMode.system);
    });

    test('알수없는_값은_기본_dark_모드로_떨어진다', () {
      expect(themeModeFromTweakValue('Magenta'), kDefaultThemeMode);
      expect(kDefaultThemeMode, ThemeMode.dark);
    });
  });

  group('themeModeProvider', () {
    testWidgets('Light_트윅스에서_MaterialApp_themeMode가_light다', (tester) async {
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            tweaksLoaderProvider.overrideWith(
              (ref) =>
                  () => _lightTweaks,
            ),
          ],
          child: const _ThemeModeHarness(),
        ),
      );
      await tester.pumpAndSettle();
      expect(_findApp(tester).themeMode, ThemeMode.light);
    });

    testWidgets('Dark_트윅스에서_MaterialApp_themeMode가_dark다', (tester) async {
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            tweaksLoaderProvider.overrideWith(
              (ref) =>
                  () => _darkTweaks,
            ),
          ],
          child: const _ThemeModeHarness(),
        ),
      );
      await tester.pumpAndSettle();
      expect(_findApp(tester).themeMode, ThemeMode.dark);
    });

    testWidgets('save_후_themeModeProvider가_새_모드로_재발행한다', (tester) async {
      final container = ProviderContainer(
        overrides: [
          tweaksLoaderProvider.overrideWith(
            (ref) =>
                () => _lightTweaks,
          ),
          tweaksSaverProvider.overrideWith((ref) => (_) {}),
        ],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const _ThemeModeHarness(),
        ),
      );
      await tester.pumpAndSettle();
      expect(_findApp(tester).themeMode, ThemeMode.light);

      await container.read(tweaksProvider.notifier).save(_darkTweaks);
      await tester.pumpAndSettle();

      expect(_findApp(tester).themeMode, ThemeMode.dark);
    });
  });
}
