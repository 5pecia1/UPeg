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

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/pages/expanded_modal_page.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/state/dispatch_stream_provider.dart';

import '../test_helpers/tool_fixture.dart';

import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/dispatch_stream_fixture.dart';

// Use a non-bespoke tool so the generic form renders and the footer
// "+ pin" button is the path under test, not a bespoke shortcut.
final ToolDto _fixtureTool = fixtureToolDto(
  id: 'fixture.echo',
  toolkit: 'fixture',
  label: 'echo',
);

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
}) {
  return ProviderScope(
    overrides: [
      ...i18nTestOverrides,
      currentBoardKeyProvider.overrideWith(
        () => _SeededCurrentBoardNotifier(boardKey),
      ),
      addPinMutatorProvider.overrideWithValue(pinFn),
      dispatchStreamFnProvider.overrideWithValue(
        stubDispatchStream(
          ({required toolId, required args, required approve}) async =>
              const CanonicalToolResult(ok: true, outputs: []),
        ),
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
      expect(find.widgetWithText(TextButton, '+ pin'), findsOneWidget);
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
        await tester.tap(find.widgetWithText(TextButton, '+ pin'));
        await tester.pumpAndSettle();
        expect(observedBoard, equals(BoardKey.parse('dev')));
        expect(observedTool, equals(ToolId.parse('fixture.echo')));
      },
    );

    testWidgets('ExpandedModalPage_pin_success_notifies_via_a_SnackBar', (
      tester,
    ) async {
      await tester.pumpWidget(_harness(pinFn: (_, _) => 'pin'));
      await tester.tap(find.widgetWithText(TextButton, '+ pin'));
      await tester.pump();
      expect(find.byType(SnackBar), findsOneWidget);
    });
  });
}
