import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/presentation_view.dart';
import 'package:upeg/src/state/presentation_resolver_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/pin.dart';

import '../test_helpers/i18n_test_catalog.dart';

void main() {
  for (final diagnostic in [false, true]) {
    testWidgets(
      'compact rich result replaces raw rows unless resolution failed: $diagnostic',
      (tester) async {
        await tester.pumpWidget(
          ProviderScope(
            overrides: [
              ...i18nTestOverrides,
              presentationViewResolverProvider.overrideWithValue(
                ({required toolId, required outputsJson}) =>
                    PresentationViewDto(
                      status: const PresentationStatusDto(
                        label: 'Ready',
                        tone: 'info',
                      ),
                      summary: const [
                        PresentationFieldDto(label: 'Changes', value: '3'),
                      ],
                      notices: const [],
                      rowDetails: const [],
                      actions: const [],
                      diagnostics: diagnostic
                          ? ['missing source field']
                          : const [],
                    ),
              ),
            ],
            child: MaterialApp(
              theme: UpegTheme.darkTheme(),
              home: const Scaffold(
                body: SizedBox(
                  width: 220,
                  height: 160,
                  child: Pin(
                    placement: PlacementDto(
                      toolId: 'demo.result',
                      pinId: 'demo.result',
                      x: 0,
                      y: 0,
                      w: 1,
                      h: 1,
                    ),
                    outputFields: [
                      OutputFieldDto(
                        key: 'result',
                        label: 'Raw result',
                        fieldType: OutputFieldType.text(),
                      ),
                    ],
                    outputResult: CanonicalToolResult(
                      ok: true,
                      primaryOutputId: 'result',
                      outputs: [
                        CanonicalOutputEntry(
                          id: 'result',
                          kind: 'text',
                          value: CanonicalOutputValue.string(
                            value: 'opaque raw payload',
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
          ),
        );
        expect(
          find.text('Changes: 3'),
          diagnostic ? findsNothing : findsOneWidget,
        );
        expect(
          find.text('opaque raw payload'),
          diagnostic ? findsOneWidget : findsNothing,
        );
        expect(tester.takeException(), isNull);
      },
    );
  }
}
