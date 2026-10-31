/// Widget tests for the Batch O1 / I12 "+ pin" footer button on
/// [`ExpandedModalPage`]. The button reads the active board from
/// [`currentBoardKeyProvider`] and dispatches the pin through the central
/// pegboard mutation path.
///
/// The dispatch seam is overridden here so widget tests never load
/// the FRB dylib. A [`Scaffold`]
/// ancestor is provided by `MaterialApp`'s default route so the
/// `ScaffoldMessenger.showSnackBar` call can find one.
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/pages/expanded_modal_page.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/focused_pin_provider.dart';
import 'package:upeg/src/state/pegboard_mutations_provider.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/state/dispatch_stream_provider.dart';

import '../test_helpers/tool_fixture.dart';
import '../test_helpers/fake_keyboard_resolver.dart';

import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/pin_persistence_overrides.dart';
import '../test_helpers/dispatch_stream_fixture.dart';

// Use a non-bespoke tool so the generic form renders and the footer
// "+ pin" button is the path under test, not a bespoke shortcut.
final ToolDto _fixtureTool = fixtureToolDto(
  id: 'fixture.echo',
  toolkit: 'fixture',
  label: 'echo',
);

class _SlowPinMutations extends PegboardMutations {
  _SlowPinMutations(super.ref, this.pinFuture, this.onPinAttempt);

  final Future<PinId?> pinFuture;
  final VoidCallback? onPinAttempt;

  @override
  Future<PinId?> pin(BoardKey boardKey, ToolId toolId) {
    onPinAttempt?.call();
    return pinFuture;
  }
}

/// Pre-seeds [`currentBoardKeyProvider`] with a fixed key so the
/// modal sees an active board without exercising the persistence
/// restore path.
class _SeededCurrentBoardNotifier extends CurrentBoardNotifier {
  _SeededCurrentBoardNotifier(this._seed);
  final String _seed;
  @override
  BoardKey? build() => BoardKey.parse(_seed);
}

Widget _harness({
  required AddPinMutator pinFn,
  String boardKey = 'dev',
  ToolDto? tool,
  Future<PinId?>? slowPin,
  VoidCallback? onPinAttempt,
  VoidCallback? onRun,
}) {
  return ProviderScope(
    overrides: [
      ...i18nTestOverrides,
      fakeKeyboardResolverOverride,
      ...pinPersistenceTestOverrides,
      currentBoardKeyProvider.overrideWith(
        () => _SeededCurrentBoardNotifier(boardKey),
      ),
      addPinMutatorProvider.overrideWithValue(pinFn),
      if (slowPin != null)
        pegboardMutationsProvider.overrideWith(
          (ref) => _SlowPinMutations(ref, slowPin, onPinAttempt),
        ),
      dispatchStreamFnProvider.overrideWithValue(
        stubDispatchStream(({
          required toolId,
          required args,
          required approve,
        }) async {
          onRun?.call();
          return const CanonicalToolResult(ok: true, outputs: []);
        }),
      ),
    ],
    child: MaterialApp(
      theme: UpegTheme.darkTheme(),
      home: ExpandedModalPage(tool: tool ?? _fixtureTool),
    ),
  );
}

void main() {
  group('ExpandedModalPage + pin (Batch O1, I12)', () {
    testWidgets('ExpandedModalPage_footer_exposes_the_pin_button', (
      tester,
    ) async {
      await tester.pumpWidget(_harness(pinFn: (_, _) => 'pin'));
      expect(find.widgetWithText(TextButton, 'Add to board'), findsOneWidget);
    });

    testWidgets(
      'ExpandedModalPage_pin_button_tap_calls_pinTool_on_the_currentBoard',
      (tester) async {
        BoardKey? observedBoard;
        ToolId? observedTool;
        await tester.pumpWidget(
          _harness(
            boardKey: 'dev',
            pinFn: (board, tool) {
              observedBoard = board;
              observedTool = tool;
              return 'pin';
            },
          ),
        );
        await tester.tap(find.byKey(const Key('expanded-modal-pin-btn')));
        await tester.pumpAndSettle();
        expect(observedBoard, equals(BoardKey.parse('dev')));
        expect(observedTool, equals(ToolId.parse('fixture.echo')));
        final container = ProviderScope.containerOf(
          tester.element(find.byType(ExpandedModalPage)),
        );
        expect(container.read(focusedPinProvider), PinId.parse('pin'));
      },
    );

    testWidgets('ExpandedModalPage_pin_success_notifies_via_a_SnackBar', (
      tester,
    ) async {
      await tester.pumpWidget(_harness(pinFn: (_, _) => 'pin'));
      await tester.tap(find.byKey(const Key('expanded-modal-pin-btn')));
      await tester.pump();
      expect(find.byType(SnackBar), findsOneWidget);
    });

    testWidgets('completed pin actions can create another new instance', (
      tester,
    ) async {
      var calls = 0;
      await tester.pumpWidget(
        _harness(
          pinFn: (_, _) {
            calls++;
            return 'pin-$calls';
          },
        ),
      );
      final button = find.byKey(const Key('expanded-modal-pin-btn'));
      await tester.tap(button);
      await tester.pump();
      expect(find.text('Add as new pin'), findsOneWidget);
      await tester.tap(button);
      await tester.pumpAndSettle();
      expect(calls, 2);
      expect(find.text('Add as new pin'), findsOneWidget);
    });

    testWidgets(
      'pending pin keeps its captured input and blocks edit and Run',
      (tester) async {
        final completion = Completer<PinId?>();
        var runs = 0;
        var pinAttempts = 0;
        final tool = fixtureToolDto(
          id: 'fixture.input',
          label: 'Input',
          inputFields: const [
            InputFieldDto(
              key: 'value',
              label: 'Value',
              fieldType: InputFieldType.text(),
              required_: true,
            ),
          ],
        );
        await tester.pumpWidget(
          _harness(
            pinFn: (_, _) => 'unused',
            slowPin: completion.future,
            onPinAttempt: () => pinAttempts++,
            onRun: () => runs++,
            tool: tool,
          ),
        );
        final field = find.byKey(const Key('field-value'));
        await tester.enterText(field, 'captured');
        await tester.tap(find.byKey(const Key('expanded-modal-pin-btn')));
        await tester.pump();
        expect(
          tester
              .widget<TextButton>(
                find.byKey(const Key('expanded-modal-pin-btn')),
              )
              .onPressed,
          isNull,
        );
        await tester.sendKeyEvent(LogicalKeyboardKey.f1);
        await tester.sendKeyEvent(LogicalKeyboardKey.f3);
        await tester.sendKeyEvent(LogicalKeyboardKey.keyX);
        await tester.pump();
        expect(runs, 0);
        expect(pinAttempts, 1);
        expect(
          tester
              .widget<EditableText>(
                find.descendant(of: field, matching: find.byType(EditableText)),
              )
              .controller
              .text,
          'captured',
        );

        completion.complete(PinId.parse('created'));
        await tester.pumpAndSettle();
        expect(find.text('Add as new pin'), findsOneWidget);
        expect(
          tester
              .widget<EditableText>(
                find.descendant(of: field, matching: find.byType(EditableText)),
              )
              .controller
              .text,
          'captured',
        );
      },
    );
  });
}
