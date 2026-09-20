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
    test('maps_Light_to_ThemeMode_light', () {
      expect(themeModeFromTweakValue('Light'), ThemeMode.light);
    });

    test('maps_Dark_to_ThemeMode_dark', () {
      expect(themeModeFromTweakValue('Dark'), ThemeMode.dark);
    });

    test('maps_System_to_ThemeMode_system', () {
      expect(themeModeFromTweakValue('System'), ThemeMode.system);
    });

    test('unknown_values_fall_back_to_default_dark_mode', () {
      expect(themeModeFromTweakValue('Magenta'), kDefaultThemeMode);
      expect(kDefaultThemeMode, ThemeMode.dark);
    });
  });

  group('themeModeProvider', () {
    testWidgets('MaterialApp_themeMode_is_light_for_Light_tweaks', (
      tester,
    ) async {
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

    testWidgets('MaterialApp_themeMode_is_dark_for_Dark_tweaks', (
      tester,
    ) async {
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

    testWidgets('themeModeProvider_reemits_new_mode_after_save', (
      tester,
    ) async {
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
