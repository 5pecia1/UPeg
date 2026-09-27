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
  pinId: 'num.hex_to_decimal',
  x: 1,
  y: 0,
  w: 1,
  h: 1,
);

void main() {
  group('Move pin slot dispatch (F12)', () {
    test('movePinSlot_calls_the_FRB_seam_when_a_pin_is_focused', () async {
      PinId? observedPin;
      int? observedDelta;
      Future<void> recorder(PinId pinId, int delta) async {
        observedPin = pinId;
        observedDelta = delta;
      }

      final container = ProviderContainer(
        overrides: [movePinSlotFnProvider.overrideWithValue(recorder)],
      );
      addTearDown(container.dispose);

      container
          .read(focusedPinProvider.notifier)
          .focus(PinId.parse('num.hex_to_decimal'));
      await dispatchMovePinSlot(
        container,
        cmd: const KeyboardCommandDto.movePinPrev(),
      );

      expect(observedPin, PinId.parse('num.hex_to_decimal'));
      expect(observedDelta, -1);
    });

    test('movePinSlot_skips_the_seam_without_a_focused_pin', () async {
      var called = false;
      Future<void> recorder(PinId pinId, int delta) async {
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

    test('movePinSlot_passes_delta_plus_1_for_MovePinNext', () async {
      int? observedDelta;
      Future<void> recorder(PinId pinId, int delta) async {
        observedDelta = delta;
      }

      final container = ProviderContainer(
        overrides: [movePinSlotFnProvider.overrideWithValue(recorder)],
      );
      addTearDown(container.dispose);

      container
          .read(focusedPinProvider.notifier)
          .focus(PinId.parse('id.uuid_v7'));
      await dispatchMovePinSlot(
        container,
        cmd: const KeyboardCommandDto.movePinNext(),
      );

      expect(observedDelta, 1);
    });

    test(
      'the_default_movePinSlot_commits_to_the_slot_after_the_current_placement',
      () async {
        PinId? observedPin;
        int? observedX;
        int? observedY;
        Future<void> recorder(PinId pinId, int anchorX, int anchorY) async {
          observedPin = pinId;
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
            .focus(PinId.parse('num.hex_to_decimal'));
        await dispatchMovePinSlot(
          container,
          cmd: const KeyboardCommandDto.movePinNext(),
        );

        expect(observedPin, PinId.parse('num.hex_to_decimal'));
        expect(observedX, 2);
        expect(observedY, 0);
      },
    );
  });
}
