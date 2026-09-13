/// Widget tests for the popup → board pin-activation bridge.
///
/// The popup writes the tapped tool id into [pendingActivationProvider]
/// and flips [windowModeProvider] to full. Once the board page mounts
/// it runs `pinActivationFor(toolId, '{}')`, dispatches the result,
/// and clears the slot so the next popup tap fires cleanly.
///
/// Pumping the full `BoardPage` requires a Rust-opaque `AppInitReport`
/// with no Dart-side constructor, so we exercise the bridge through
/// the [PendingActivationBridge] widget the board page wraps its
/// canvas in. The `pinActivationProvider` seam is overridden so no FRB
/// dylib call leaves the test process.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/pages/expanded_modal_page.dart';
import 'package:upeg/src/rust/api/pin_activation.dart';
import 'package:upeg/src/state/pending_activation_provider.dart';
import 'package:upeg/src/state/pin_activation_provider.dart';
import 'package:upeg/src/state/tools_provider.dart';
import 'package:upeg/src/widgets/pending_activation_bridge.dart';

import '../test_helpers/tool_fixture.dart';

import '../test_helpers/i18n_test_catalog.dart';

void main() {
  group('PendingActivationBridge', () {
    testWidgets('mount는_pending_toolId에_대해_pinActivationFor를_호출한다', (
      tester,
    ) async {
      String? observedToolId;
      final container = ProviderContainer(
        overrides: [
          ...i18nTestOverrides,
          toolsLoaderProvider.overrideWithValue(() => const []),
          pinActivationProvider.overrideWithValue(({
            required ToolId toolId,
            required String argsJson,
          }) {
            observedToolId = toolId.value;
            return const PinActivationDto.openModal(toolId: 'fixture.unused');
          }),
        ],
      );
      addTearDown(container.dispose);

      container
          .read(pendingActivationProvider.notifier)
          .set(ToolId.parse('num.hex_to_decimal'));

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(
            home: PendingActivationBridge(child: SizedBox.shrink()),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(observedToolId, 'num.hex_to_decimal');
    });

    testWidgets('dispatch_후_pendingActivationProvider는_null로_초기화된다', (
      tester,
    ) async {
      final container = ProviderContainer(
        overrides: [
          ...i18nTestOverrides,
          toolsLoaderProvider.overrideWithValue(() => const []),
          pinActivationProvider.overrideWithValue(
            ({required ToolId toolId, required String argsJson}) =>
                PinActivationDto.openModal(toolId: toolId.value),
          ),
        ],
      );
      addTearDown(container.dispose);

      container
          .read(pendingActivationProvider.notifier)
          .set(ToolId.parse('id.uuid_v7'));

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(
            home: PendingActivationBridge(child: SizedBox.shrink()),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(container.read(pendingActivationProvider), isNull);
    });

    testWidgets('OpenModal_activation은_ExpandedModalPage를_연다', (tester) async {
      final container = ProviderContainer(
        overrides: [
          ...i18nTestOverrides,
          toolsLoaderProvider.overrideWithValue(
            () => [
              fixtureToolDto(id: 'num.hex_to_decimal', label: 'Hex to Dec'),
            ],
          ),
          pinActivationProvider.overrideWithValue(
            ({required ToolId toolId, required String argsJson}) =>
                PinActivationDto.openModal(toolId: toolId.value),
          ),
        ],
      );
      addTearDown(container.dispose);

      container
          .read(pendingActivationProvider.notifier)
          .set(ToolId.parse('num.hex_to_decimal'));

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(
            home: PendingActivationBridge(child: SizedBox.shrink()),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.byType(ExpandedModalPage), findsOneWidget);
    });
  });
}
