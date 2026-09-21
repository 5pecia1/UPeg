/// Widget tests for the PinColorDialog.
///
/// Tests the dialog flow: swatch selection, HEX input validation,
/// Save/Reset/Cancel behavior. Mirrors the TUI pin color editor
/// contract from Task 4.
library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/pin_color_dialog.dart';

import '../test_helpers/i18n_test_catalog.dart';

const String _validHex = '#FF5733';
const String _invalidHex = 'GGGGGG';
const String _validHexLower = '#ff5733';

void main() {
  group('PinColorDialog', () {
    Widget buildHarness({
      String? initialColor,
      required void Function(String? colorHex) onSave,
      required void Function(String? previousColor) onReset,
      required VoidCallback onCancel,
    }) {
      return ProviderScope(
        overrides: [...i18nTestOverrides],
        child: MaterialApp(
          theme: UpegTheme.darkTheme(),
          home: Scaffold(
            body: Center(
              child: Material(
                color: Colors.transparent,
                child: PinColorDialog(
                  initialColor: initialColor,
                  onSave: onSave,
                  onReset: onReset,
                  onCancel: onCancel,
                ),
              ),
            ),
          ),
        ),
      );
    }

    group('swatch selection', () {
      testWidgets('selecting_a_swatch_updates_the_hex_input', (tester) async {
        await tester.pumpWidget(
          buildHarness(onSave: (_) {}, onReset: (_) {}, onCancel: () {}),
        );

        await tester.pumpAndSettle();

        const String swatchKey = 'color-swatch-ff5733';
        final swatch = find.byKey(Key(swatchKey));
        expect(swatch, findsWidgets);

        await tester.tap(swatch.first);
        await tester.pump();

        expect(find.text('#FF5733'), findsOneWidget);
      });
    });

    group('HEX validation', () {
      testWidgets('a_valid_hex_shows_no_error_message', (tester) async {
        await tester.pumpWidget(
          buildHarness(
            initialColor: '#000000',
            onSave: (_) {},
            onReset: (_) {},
            onCancel: () {},
          ),
        );

        await tester.pumpAndSettle();

        // Valid initial color shows no error
        expect(find.text('Invalid HEX color'), findsNothing);
        expect(find.byKey(const Key('pin-color-save')), findsWidgets);
      });

      testWidgets('an_invalid_hex_disables_the_save_button', (tester) async {
        await tester.pumpWidget(
          buildHarness(
            initialColor: _validHex,
            onSave: (_) {},
            onReset: (_) {},
            onCancel: () {},
          ),
        );

        await tester.pumpAndSettle();

        // Clear field and type invalid hex
        await tester.enterText(find.byType(TextField), _invalidHex);
        await tester.pump();

        // Save button should be disabled for invalid input
        final saveButton = tester.widget<ElevatedButton>(
          find.byKey(const Key('pin-color-save')),
        );
        expect(saveButton.onPressed, isNull);
      });

      testWidgets('hex_is_normalized_to_uppercase', (tester) async {
        await tester.pumpWidget(
          buildHarness(
            initialColor: _validHexLower,
            onSave: (_) {},
            onReset: (_) {},
            onCancel: () {},
          ),
        );
        await tester.pumpAndSettle();

        // The text field should show uppercase
        expect(find.text('#FF5733'), findsWidgets);
      });
    });

    group('save/reset/cancel', () {
      testWidgets('the_save_button_invokes_the_onsave_callback', (
        tester,
      ) async {
        var saveCalled = 0;
        String? savedColor;
        await tester.pumpWidget(
          buildHarness(
            initialColor: _validHex,
            onSave: (colorHex) {
              saveCalled++;
              savedColor = colorHex;
            },
            onReset: (_) {},
            onCancel: () {},
          ),
        );
        await tester.pumpAndSettle();

        await tester.tap(find.byKey(const Key('pin-color-save')));
        await tester.pumpAndSettle();

        expect(saveCalled, 1);
        expect(savedColor, _validHex);
      });

      testWidgets('the_reset_button_invokes_the_onreset_callback', (
        tester,
      ) async {
        var resetCalled = 0;
        await tester.pumpWidget(
          buildHarness(
            initialColor: '#FFFFFF',
            onSave: (_) {},
            onReset: (_) => resetCalled++,
            onCancel: () {},
          ),
        );
        await tester.pumpAndSettle();

        await tester.tap(find.byKey(const Key('pin-color-reset')));
        await tester.pump();

        expect(resetCalled, 1);
      });

      testWidgets('the_cancel_button_invokes_the_oncancel_callback', (
        tester,
      ) async {
        var cancelCalled = 0;
        await tester.pumpWidget(
          buildHarness(
            initialColor: _validHex,
            onSave: (_) {},
            onReset: (_) {},
            onCancel: () => cancelCalled++,
          ),
        );
        await tester.pumpAndSettle();

        await tester.tap(find.byKey(const Key('pin-color-cancel')));
        await tester.pump();

        expect(cancelCalled, 1);
      });
    });

    group('keyboard shortcuts', () {
      testWidgets('escape_triggers_cancel', (tester) async {
        var cancelCalled = 0;
        await tester.pumpWidget(
          buildHarness(
            initialColor: _validHex,
            onSave: (_) {},
            onReset: (_) {},
            onCancel: () => cancelCalled++,
          ),
        );
        await tester.pumpAndSettle();

        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await tester.pump();

        expect(cancelCalled, 1);
      });
    });

    group('default state', () {
      testWidgets('a_null_initial_color_starts_with_an_empty_hex_input', (
        tester,
      ) async {
        await tester.pumpWidget(
          buildHarness(
            initialColor: null,
            onSave: (_) {},
            onReset: (_) {},
            onCancel: () {},
          ),
        );
        await tester.pumpAndSettle();

        expect(find.text(''), findsWidgets);
      });
    });
  });
}
