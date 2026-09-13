import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/state/focused_pin_provider.dart';

void main() {
  test('focusPlacement은_PlacementDto의_tool_id를_focused_pin으로_저장한다', () {
    final container = ProviderContainer();
    addTearDown(container.dispose);

    focusPlacement(
      container,
      const PlacementDto(toolId: 'num.hex_to_decimal', x: 0, y: 0, w: 1, h: 1),
    );

    expect(
      container.read(focusedPinProvider),
      ToolId.parse('num.hex_to_decimal'),
    );
  });
}
