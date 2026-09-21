import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/keyboard.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/reorder_pin_provider.dart';
import 'package:upeg/src/state/pegboard_mutations_provider.dart';

class _SeededCurrentBoardNotifier extends CurrentBoardNotifier {
  _SeededCurrentBoardNotifier(this._seed);

  final String? _seed;

  @override
  BoardKey? build() => _seed == null ? null : BoardKey.parse(_seed);
}

void main() {
  group('ReorderPinProvider', () {
    test('reorderPin_default_calls_atomic_reorder_mutator_once', () async {
      BoardKey? observedBoard;
      ToolId? observedTool;
      OrderDirectionDto? observedDirection;
      var callCount = 0;

      final container = ProviderContainer(
        overrides: [
          currentBoardKeyProvider.overrideWith(
            () => _SeededCurrentBoardNotifier('dev'),
          ),
          reorderPinMutatorProvider.overrideWith(
            (ref) => (boardKey, toolId, direction) {
              callCount += 1;
              observedBoard = boardKey;
              observedTool = toolId;
              observedDirection = direction;
            },
          ),
        ],
      );
      addTearDown(container.dispose);

      await container.read(reorderPinFnProvider)(
        ToolId.parse('num.hex_to_decimal'),
        OrderDirectionDto.next,
      );

      expect(callCount, 1);
      expect(observedBoard, BoardKey.parse('dev'));
      expect(observedTool, ToolId.parse('num.hex_to_decimal'));
      expect(observedDirection, OrderDirectionDto.next);
    });

    test(
      'reorderPin_default_does_not_call_mutator_without_current_board',
      () async {
        var callCount = 0;
        final container = ProviderContainer(
          overrides: [
            currentBoardKeyProvider.overrideWith(
              () => _SeededCurrentBoardNotifier(null),
            ),
            reorderPinMutatorProvider.overrideWith(
              (ref) => (_, _, _) {
                callCount += 1;
              },
            ),
          ],
        );
        addTearDown(container.dispose);

        await container.read(reorderPinFnProvider)(
          ToolId.parse('num.hex_to_decimal'),
          OrderDirectionDto.next,
        );

        expect(callCount, 0);
      },
    );
  });
}
