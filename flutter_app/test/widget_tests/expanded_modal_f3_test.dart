/// Widget tests for the Batch O2 / I13 F3 shortcut on
/// [`ExpandedModalPage`]. F3 dispatches the same pin path as the
/// "+ pin" footer button (cycle O1) so the modal exposes a single
/// pin entrypoint regardless of mouse vs keyboard.
///
/// Bespoke forms (`hex_to_dec_form`, `uuid_v7_form`) intentionally
/// bind F1 and F2 only — F3 must bubble up to the modal-level
/// `Shortcuts` block. The bespoke-form test below pumps the
/// `num.hex_to_decimal` tool (which mounts `HexToDecForm`) and
/// verifies F3 still triggers the modal's pin action.
library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/pages/expanded_modal_page.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/expanded_modal/bespoke_forms/registry.dart';
import 'package:upeg/src/state/dispatch_stream_provider.dart';

import '../test_helpers/tool_fixture.dart';

import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/dispatch_stream_fixture.dart';

final ToolDto _genericTool = fixtureToolDto(
  id: 'fixture.echo',
  toolkit: 'fixture',
  label: 'echo',
);

final ToolDto _hexToDecTool = fixtureToolDto(
  id: hexToDecToolId,
  toolkit: 'convert',
  label: 'hex → dec',
);

class _SeededCurrentBoardNotifier extends CurrentBoardNotifier {
  _SeededCurrentBoardNotifier(this._seed);
  final String _seed;
  @override
  BoardKey? build() => BoardKey.parse(_seed);
}

Widget _harness({
  required AddPinMutator pinFn,
  required ToolDto tool,
  String boardKey = 'dev',
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
      home: ExpandedModalPage(tool: tool),
    ),
  );
}

void main() {
  group('ExpandedModalPage F3 shortcut (Batch O2, I13 F3)', () {
    testWidgets('ExpandedModalPage_F3_invokes_pinTool', (tester) async {
      BoardKey? observedBoard;
      ToolId? observedTool;
      await tester.pumpWidget(
        _harness(
          tool: _genericTool,
          pinFn: (board, tool) {
            observedBoard = board;
            observedTool = tool;
            return 'pin';
          },
        ),
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.f3);
      await tester.pumpAndSettle();
      expect(observedBoard, equals(BoardKey.parse('dev')));
      expect(observedTool, equals(ToolId.parse('fixture.echo')));
    });

    testWidgets(
      'ExpandedModalPage_F3_invokes_pinTool_even_inside_a_bespoke_form',
      (tester) async {
        BoardKey? observedBoard;
        ToolId? observedTool;
        await tester.pumpWidget(
          _harness(
            tool: _hexToDecTool,
            pinFn: (board, tool) {
              observedBoard = board;
              observedTool = tool;
              return 'pin';
            },
          ),
        );
        // The bespoke HexToDecForm autofocuses its own Focus node and
        // binds F1+F2. F3 must propagate past it to the modal shell.
        await tester.sendKeyEvent(LogicalKeyboardKey.f3);
        await tester.pumpAndSettle();
        expect(observedBoard, equals(BoardKey.parse('dev')));
        expect(observedTool, equals(ToolId.parse(hexToDecToolId)));
      },
    );
  });
}
