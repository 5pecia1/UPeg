/// Widget tests for the pin activation routing.
///
/// The activation decision itself is exercised in upeg-frb Rust unit
/// tests (`pin_activation_for_가_*`). Here we verify that the Dart
/// switch dispatches to the right Dart side-effect for each variant.
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/pin_activation.dart';

/// Apply a `PinActivationDto` to a set of stub handlers. Lives inline
/// (not in lib/) because the production switch is embedded in
/// `BoardPage._dispatchActivation`; this test pins the contract.
///
/// `PinActivationDto::OpenControlledEmbed` was removed in the v2
/// design — ControlledEmbed pins render inline (bodyOverride) and
/// never navigate.
void _dispatchActivation(
  PinActivationDto activation, {
  required void Function(String toolId) onOpenEmbed,
  required void Function(String toolId) onDispatchImmediate,
  required void Function(String toolId) onOpenModal,
}) {
  switch (activation) {
    case PinActivationDto_OpenEmbed(:final toolId):
      onOpenEmbed(toolId);
    case PinActivationDto_DispatchImmediate(:final toolId):
      onDispatchImmediate(toolId);
    case PinActivationDto_OpenModal(:final toolId):
      onOpenModal(toolId);
  }
}

void main() {
  group('pin activation dispatch', () {
    testWidgets('Pin_탭은_PinKind_Embed에_대해_EmbedPage를_연다', (tester) async {
      String? openedToolId;
      _dispatchActivation(
        const PinActivationDto.openEmbed(toolId: 'embed.transform_tools'),
        onOpenEmbed: (id) => openedToolId = id,
        onDispatchImmediate: (_) => fail('expected OpenEmbed branch'),
        onOpenModal: (_) => fail('expected OpenEmbed branch'),
      );

      expect(openedToolId, 'embed.transform_tools');
    });

    testWidgets('Pin_탭은_DispatchImmediate에_대해_dispatchTool을_호출한다', (
      tester,
    ) async {
      String? dispatchedTool;
      _dispatchActivation(
        const PinActivationDto.dispatchImmediate(toolId: 'id.uuid_v7'),
        onOpenEmbed: (_) => fail('expected DispatchImmediate branch'),
        onDispatchImmediate: (t) => dispatchedTool = t,
        onOpenModal: (_) => fail('expected DispatchImmediate branch'),
      );

      expect(dispatchedTool, 'id.uuid_v7');
    });

    testWidgets('Pin_탭은_OpenModal에_대해_ExpandedModal을_연다', (tester) async {
      String? openedTool;
      _dispatchActivation(
        const PinActivationDto.openModal(toolId: 'num.hex_to_decimal'),
        onOpenEmbed: (_) => fail('expected OpenModal branch'),
        onDispatchImmediate: (_) => fail('expected OpenModal branch'),
        onOpenModal: (t) => openedTool = t,
      );

      expect(openedTool, 'num.hex_to_decimal');
    });
  });
}
