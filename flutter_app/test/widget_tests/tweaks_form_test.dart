/// Widget tests for [TweaksForm].
///
/// Layout contract (inherited from the retired Dioxus settings form):
/// theme / accent / locale render as radio-style segmented buttons, and
/// changes apply immediately through `tweaksProvider` (no separate save
/// button). The Rust FRB call is swapped out via the loader/saver
/// providers so the dylib is never loaded in test mode.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/rust/api/i18n.dart' show LocaleDto;
import 'package:upeg/src/rust/api/pause.dart' as services_frb;
import 'package:upeg/src/rust/api/status.dart' as status_frb;
import 'package:upeg/src/rust/api/tweaks.dart';
import 'package:upeg/src/state/status_provider.dart';
import 'package:upeg/src/state/tweaks_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/tweaks_form.dart';

class _FakeStatusNotifier extends StatusNotifier {
  @override
  status_frb.StatusSnapshotDto build() {
    return const status_frb.StatusSnapshotDto(
      network: status_frb.NetworkStatusDto(
        reachability: status_frb.NetworkReachabilityDto.offline,
        label: 'offline',
      ),
      paused: services_frb.PausedStateDto.running,
      mcpImportCount: 0,
      mcpImportPhase: status_frb.McpImportPhaseDto.notStarted,
      buildVersion: 'test',
    );
  }
}

String _fakeTranslateArgs(
  String key,
  LocaleDto locale,
  List<String> argKeys,
  List<String> argVals,
) {
  var out = _fakeCatalog[key]?[locale] ?? key;
  for (var i = 0; i < argKeys.length; i++) {
    out = out.replaceAll('{${argKeys[i]}}', argVals[i]);
  }
  return out;
}

const TweaksDto _defaultDto = TweaksDto(
  theme: 'Light',
  accent: 'Green',
  showHoles: true,
  locale: 'En',
  localHttpHost: false,
);

const Map<String, Map<LocaleDto, String>> _fakeCatalog = {
  'settings.section.theme': {LocaleDto.en: 'theme', LocaleDto.ko: '테마'},
  'settings.section.layout': {LocaleDto.en: 'layout', LocaleDto.ko: '레이아웃'},
  'settings.section.language': {LocaleDto.en: 'language', LocaleDto.ko: '언어'},
  'settings.radio.mode': {LocaleDto.en: 'mode', LocaleDto.ko: '모드'},
  'settings.radio.accent': {LocaleDto.en: 'accent', LocaleDto.ko: '강조색'},
  'settings.radio.locale': {LocaleDto.en: 'locale', LocaleDto.ko: '언어'},
  'settings.toggle.peg_holes': {
    LocaleDto.en: 'peg holes',
    LocaleDto.ko: '페그 구멍',
  },
  'settings.toggle.on': {LocaleDto.en: 'on', LocaleDto.ko: '켬'},
  'settings.toggle.off': {LocaleDto.en: 'off', LocaleDto.ko: '끔'},
  'settings.section.host': {LocaleDto.en: 'host', LocaleDto.ko: '호스트'},
  'settings.toggle.local_http_host': {
    LocaleDto.en: 'local HTTP host',
    LocaleDto.ko: '로컬 HTTP host',
  },
  'settings.toggle.local_http_host_help': {
    LocaleDto.en: 'serve the REST/MCP host from this app; restart to apply',
    LocaleDto.ko: '이 앱이 REST/MCP host가 된다. 적용하려면 재시작한다',
  },
};

String _fakeTranslate(String key, LocaleDto locale) {
  return _fakeCatalog[key]?[locale] ?? key;
}

Widget _harness({
  TweaksDto initial = _defaultDto,
  void Function(TweaksDto)? onSave,
  List<String> locales = const <String>['En', 'Ko'],
  List<String> themes = const <String>['Light', 'Dark'],
  List<String> accents = const <String>['Green', 'Amber', 'Cyan', 'Pink'],
}) {
  return ProviderScope(
    overrides: [
      tweaksLoaderProvider.overrideWith(
        (ref) =>
            () => initial,
      ),
      tweaksSaverProvider.overrideWith((ref) => onSave ?? (TweaksDto _) {}),
      supportedLocalesProvider.overrideWith(
        (ref) =>
            () => locales,
      ),
      supportedThemesProvider.overrideWith(
        (ref) =>
            () => themes,
      ),
      supportedAccentsProvider.overrideWith(
        (ref) =>
            () => accents,
      ),
      i18nTranslateOverride.overrideWithValue(_fakeTranslate),
      i18nTranslateArgsOverride.overrideWithValue(_fakeTranslateArgs),
      statusSnapshotProvider.overrideWith(() => _FakeStatusNotifier()),
    ],
    child: MaterialApp(
      theme: UpegTheme.darkTheme(),
      // Production hosts TweaksForm inside a SingleChildScrollView (see
      // settings_overlay.dart); mirror that so the taller section stack
      // (theme…host + host attach) scrolls instead of overflowing the
      // bare 800×600 test viewport.
      home: const Scaffold(body: SingleChildScrollView(child: TweaksForm())),
    ),
  );
}

