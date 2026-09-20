/// RED test for the modal header / footer cosmetic batch.
///
/// - Header gains a `KindBadge` chip that reads `tool.pinKind` and a
///   tool-kind-aware icon from `iconForTool(...)`.
/// - Footer gains an `invokerLabel(tool.invoker)` line ("function" /
///   "external" / "http" / …).
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/pages/expanded_modal_page.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/expanded_modal/kind_badge.dart';
import 'package:upeg/src/state/dispatch_stream_provider.dart';

import '../test_helpers/tool_fixture.dart';

import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/dispatch_stream_fixture.dart';

Widget _harness({required ToolDto tool}) {
  return ProviderScope(
    overrides: [
      ...i18nTestOverrides,
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
  group('ExpandedModalPage header chrome', () {
    testWidgets(
      'the_modal_header_shows_a_kindbadge_matching_the_tool_pinkind',
      (tester) async {
        final tool = fixtureToolDto(
          id: 'fixture.embed',
          label: 'EmbedTool',
          pinKind: PinKindDto.embed,
        );
        await tester.pumpWidget(_harness(tool: tool));
        expect(find.byType(KindBadge), findsOneWidget);
        // The label is the uppercased pinKind name.
        expect(find.text('EMBED'), findsOneWidget);
      },
    );

    testWidgets('the_modal_header_shows_the_icon_from_icon_for_tool', (
      tester,
    ) async {
      final inline = fixtureToolDto(
        id: 'inline.tool',
        label: 'inline',
        pinKind: PinKindDto.inline,
      );
      await tester.pumpWidget(_harness(tool: inline));
      // inline → Icons.flash_on (per iconForTool mapping).
      expect(find.byIcon(iconForTool(inline)), findsOneWidget);
      // The old hard-coded icon is gone.
      expect(find.byIcon(Icons.crop_square_outlined), findsNothing);
    });
  });

  group('ExpandedModalPage footer chrome', () {
    testWidgets('the_modal_footer_shows_the_invoker_label', (tester) async {
      final tool = fixtureToolDto(
        id: 'fixture.http',
        label: 'HttpTool',
        invoker: InvokerDto.http,
      );
      await tester.pumpWidget(_harness(tool: tool));
      // invokerDtoLabel(InvokerDto.http) → 'http'.
      expect(find.textContaining('http'), findsWidgets);
    });
  });
}
