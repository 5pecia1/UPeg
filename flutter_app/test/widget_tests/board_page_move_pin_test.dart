/// F12 — `Cmd+[` / `Cmd+]` reorder the focused pin via `move_pin`.
///
/// The shortcut moves the focused pin one anchor in the matching
/// direction. The Flutter `BoardPage` originally had no
/// MovePinPrev / MovePinNext handling (inventory row F12 — 🚫 not
/// implemented).
///
/// The keyboard FRB unit tests cover the Cmd+[/] → DTO mapping. The
/// Dart-side contract this test pins is:
///   * `focusedPinProvider` remembers which pin id is in focus.
///   * Dispatching a `MovePinPrev` / `MovePinNext` command calls
///     the overridable `movePinSlotFnProvider` with the focused
///     tool id + delta (-1 / +1).
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/keyboard.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/focused_pin_provider.dart';
import 'package:upeg/src/state/layout_provider.dart';
import 'package:upeg/src/state/move_pin_commit_provider.dart';
import 'package:upeg/src/state/move_pin_slot_provider.dart';

class _SeededCurrentBoardNotifier extends CurrentBoardNotifier {
  _SeededCurrentBoardNotifier(this._seed);

  final String _seed;

  @override
  BoardKey? build() => BoardKey.parse(_seed);
}

const _focusedPlacement = PlacementDto(
  toolId: 'num.hex_to_decimal',
  x: 1,
  y: 0,
  w: 1,
  h: 1,
);

void main() {
  group('Move pin slot dispatch (F12)', () {
    test('movePinSlot은_focused_pin이_있으면_FRB_seam을_호출한다', () async {
      ToolId? observedTool;
      int? observedDelta;
      Future<void> recorder(ToolId toolId, int delta) async {
        observedTool = toolId;
        observedDelta = delta;
      }

      final container = ProviderContainer(
        overrides: [movePinSlotFnProvider.overrideWithValue(recorder)],
      );
      addTearDown(container.dispose);

      container
          .read(focusedPinProvider.notifier)
          .focus(ToolId.parse('num.hex_to_decimal'));
      await dispatchMovePinSlot(
        container,
        cmd: const KeyboardCommandDto.movePinPrev(),
      );

      expect(observedTool, ToolId.parse('num.hex_to_decimal'));
      expect(observedDelta, -1);
    });

    test('movePinSlot은_focused_pin이_없으면_seam을_호출하지_않는다', () async {
      var called = false;
      Future<void> recorder(ToolId toolId, int delta) async {
        called = true;
      }

      final container = ProviderContainer(
        overrides: [movePinSlotFnProvider.overrideWithValue(recorder)],
      );
      addTearDown(container.dispose);

      await dispatchMovePinSlot(
        container,
        cmd: const KeyboardCommandDto.movePinNext(),
      );

      expect(called, isFalse);
    });

    test('movePinSlot은_MovePinNext에서_delta가_plus_1이다', () async {
      int? observedDelta;
      Future<void> recorder(ToolId toolId, int delta) async {
        observedDelta = delta;
      }

      final container = ProviderContainer(
        overrides: [movePinSlotFnProvider.overrideWithValue(recorder)],
      );
      addTearDown(container.dispose);

      container
          .read(focusedPinProvider.notifier)
          .focus(ToolId.parse('id.uuid_v7'));
      await dispatchMovePinSlot(
        container,
        cmd: const KeyboardCommandDto.movePinNext(),
      );

      expect(observedDelta, 1);
    });

    test('movePinSlot_기본구현은_현재_placement의_다음_slot으로_commit한다', () async {
      ToolId? observedTool;
      int? observedX;
      int? observedY;
      Future<void> recorder(ToolId toolId, int anchorX, int anchorY) async {
        observedTool = toolId;
        observedX = anchorX;
        observedY = anchorY;
      }

      final container = ProviderContainer(
        overrides: [
          currentBoardKeyProvider.overrideWith(
            () => _SeededCurrentBoardNotifier('dev'),
          ),
          layoutLoaderProvider.overrideWithValue(
            (boardKey) => const LayoutSnapshotDto(
              boardKey: 'dev',
              boardCols: 4,
              placements: [_focusedPlacement],
            ),
          ),
          movePinCommitFnProvider.overrideWithValue(recorder),
        ],
      );
      addTearDown(container.dispose);

      container
          .read(focusedPinProvider.notifier)
          .focus(ToolId.parse('num.hex_to_decimal'));
      await dispatchMovePinSlot(
        container,
        cmd: const KeyboardCommandDto.movePinNext(),
      );

      expect(observedTool, ToolId.parse('num.hex_to_decimal'));
      expect(observedX, 2);
      expect(observedY, 0);
    });
  });
}
