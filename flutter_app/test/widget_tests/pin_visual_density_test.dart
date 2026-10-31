/// Pin chrome after the visual-density cleanup: the footer keeps its row but
/// no longer repeats the kind and invoker, the move handle is muted, the
/// title is drawn once, and large text does not overflow the chrome.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tweaks.dart';
import 'package:upeg/src/rust/api/tools.dart' as rust_tools;
import 'package:upeg/src/state/tweaks_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/pin.dart';
import 'package:upeg/src/widgets/pin/pin_footer.dart';
import 'package:upeg/src/widgets/pin/pin_header.dart';
import 'package:upeg/src/widgets/pin/pin_move_handle.dart';

import '../test_helpers/i18n_test_catalog.dart';

const _placement = PlacementDto(
  toolId: 'num.hex_to_decimal',
  pinId: 'num.hex_to_decimal',
  x: 0,
  y: 0,
  w: 1,
  h: 1,
);

const _gasResult = rust_tools.CanonicalToolResult(
  ok: true,
  primaryOutputId: 'gwei',
  outputs: [
    rust_tools.CanonicalOutputEntry(
      id: 'gwei',
      label: 'g',
      kind: 'number',
      value: rust_tools.CanonicalOutputValue.number(value: 32.5),
    ),
  ],
);

/// Pumps [pin] in a production-sized (168x150) cell.
Future<void> _pumpPin(
  WidgetTester tester,
  Pin pin, {
  ThemeData? theme,
  double textScale = 1,
  PinHandleStyleDto handleStyle = PinHandleStyleDto.peg,
}) {
  return tester.pumpWidget(
    ProviderScope(
      overrides: [
        ...i18nTestOverrides,
        tweaksLoaderProvider.overrideWith(
          (ref) =>
              () => TweaksDto(
                theme: 'Dark',
                accent: 'Green',
                showHoles: true,
                locale: 'En',
                localHttpHost: false,
                pinHandleStyle: handleStyle,
              ),
        ),
        tweaksSaverProvider.overrideWith((ref) => (_) {}),
      ],
      child: MaterialApp(
        theme: theme ?? UpegTheme.darkTheme(),
        builder: (context, child) => MediaQuery(
          data: MediaQuery.of(
            context,
          ).copyWith(textScaler: TextScaler.linear(textScale)),
          child: child!,
        ),
        home: Scaffold(body: SizedBox(width: 168, height: 150, child: pin)),
      ),
    ),
  );
}

Pin _pin({
  UpegPinKind? kind,
  bool running = false,
  bool stale = false,
  bool restored = false,
  bool showMoveHandle = false,
}) => Pin(
  placement: _placement,
  toolLabel: 'hex → dec',
  toolDescription: 'Converts a hex string.',
  pinKind: kind,
  outputResult: restored ? _gasResult : null,
  restoredAt: restored
      ? DateTime.now().subtract(const Duration(minutes: 5))
      : null,
  running: running,
  stale: stale,
  showMoveHandle: showMoveHandle,
);

double _footerHeight(WidgetTester tester) =>
    tester.getSize(find.byType(PinFooter)).height;

