/// Widget tests for [showSettingsOverlay].
///
/// Settings modal shell contract:
/// title strip + a close button that dismisses the dialog. The
/// underlying [TweaksForm] is tested separately — we only care that the
/// modal shell renders + the close button pops the route.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/rust/api/i18n.dart' show LocaleDto;
import 'package:upeg/src/rust/api/pause.dart' as pause_frb;
import 'package:upeg/src/rust/api/status.dart' as status_frb;
import 'package:upeg/src/rust/api/tweaks.dart';
import 'package:upeg/src/state/status_provider.dart';
import 'package:upeg/src/state/diagnostics_provider.dart';
import 'package:upeg/src/state/project_context_provider.dart';
import 'package:upeg/src/state/tweaks_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/settings_overlay.dart';
import 'package:upeg/src/widgets/tweaks_form.dart';

import '../test_helpers/fake_keyboard_resolver.dart';

class _FakeStatusNotifier extends StatusNotifier {
  @override
  status_frb.StatusSnapshotDto build() {
    return const status_frb.StatusSnapshotDto(
      network: status_frb.NetworkStatusDto(
        reachability: status_frb.NetworkReachabilityDto.offline,
        label: 'offline',
      ),
      paused: pause_frb.PausedStateDto.running,
      mcpImportCount: 0,
      mcpImportPhase: status_frb.McpImportPhaseDto.notStarted,
      buildVersion: 'test',
    );
  }
}

class _FakeProjectApi implements ProjectContextApi {
  const _FakeProjectApi();

  @override
  Future<ProjectDefinition?> current() async => null;

  @override
  Future<ProjectDefinition> validateRoot(String root) =>
      throw UnimplementedError();

  @override
  Future<ProjectActivation> activate(String root) => throw UnimplementedError();

  @override
  Future<ProjectActivation> setToolChoice({
    required String root,
    required String toolId,
    required ProjectToolChoice choice,
  }) => throw UnimplementedError();

  @override
  Future<void> close() => throw UnimplementedError();
}

class _FakeDiagnosticsApi implements DiagnosticsApi {
  final List<DiagnosticSummary> items = [];

  @override
  Future<List<DiagnosticSummary>> list({required int limit}) async =>
      items.take(limit).toList();

  @override
  Future<DiagnosticReport?> show(String id) => throw UnimplementedError();

  @override
  Future<String> export(String id, {required bool debug}) =>
      throw UnimplementedError();
}

const TweaksDto _defaultDto = TweaksDto(
  theme: 'Dark',
  accent: 'Green',
  showHoles: true,
  locale: 'En',
  localHttpHost: false,
);

const Map<String, Map<LocaleDto, String>> _fakeCatalog = {
  'settings.title': {LocaleDto.en: 'settings', LocaleDto.ko: '설정'},
  'settings.close': {LocaleDto.en: 'close', LocaleDto.ko: '닫기'},
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
  // BackupSection mounts inside TweaksForm — the buttons must resolve
  // to short labels or the key-as-marker fallback overflows the row.
  'settings.backup.export': {LocaleDto.en: 'export', LocaleDto.ko: '보내기'},
  'settings.backup.import': {LocaleDto.en: 'import', LocaleDto.ko: '가져오기'},
};

String _fakeTranslate(String key, LocaleDto locale) {
  return _fakeCatalog[key]?[locale] ?? key;
}

String _fakeTranslateArgs(
  String key,
  LocaleDto locale,
  List<String> argKeys,
  List<String> argVals,
) {
  // The Services section's loaded-count uses `{n}`; the parity test
  // doesn't care about the exact substitution, so a trivial template
  // returns the key (no widget assertion depends on the rendered text
  // here — only on the toggle / buttons).
  var out = _fakeCatalog[key]?[locale] ?? key;
  for (var i = 0; i < argKeys.length; i++) {
    out = out.replaceAll('{${argKeys[i]}}', argVals[i]);
  }
  return out;
}

