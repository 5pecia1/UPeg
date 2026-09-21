/// Widget tests for [`openExpandedModalForToolId`] / [`openExpandedModalForPlacement`].
///
/// The helper guards the "unknown tool id" branch — the BoardPage relies
/// on it to surface a snackbar instead of crashing when a placement
/// references a tool that the catalog hasn't loaded yet. We also verify
/// that a known toolId pushes the modal route.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/pages/expanded_modal_page.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/tools_provider.dart';
import 'package:upeg/src/widgets/expanded_modal_button.dart';
import 'package:upeg/src/state/dispatch_stream_provider.dart';

import '../test_helpers/tool_fixture.dart';

import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/dispatch_stream_fixture.dart';

final ToolDto _hex = fixtureToolDto(
  id: 'num.hex_to_decimal',
  toolkit: 'convert',
  label: 'hex → dec',
  tags: const <String>['convert'],
);

Widget _hostHarness(
  Future<void> Function(BuildContext context, WidgetRef ref) onReady, {
  List<ToolDto>? tools,
}) {
  final List<ToolDto> effectiveTools = tools ?? <ToolDto>[_hex];
  return ProviderScope(
    overrides: [
      ...i18nTestOverrides,
      toolsLoaderProvider.overrideWith(
        (ref) =>
            () => effectiveTools,
      ),
      // Stub out FFI calls performed by the modal page itself so a
      // navigation push doesn't hit the dylib.
      dispatchStreamFnProvider.overrideWithValue(
        stubDispatchStream(
          ({required toolId, required args, required approve}) async =>
              const CanonicalToolResult(ok: true, outputs: []),
        ),
      ),
    ],
    child: MaterialApp(
      home: Consumer(
        builder: (context, ref, _) {
          // Subscribe to toolsProvider so the FutureProvider resolves
          // during pumpAndSettle. Without this watch the family lookup
          // would observe a still-loading parent and return null.
          ref.watch(toolsProvider);
          return _ReadyButton(onReady: () => onReady(context, ref));
        },
      ),
    ),
  );
}

/// Tappable affordance so the test can drive the helper inside a
/// child BuildContext that has a Navigator above it.
class _ReadyButton extends StatelessWidget {
  const _ReadyButton({required this.onReady});

  final Future<void> Function() onReady;

  @override
  Widget build(BuildContext context) => Scaffold(
    body: Center(
      child: TextButton(
        key: const Key('ready-btn'),
        onPressed: () {
          onReady();
        },
        child: const Text('go'),
      ),
    ),
  );
}

void main() {
  group('openExpandedModalForToolId', () {
    testWidgets(
      'openExpandedModalForToolId_returns_the_sentinel_for_an_unknown_id',
      (tester) async {
        ExpandedModalOpenResult? result;
        await tester.pumpWidget(
          _hostHarness((context, ref) async {
            result = await openExpandedModalForToolId(
              context,
              ref,
              ToolId.parse('nonexistent.tool'),
            );
          }),
        );
        // Wait for the tools loader to resolve.
        await tester.pumpAndSettle();

        await tester.tap(find.byKey(const Key('ready-btn')));
        await tester.pumpAndSettle();

        expect(result, isA<ExpandedModalToolUnknown>());
        expect(
          (result! as ExpandedModalToolUnknown).toolId,
          ToolId.parse('nonexistent.tool'),
        );
      },
    );

    testWidgets('openExpandedModalForToolId_opens_a_modal_for_a_known_id', (
      tester,
    ) async {
      ExpandedModalOpenResult? result;
      await tester.pumpWidget(
        _hostHarness((context, ref) async {
          result = await openExpandedModalForToolId(
            context,
            ref,
            ToolId.parse('num.hex_to_decimal'),
          );
        }),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('ready-btn')));
      // Allow the modal route to push + finish its transition animation.
      await tester.pumpAndSettle(const Duration(seconds: 1));

      // The modal page was pushed: an ExpandedModalPage exists in the
      // widget tree.
      expect(find.byType(ExpandedModalPage), findsOneWidget);
      // Close it so the test completes its open-future cleanly.
      final navigator = tester.state<NavigatorState>(find.byType(Navigator));
      navigator.pop();
      await tester.pumpAndSettle();

      expect(result, isA<ExpandedModalOpened>());
    });
  });

  group('openExpandedModalForPlacement', () {
    testWidgets(
      'openExpandedModalForPlacement_delegates_to_the_placement_toolId',
      (tester) async {
        ExpandedModalOpenResult? result;
        const placement = PlacementDto(
          toolId: 'num.hex_to_decimal',
          x: 0,
          y: 0,
          w: 1,
          h: 1,
        );
        await tester.pumpWidget(
          _hostHarness((context, ref) async {
            result = await openExpandedModalForPlacement(
              context,
              ref,
              placement,
            );
          }),
        );
        await tester.pumpAndSettle();

        await tester.tap(find.byKey(const Key('ready-btn')));
        await tester.pumpAndSettle(const Duration(seconds: 1));

        expect(find.byType(ExpandedModalPage), findsOneWidget);
        final navigator = tester.state<NavigatorState>(find.byType(Navigator));
        navigator.pop();
        await tester.pumpAndSettle();
        expect(result, isA<ExpandedModalOpened>());
      },
    );
  });
}