void main() {
  group('Pin footer', () {
    testWidgets('an_idle_footer_keeps_its_row_but_draws_no_text', (
      tester,
    ) async {
      for (final kind in UpegPinKind.values) {
        await _pumpPin(tester, _pin(kind: kind));

        expect(find.byType(PinFooter), findsOneWidget, reason: kind.name);
        expect(
          find.descendant(
            of: find.byType(PinFooter),
            matching: find.byType(Text),
          ),
          findsNothing,
          reason: '${kind.name} footer must not repeat the kind or invoker',
        );
        expect(find.text(kind.label), findsNothing);
      }
    });

    testWidgets('every_status_keeps_the_footer_at_the_idle_height', (
      tester,
    ) async {
      final statuses = <String, Pin>{
        'running': _pin(kind: UpegPinKind.inline, running: true),
        'stale': _pin(kind: UpegPinKind.inline, stale: true),
        'restored': _pin(kind: UpegPinKind.inline, restored: true),
        'all': _pin(
          kind: UpegPinKind.inline,
          running: true,
          stale: true,
          restored: true,
        ),
      };

      // Under large text the restored badge outgrows an unscaled minimum
      // height, so every scale is checked, not just the default one.
      for (final scale in <double>[1, 1.5, 2]) {
        await _pumpPin(
          tester,
          _pin(kind: UpegPinKind.inline),
          textScale: scale,
        );
        final idle = _footerHeight(tester);
        expect(idle, greaterThan(0));

        for (final status in statuses.entries) {
          await _pumpPin(tester, status.value, textScale: scale);

          // An Inline pin runs on every keystroke; a status flipping on must
          // not reflow the body.
          expect(
            _footerHeight(tester),
            idle,
            reason: '${status.key} at text scale $scale',
          );
        }
      }
    });

    testWidgets('running_stale_and_restored_markers_still_render', (
      tester,
    ) async {
      await _pumpPin(
        tester,
        _pin(
          kind: UpegPinKind.live,
          running: true,
          stale: true,
          restored: true,
        ),
      );

      expect(find.byKey(const Key('pin-running-indicator')), findsOneWidget);
      expect(find.byKey(const Key('pin-stale-dot')), findsOneWidget);
      expect(find.byKey(pinRestoredBadgeKey), findsOneWidget);
    });

    testWidgets('large_text_does_not_overflow_the_pin_chrome', (tester) async {
      for (final scale in <double>[1.5, 2]) {
        await _pumpPin(
          tester,
          _pin(
            kind: UpegPinKind.live,
            running: true,
            stale: true,
            restored: true,
            showMoveHandle: true,
          ),
          textScale: scale,
        );

        expect(tester.takeException(), isNull, reason: 'text scale $scale');
      }
    });
  });

  group('Pin handles and title', () {
    testWidgets('the_move_handle_uses_peg_colors_in_both_themes', (
      tester,
    ) async {
      for (final theme in <ThemeData>[
        UpegTheme.darkTheme(),
        UpegTheme.lightTheme(),
      ]) {
        await _pumpPin(tester, _pin(showMoveHandle: true), theme: theme);
        await tester.pumpAndSettle(); // the app animates a theme change

        final tokens = theme.extension<UpegTokens>()!;
        final handle = tester.widget<CustomPaint>(find.byKey(pinMoveHandleKey));
        final painter = handle.painter! as PinMoveHandlePainter;
        expect(painter.head, tokens.pin.withValues(alpha: 0.75));
        expect(painter.stem, tokens.fg3);
      }
    });

    testWidgets('handle_and_header_connector_update_live_with_saved_style', (
      tester,
    ) async {
      await _pumpPin(tester, _pin(showMoveHandle: true));

      expect(find.byKey(pinHeaderPegConnectorKey), findsOneWidget);
      final container = ProviderScope.containerOf(
        tester.element(find.byType(Pin)),
        listen: false,
      );
      const dotsTweaks = TweaksDto(
        theme: 'Dark',
        accent: 'Green',
        showHoles: true,
        locale: 'En',
        localHttpHost: false,
        pinHandleStyle: PinHandleStyleDto.dots,
      );
      await container.read(tweaksProvider.notifier).save(dotsTweaks);
      await tester.pumpAndSettle();

      final icon = tester.widget<Icon>(find.byKey(pinMoveHandleKey));
      expect(icon.icon, Icons.drag_indicator);
      expect(icon.size, 13);
      expect(icon.color, UpegTheme.darkTheme().extension<UpegTokens>()!.fg3);
      // Peg mode has the handle paint and header connector. Dots mode uses
      // an Icon and removes the connector.
      expect(find.byKey(pinHeaderPegConnectorKey), findsNothing);

      await container
          .read(tweaksProvider.notifier)
          .save(
            const TweaksDto(
              theme: 'Dark',
              accent: 'Green',
              showHoles: true,
              locale: 'En',
              localHttpHost: false,
              pinHandleStyle: PinHandleStyleDto.peg,
            ),
          );
      await tester.pumpAndSettle();
      expect(find.byKey(pinHeaderPegConnectorKey), findsOneWidget);
      expect(find.byKey(pinMoveHandleKey), findsOneWidget);
    });

    testWidgets('the_title_is_drawn_once_even_with_a_description', (
      tester,
    ) async {
      await _pumpPin(tester, _pin(kind: UpegPinKind.action));

      expect(find.text('hex → dec'), findsOneWidget);
      expect(find.text('Converts a hex string.'), findsOneWidget);
    });
  });
}