Widget _hostHarness({
  TweaksDto initial = _defaultDto,
  void Function(TweaksDto value)? onSave,
  DiagnosticsApi? diagnosticsApi,
}) {
  return ProviderScope(
    overrides: [
      tweaksLoaderProvider.overrideWith(
        (ref) =>
            () => initial,
      ),
      tweaksSaverProvider.overrideWith(
        (ref) =>
            (TweaksDto value) => onSave?.call(value),
      ),
      supportedLocalesProvider.overrideWith(
        (ref) =>
            () => const <String>['En', 'Ko'],
      ),
      supportedThemesProvider.overrideWith(
        (ref) =>
            () => const <String>['Dark', 'Light'],
      ),
      supportedAccentsProvider.overrideWith(
        (ref) =>
            () => const <String>['Green', 'Amber'],
      ),
      i18nTranslateOverride.overrideWithValue(_fakeTranslate),
      i18nTranslateArgsOverride.overrideWithValue(_fakeTranslateArgs),
      fakeKeyboardResolverOverride,
      // The status bar reads this — the FRB-defaulted version would
      // crash because the dylib never loads in widget tests.
      statusSnapshotProvider.overrideWith(() => _FakeStatusNotifier()),
      projectContextApiProvider.overrideWithValue(const _FakeProjectApi()),
      diagnosticsApiProvider.overrideWithValue(
        diagnosticsApi ?? _FakeDiagnosticsApi(),
      ),
    ],
    child: MaterialApp(
      theme: UpegTheme.darkTheme(),
      home: Builder(
        builder: (context) => Scaffold(
          body: Center(
            child: TextButton(
              key: const Key('open-settings-btn'),
              onPressed: () => showSettingsOverlay(context),
              child: const Text('open'),
            ),
          ),
        ),
      ),
    ),
  );
}

