/// Widget tests for [ProviderNotConfiguredBody].
///
/// The dead-end fix contract: the "needs setup" state must point at the
/// concrete CLI command (`upeg credential add <name>`), offer a working
/// copy button, and render all copy through `t()` so a locale flip
/// re-renders live. FRB calls are swapped via [i18nTranslateOverride] /
/// tweaks loader overrides so no native dylib is required.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/rust/api/tweaks.dart';
import 'package:upeg/src/state/tweaks_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';
import 'package:upeg/src/widgets/provider_not_configured_body.dart';
import '../test_helpers/i18n_test_catalog.dart';

class _RecordingClipboardWriter extends ClipboardWriter {
  final List<String> writes = <String>[];

  @override
  Future<void> write(String text) async {
    writes.add(text);
  }
}

TweaksDto _tweaks(String locale) => TweaksDto(
  theme: 'Dark',
  accent: 'Green',
  showHoles: true,
  locale: locale,
  localHttpHost: false,
);

Widget _harness({
  String? credentialName,
  required ClipboardWriter writer,
  List<Override> overrides = const <Override>[],
}) {
  return ProviderScope(
    overrides: [
      ...i18nTestOverrides,
      clipboardWriterProvider.overrideWithValue(writer),
      tweaksLoaderProvider.overrideWith(
        (ref) =>
            () => _tweaks('En'),
      ),
      ...overrides,
    ],
    child: MaterialApp(
      theme: UpegTheme.darkTheme(),
      home: Scaffold(
        body: ProviderNotConfiguredBody(credentialName: credentialName),
      ),
    ),
  );
}

void main() {
  group('ProviderNotConfiguredBody', () {
    testWidgets(
      'the_needs_setup_state_renders_the_cli_command_and_copy_button',
      (tester) async {
        final writer = _RecordingClipboardWriter();
        await tester.pumpWidget(
          _harness(credentialName: 'demo_api_key', writer: writer),
        );
        await tester.pumpAndSettle();

        expect(find.text('needs setup'), findsOneWidget);
        expect(
          find.text('Set up a credential in the terminal:'),
          findsOneWidget,
        );
        expect(find.text(credentialAddCommand('demo_api_key')), findsOneWidget);
        expect(find.byType(CopyToClipboardButton), findsOneWidget);
      },
    );

    testWidgets(
      'without_a_credential_name_the_command_renders_with_the_placeholder',
      (tester) async {
        final writer = _RecordingClipboardWriter();
        await tester.pumpWidget(_harness(writer: writer));
        await tester.pumpAndSettle();

        final command = credentialAddCommand();
        expect(command, contains(credentialNamePlaceholder));
        expect(find.text(command), findsOneWidget);
      },
    );

    testWidgets(
      'tapping_the_copy_button_copies_the_cli_command_to_the_clipboard',
      (tester) async {
        final writer = _RecordingClipboardWriter();
        await tester.pumpWidget(
          _harness(credentialName: 'demo_api_key', writer: writer),
        );
        await tester.pumpAndSettle();

        await tester.tap(find.byType(CopyToClipboardButton));
        await tester.pumpAndSettle();

        expect(writer.writes, [credentialAddCommand('demo_api_key')]);
      },
    );

    testWidgets(
      'switching_the_locale_renders_the_needs_setup_label_in_korean',
      (tester) async {
        final writer = _RecordingClipboardWriter();
        final container = ProviderContainer(
          overrides: [
            ...i18nTestOverrides,
            clipboardWriterProvider.overrideWithValue(writer),
            tweaksLoaderProvider.overrideWith(
              (ref) =>
                  () => _tweaks('En'),
            ),
            tweaksSaverProvider.overrideWith((ref) => (TweaksDto next) {}),
          ],
        );
        addTearDown(container.dispose);

        await tester.pumpWidget(
          UncontrolledProviderScope(
            container: container,
            child: MaterialApp(
              theme: UpegTheme.darkTheme(),
              home: const Scaffold(body: ProviderNotConfiguredBody()),
            ),
          ),
        );
        await tester.pumpAndSettle();

        expect(find.text('needs setup'), findsOneWidget);
        expect(find.text('설정 필요'), findsNothing);

        await container.read(tweaksProvider.notifier).save(_tweaks('Ko'));
        await tester.pumpAndSettle();

        expect(find.text('needs setup'), findsNothing);
        expect(find.text('설정 필요'), findsOneWidget);
        expect(find.text('터미널에서 credential을 설정하세요:'), findsOneWidget);
        // CLI syntax stays locale-independent.
        expect(find.text(credentialAddCommand()), findsOneWidget);
      },
    );

    testWidgets(
      'switching_the_locale_renders_the_needs_setup_label_and_hint_in_korean',
      (tester) async {
        final writer = _RecordingClipboardWriter();
        final container = ProviderContainer(
          overrides: [
            ...i18nTestOverrides,
            clipboardWriterProvider.overrideWithValue(writer),
            tweaksLoaderProvider.overrideWith(
              (ref) =>
                  () => _tweaks('En'),
            ),
            tweaksSaverProvider.overrideWith((ref) => (TweaksDto _) {}),
          ],
        );
        addTearDown(container.dispose);
        await container.read(tweaksProvider.future);

        await tester.pumpWidget(
          UncontrolledProviderScope(
            container: container,
            child: const MaterialApp(
              home: Scaffold(body: ProviderNotConfiguredBody()),
            ),
          ),
        );
        await tester.pumpAndSettle();

        expect(
          find.text(i18nEn(providerNotConfiguredLabelKey)),
          findsOneWidget,
        );
        expect(find.text(i18nEn(providerNotConfiguredHintKey)), findsOneWidget);

        await container.read(tweaksProvider.notifier).save(_tweaks('Ko'));
        await tester.pumpAndSettle();

        expect(
          find.text(i18nKo(providerNotConfiguredLabelKey)),
          findsOneWidget,
        );
        expect(find.text(i18nKo(providerNotConfiguredHintKey)), findsOneWidget);
        expect(find.text(i18nEn(providerNotConfiguredLabelKey)), findsNothing);
      },
    );
  });
}
