/// Widget tests for [UpegTheme.forAccent] + MaterialApp accent wiring.
///
/// K03: changing `tweaks.accent` must flip the live MaterialApp accent
/// (`ColorScheme.primary`) without a restart. The factory
/// `UpegTheme.forAccent(Accent, brightness:)` is the single place where
/// accent variants resolve to a token palette; consumer widgets just
/// `ref.watch(accentProvider)`.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/rust/api/tweaks.dart';
import 'package:upeg/src/state/accent.dart';
import 'package:upeg/src/state/tweaks_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';

import '../test_helpers/color_contrast.dart';

const TweaksDto _greenDark = TweaksDto(
  theme: 'Dark',
  accent: 'Green',
  showHoles: true,
  locale: 'En',
  localHttpHost: false,
);

const TweaksDto _pinkDark = TweaksDto(
  theme: 'Dark',
  accent: 'Pink',
  showHoles: true,
  locale: 'En',
  localHttpHost: false,
);

const TweaksDto _cyanLight = TweaksDto(
  theme: 'Light',
  accent: 'Cyan',
  showHoles: true,
  locale: 'En',
  localHttpHost: false,
);

/// Minimal harness that paints `MaterialApp.theme` from `forAccent` +
/// `accentProvider`. Mirrors the production `UpegApp.build` wiring but
/// stays small enough for assertion convenience.
class _AccentHarness extends ConsumerWidget {
  const _AccentHarness();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final accent = ref.watch(accentProvider);
    final theme = UpegTheme.forAccent(accent, brightness: Brightness.dark);
    final lightTheme = UpegTheme.forAccent(
      accent,
      brightness: Brightness.light,
    );
    return MaterialApp(
      key: const Key('harness-app'),
      theme: lightTheme,
      darkTheme: theme,
      themeMode: ThemeMode.dark,
      home: const Scaffold(body: SizedBox.shrink()),
    );
  }
}

void main() {
  group('UpegTheme.forAccent', () {
    test('forAccent는_Green_Dark에서_초록_palette_accent를_반환한다', () {
      final theme = UpegTheme.forAccent(
        Accent.green,
        brightness: Brightness.dark,
      );
      final tokens = theme.extension<UpegTokens>()!;
      // Green dark palette mirrors the pre-batch hardcoded accent
      // (oklch(0.78 0.16 145) ≈ #5BD16C).
      expect(tokens.accent, const Color(0xFF5BD16C));
    });

    test('forAccent는_Pink_Dark에서_핑크_palette_accent를_반환한다', () {
      final theme = UpegTheme.forAccent(
        Accent.pink,
        brightness: Brightness.dark,
      );
      final tokens = theme.extension<UpegTokens>()!;
      // Pink dark must be visually distinct from Green dark.
      expect(tokens.accent, isNot(equals(const Color(0xFF5BD16C))));
    });

    test('forAccent는_Cyan_Light에서_시안_palette_accent를_반환한다', () {
      final theme = UpegTheme.forAccent(
        Accent.cyan,
        brightness: Brightness.light,
      );
      final tokens = theme.extension<UpegTokens>()!;
      // Cyan light must differ from Green light (the previous hardcoded
      // value #2E8C44).
      expect(tokens.accent, isNot(equals(const Color(0xFF2E8C44))));
    });

    test('forAccent는_네_변형_모두_상호_고유한_accent_색을_반환한다', () {
      final greens = UpegTheme.forAccent(
        Accent.green,
        brightness: Brightness.dark,
      ).extension<UpegTokens>()!.accent;
      final amber = UpegTheme.forAccent(
        Accent.amber,
        brightness: Brightness.dark,
      ).extension<UpegTokens>()!.accent;
      final cyan = UpegTheme.forAccent(
        Accent.cyan,
        brightness: Brightness.dark,
      ).extension<UpegTokens>()!.accent;
      final pink = UpegTheme.forAccent(
        Accent.pink,
        brightness: Brightness.dark,
      ).extension<UpegTokens>()!.accent;
      final all = {greens, amber, cyan, pink};
      expect(all.length, 4, reason: 'each variant must be distinct');
    });

    test('라이트 테마의 모든 accent 전경은 실제 primary 배경과 4.5 이상 대비된다', () {
      for (final accent in Accent.values) {
        final scheme = UpegTheme.forAccent(
          accent,
          brightness: Brightness.light,
        ).colorScheme;

        expect(
          colorContrastRatio(scheme.onPrimary, scheme.primary),
          greaterThanOrEqualTo(minimumNormalTextContrastRatio),
          reason: '${accent.name} Run foreground must remain readable',
        );
      }
    });

    test('compact 선택 chip의 secondary 전경 배경 쌍은 4.5 이상 대비된다', () {
      for (final brightness in Brightness.values) {
        final scheme = UpegTheme.forAccent(
          Accent.green,
          brightness: brightness,
        ).colorScheme;

        expect(
          colorContrastRatio(scheme.onSecondary, scheme.secondary),
          greaterThanOrEqualTo(minimumNormalTextContrastRatio),
          reason: '${brightness.name} selected chip pair must remain readable',
        );
      }
    });
  });

  group('MaterialApp accent wiring', () {
    testWidgets('App은_tweaks_accent_Green일때_초록_primary를_사용한다', (tester) async {
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            tweaksLoaderProvider.overrideWith(
              (ref) =>
                  () => _greenDark,
            ),
          ],
          child: const _AccentHarness(),
        ),
      );
      await tester.pumpAndSettle();

      final ctx = tester.element(find.byType(Scaffold).first);
      final primary = Theme.of(ctx).colorScheme.primary;
      expect(primary, const Color(0xFF5BD16C));
    });

    testWidgets('App은_tweaks_accent_Pink일때_초록과_다른_primary를_사용한다', (
      tester,
    ) async {
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            tweaksLoaderProvider.overrideWith(
              (ref) =>
                  () => _pinkDark,
            ),
          ],
          child: const _AccentHarness(),
        ),
      );
      await tester.pumpAndSettle();

      final ctx = tester.element(find.byType(Scaffold).first);
      final primary = Theme.of(ctx).colorScheme.primary;
      expect(primary, isNot(equals(const Color(0xFF5BD16C))));
    });

    testWidgets('App은_tweaks_accent_save_후_primary가_갱신된다', (tester) async {
      final container = ProviderContainer(
        overrides: [
          tweaksLoaderProvider.overrideWith(
            (ref) =>
                () => _greenDark,
          ),
          tweaksSaverProvider.overrideWith((ref) => (_) {}),
        ],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const _AccentHarness(),
        ),
      );
      await tester.pumpAndSettle();

      final ctx1 = tester.element(find.byType(Scaffold).first);
      expect(Theme.of(ctx1).colorScheme.primary, const Color(0xFF5BD16C));

      await container.read(tweaksProvider.notifier).save(_cyanLight);
      await tester.pumpAndSettle();

      final ctx2 = tester.element(find.byType(Scaffold).first);
      expect(
        Theme.of(ctx2).colorScheme.primary,
        isNot(equals(const Color(0xFF5BD16C))),
      );
    });
  });
}