void main() {
  group('showSettingsOverlay', () {
    testWidgets('settingsOverlay_renders_the_title_and_close_button', (
      tester,
    ) async {
      await tester.pumpWidget(_hostHarness());
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('open-settings-btn')));
      await tester.pumpAndSettle();

      expect(find.text('SETTINGS'), findsOneWidget);
      expect(find.byKey(const Key('settings-close-btn')), findsOneWidget);
    });

    // Equivalent coverage migrated from the retired `settings_page_test`
    // (the orphaned `SettingsPage` was deleted): the production settings
    // surface is this overlay, so the form-body render assertion lives
    // here now.
    testWidgets('settingsOverlay_renders_the_TweaksForm', (tester) async {
      await tester.pumpWidget(_hostHarness());
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('open-settings-btn')));
      await tester.pumpAndSettle();

      expect(find.byType(TweaksForm), findsOneWidget);
    });

    testWidgets('SettingsOverlay_shows_a_Korean_title_when_locale_is_Ko', (
      tester,
    ) async {
      const koDto = TweaksDto(
        theme: 'Dark',
        accent: 'Green',
        showHoles: true,
        locale: 'Ko',
        localHttpHost: false,
      );
      await tester.pumpWidget(_hostHarness(initial: koDto));
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('open-settings-btn')));
      await tester.pumpAndSettle();

      // EN catalog 'settings' would have rendered 'SETTINGS' upper-cased;
      // KO catalog returns '설정' (Hangul has no case form).
      expect(find.text('SETTINGS'), findsNothing);
      expect(find.text('설정'), findsOneWidget);
      expect(find.text('닫기'), findsOneWidget);
    });

    testWidgets(
      'settingsOverlay_close_button_dismisses_the_modal_without_a_transition',
      (tester) async {
        await tester.pumpWidget(_hostHarness());
        await tester.pumpAndSettle();

        await tester.tap(find.byKey(const Key('open-settings-btn')));
        await tester.pump();
        expect(find.text('SETTINGS'), findsOneWidget);

        await tester.tap(find.byKey(const Key('settings-close-btn')));
        await tester.pump();

        expect(find.text('SETTINGS'), findsNothing);
      },
    );

    testWidgets('SettingsOverlay_dismisses_the_modal_with_the_q_key', (
      tester,
    ) async {
      await tester.pumpWidget(_hostHarness());
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('open-settings-btn')));
      await tester.pumpAndSettle();
      expect(find.text('SETTINGS'), findsOneWidget);

      await tester.sendKeyEvent(LogicalKeyboardKey.keyQ);
      await tester.pumpAndSettle();

      expect(find.text('SETTINGS'), findsNothing);
    });

    testWidgets('SettingsOverlay_consumes_path_slash_in_the_project_field', (
      tester,
    ) async {
      await tester.pumpWidget(_hostHarness());
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('open-settings-btn')));
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('project-context-root-input')));
      await tester.sendKeyEvent(LogicalKeyboardKey.slash);
      await tester.pumpAndSettle();

      // The fake settings resolver maps `/` to Close. A focused editable
      // must consume it, as absolute project paths begin with this character.
      expect(find.byKey(const Key('settings-close-btn')), findsOneWidget);
      await tester.enterText(
        find.byKey(const Key('project-context-root-input')),
        '/tmp/project',
      );
      final input = tester.widget<TextField>(
        find.byKey(const Key('project-context-root-input')),
      );
      expect(input.controller?.text, '/tmp/project');
    });

    testWidgets('SettingsOverlay_refreshes_recent_reports_when_reopened', (
      tester,
    ) async {
      final api = _FakeDiagnosticsApi();
      await tester.pumpWidget(_hostHarness(diagnosticsApi: api));
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('open-settings-btn')));
      await tester.pumpAndSettle();
      expect(find.byKey(const Key('diagnostic-d-2')), findsNothing);

      await tester.tap(find.byKey(const Key('settings-close-btn')));
      await tester.pumpAndSettle();
      api.items.add(
        const DiagnosticSummary(
          id: 'd-2',
          runId: 'run-2',
          occurredAtMs: 2,
          source: 'app',
          errorCode: 'APP_FAILED',
          errorMessage: 'after first open',
          status: 'failed',
        ),
      );

      await tester.tap(find.byKey(const Key('open-settings-btn')));
      await tester.pumpAndSettle();
      expect(find.byKey(const Key('diagnostic-d-2')), findsOneWidget);
    });

    testWidgets('SettingsOverlay_moves_focus_with_j_and_k', (tester) async {
      await tester.pumpWidget(_hostHarness());
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('open-settings-btn')));
      await tester.pumpAndSettle();

      final themeContext = tester.element(
        find.byKey(const Key('tweaks-theme-radio-focus')),
      );
      final accentContext = tester.element(
        find.byKey(const Key('tweaks-accent-radio-focus')),
      );
      Focus.of(themeContext).requestFocus();
      await tester.pump();
      expect(Focus.of(themeContext).hasPrimaryFocus, isTrue);

      await tester.sendKeyEvent(LogicalKeyboardKey.keyJ);
      await tester.pump();

      expect(Focus.of(accentContext).hasPrimaryFocus, isTrue);

      await tester.sendKeyEvent(LogicalKeyboardKey.keyK);
      await tester.pump();

      expect(Focus.of(themeContext).hasPrimaryFocus, isTrue);
    });

    testWidgets('SettingsOverlay_changes_the_focused_setting_with_l_and_h', (
      tester,
    ) async {
      final saved = <TweaksDto>[];
      await tester.pumpWidget(_hostHarness(onSave: saved.add));
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('open-settings-btn')));
      await tester.pumpAndSettle();

      final themeContext = tester.element(
        find.byKey(const Key('tweaks-theme-radio-focus')),
      );
      Focus.of(themeContext).requestFocus();
      await tester.pump();

      await tester.sendKeyEvent(LogicalKeyboardKey.keyL);
      await tester.pumpAndSettle();

      expect(saved.last.theme, 'Light');
      expect(find.text('Light'), findsOneWidget);

      await tester.sendKeyEvent(LogicalKeyboardKey.keyH);
      await tester.pumpAndSettle();

      expect(saved.last.theme, 'Dark');
      expect(find.text('Dark'), findsOneWidget);
    });
  });
}
