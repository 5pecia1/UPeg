/// Focused-pin Reorder dispatch.
///
/// `Reorder` is the edit-mode `[ ]` swap command. It stays separate from
/// Cmd/Ctrl+[/] slot movement because the TUI treats it as a neighbour
/// exchange in row-major order, not as a one-cell nudge.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/keyboard.dart';
import 'package:upeg/src/state/focused_pin_provider.dart';
import 'package:upeg/src/state/reorder_pin_provider.dart';

void main() {
  group('Reorder pin dispatch', () {
    test(
      'dispatchReorderPin_passes_the_focused_pin_to_the_atomic_reorder_seam',
      () async {
        ToolId? observedTool;
        OrderDirectionDto? observedDirection;

        final container = ProviderContainer(
          overrides: [
            reorderPinFnProvider.overrideWithValue((toolId, direction) async {
              observedTool = toolId;
              observedDirection = direction;
            }),
          ],
        );
        addTearDown(container.dispose);

        container
            .read(focusedPinProvider.notifier)
            .focus(ToolId.parse('fixture.wide'));
        await dispatchReorderPin(
          container,
          const KeyboardCommandDto.reorder(direction: OrderDirectionDto.next),
        );

        expect(observedTool, ToolId.parse('fixture.wide'));
        expect(observedDirection, OrderDirectionDto.next);
      },
    );

    test(
      'dispatchReorderPin_skips_the_reorder_seam_without_a_focused_pin',
      () async {
        var called = false;

        final container = ProviderContainer(
          overrides: [
            reorderPinFnProvider.overrideWithValue((_, _) async {
              called = true;
            }),
          ],
        );
        addTearDown(container.dispose);

        await dispatchReorderPin(
          container,
          const KeyboardCommandDto.reorder(direction: OrderDirectionDto.next),
        );

        expect(called, isFalse);
      },
    );
  });
}