void main() {
  group('TweaksForm', () {
    testWidgets('TweaksForm_은_locale과_theme_라디오_옵션을_렌더한다', (tester) async {
      await tester.pumpWidget(_harness());
      await tester.pumpAndSettle();

      // Section headers
      expect(find.text('THEME'), findsOneWidget);
      expect(find.text('LAYOUT'), findsOneWidget);
      expect(find.text('LANGUAGE'), findsOneWidget);

      // Radio chips for each enum-shaped field
      expect(find.byKey(const Key('tweaks-theme-radio-Light')), findsOneWidget);
      expect(find.byKey(const Key('tweaks-theme-radio-Dark')), findsOneWidget);
      expect(
        find.byKey(const Key('tweaks-accent-radio-Green')),
        findsOneWidget,
      );
      expect(find.byKey(const Key('tweaks-locale-radio-Ko')), findsOneWidget);
    });

    testWidgets('라디오_탭은_saveTweaks를_즉시_호출한다', (tester) async {
      TweaksDto? saved;
      await tester.pumpWidget(_harness(onSave: (next) => saved = next));
      await tester.pumpAndSettle();

      // Tap the Ko locale chip — should save immediately (no Save button).
      await tester.tap(find.byKey(const Key('tweaks-locale-radio-Ko')));
      await tester.pumpAndSettle();

      expect(saved, isNotNull);
      expect(saved!.locale, 'Ko');
      expect(saved!.theme, 'Light');
    });

    testWidgets('TweaksForm은_locale_Ko로_바뀌면_LANGUAGE_섹션_헤더가_언어로_표시된다', (
      tester,
    ) async {
      await tester.pumpWidget(_harness());
      await tester.pumpAndSettle();

      // En locale: section headers render the catalog English values
      // upper-cased. "language" → "LANGUAGE".
      expect(find.text('LANGUAGE'), findsOneWidget);
      expect(find.textContaining('언어'), findsNothing);

      // Flip locale → re-render via t() helper. The "Ko" chip key is
      // generated from the serde variant name (see tweaks_form.dart).
      await tester.tap(find.byKey(const Key('tweaks-locale-radio-Ko')));
      await tester.pumpAndSettle();

      expect(find.text('LANGUAGE'), findsNothing);
      // KO catalog: "settings.section.language" → "언어" (Hangul is
      // case-less so .toUpperCase() leaves it unchanged).
      expect(find.textContaining('언어'), findsWidgets);
    });

    testWidgets('peg_holes_토글은_즉시_저장된다', (tester) async {
      TweaksDto? saved;
      await tester.pumpWidget(_harness(onSave: (next) => saved = next));
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('tweaks-show-holes-toggle')));
      await tester.pumpAndSettle();

      expect(saved, isNotNull);
      expect(saved!.showHoles, false);
    });

    testWidgets('로컬_http_host_토글은_즉시_저장된다', (tester) async {
      // service/source control plane을 대체한 단일 스위치. 상태 폴링이
      // 없으므로 저장된 설정 자체가 유일한 상태다.
      TweaksDto? saved;
      await tester.pumpWidget(_harness(onSave: (next) => saved = next));
      await tester.pumpAndSettle();

      await tester.scrollUntilVisible(
        find.byKey(kLocalHttpHostToggleKey),
        200,
        scrollable: find.byType(Scrollable).first,
      );
      await tester.tap(find.byKey(kLocalHttpHostToggleKey));
      await tester.pumpAndSettle();

      expect(saved, isNotNull);
      expect(
        saved!.localHttpHost,
        true,
        reason: '기본값 OFF에서 한 번 누르면 켜져야 한다 (FR-16 명시적 활성화)',
      );
    });

    testWidgets('로컬_http_host_행은_정적_설명을_함께_보여준다', (tester) async {
      await tester.pumpWidget(_harness());
      await tester.pumpAndSettle();

      await tester.scrollUntilVisible(
        find.byKey(kLocalHttpHostHelpKey),
        200,
        scrollable: find.byType(Scrollable).first,
      );

      expect(find.byKey(kLocalHttpHostHelpKey), findsOneWidget);
    });
  });
}
