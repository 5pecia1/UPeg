import 'package:flutter/material.dart';
import 'package:flutter/semantics.dart'
    show CustomSemanticsAction, SemanticsAction;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/pin.dart';
import 'package:upeg/src/rust/api/tools.dart' as rust_tools;

import '../test_helpers/color_contrast.dart';
import '../test_helpers/i18n_test_catalog.dart';

void main() {
  group('Pin', () {
    const placement = PlacementDto(
      toolId: 'num.hex_to_decimal',
      x: 0,
      y: 0,
      w: 1,
      h: 1,
    );

    Widget harness(Widget child) => ProviderScope(
      overrides: [...i18nTestOverrides],
      child: MaterialApp(
        theme: UpegTheme.darkTheme(),
        home: Scaffold(body: SizedBox(width: 220, height: 120, child: child)),
      ),
    );

    rust_tools.CanonicalToolResult canonicalGasResult() {
      return const rust_tools.CanonicalToolResult(
        ok: true,
        primaryOutputId: 'gwei',
        outputs: [
          rust_tools.CanonicalOutputEntry(
            id: 'gwei',
            label: 'Gas price',
            kind: 'number',
            value: rust_tools.CanonicalOutputValue.number(value: 32.5),
          ),
          rust_tools.CanonicalOutputEntry(
            id: 'delta_pct',
            label: 'Change',
            kind: 'number',
            value: rust_tools.CanonicalOutputValue.number(value: -1.2),
          ),
        ],
      );
    }

    testWidgets(
      'a_pin_draws_the_toolid_as_the_header_label_and_the_label_in_the_body',
      (tester) async {
        await tester.pumpWidget(
          harness(const Pin(placement: placement, toolLabel: 'hex → dec')),
        );

        // The Pin renders the canonical id uppercased in the header;
        // the human label sits in the body (layout inherited from the
        // retired Dioxus surface).
        expect(find.text('NUM.HEX_TO_DECIMAL'), findsOneWidget);
        expect(find.text('hex → dec'), findsOneWidget);
      },
    );

    testWidgets('a_pin_without_a_label_shows_nothing_in_the_body', (
      tester,
    ) async {
      await tester.pumpWidget(harness(const Pin(placement: placement)));

      // Header still has the uppercased id.
      expect(find.text('NUM.HEX_TO_DECIMAL'), findsOneWidget);
      // Body label is omitted when the catalog hasn't loaded yet.
      expect(find.text('num.hex_to_decimal'), findsNothing);
    });

    testWidgets('a_pin_invokes_the_ontap_callback_when_tapped', (tester) async {
      PlacementDto? tapped;
      await tester.pumpWidget(
        harness(
          Pin(
            placement: placement,
            toolLabel: 'hex → dec',
            onTap: (p) => tapped = p,
          ),
        ),
      );

      await tester.tap(find.byType(Pin));
      await tester.pumpAndSettle();

      expect(tapped, equals(placement));
    });

    testWidgets('a_pin_shows_the_pinkind_embed_badge', (tester) async {
      await tester.pumpWidget(
        harness(
          const Pin(
            placement: placement,
            toolLabel: 'hex → dec',
            // Use the Dart-side enum directly (mapped from PinKindDto by
            // BoardCanvas in production). G2 wired this path so a kind
            // → badge mapping doesn't require a string parse.
            pinKind: UpegPinKind.embed,
          ),
        ),
      );

      // The KindBadge in the Pin footer renders the variant uppercase.
      expect(find.text('EMBED'), findsOneWidget);
    });

    testWidgets(
      'a_pin_with_outputfields_renders_each_field_label_in_the_body',
      (tester) async {
        await tester.pumpWidget(
          harness(
            const Pin(
              placement: placement,
              toolLabel: 'hex → dec',
              toolDescription: 'description ignored when fields present',
              outputFields: <rust_tools.OutputFieldDto>[
                rust_tools.OutputFieldDto(
                  key: 'decimal',
                  label: 'Decimal',
                  fieldType: rust_tools.OutputFieldType_Number(),
                ),
                rust_tools.OutputFieldDto(
                  key: 'hex',
                  label: 'hex',
                  fieldType: rust_tools.OutputFieldType_Text(),
                ),
              ],
            ),
          ),
        );

        expect(find.text('Decimal'), findsOneWidget);
        expect(find.text('hex'), findsOneWidget);
        expect(
          find.text('description ignored when fields present'),
          findsNothing,
        );
      },
    );

    testWidgets('a_pin_without_outputfields_falls_back_to_the_description', (
      tester,
    ) async {
      await tester.pumpWidget(
        harness(
          const Pin(
            placement: placement,
            toolLabel: 'side effect',
            toolDescription: 'shown when no fields',
            outputFields: <rust_tools.OutputFieldDto>[],
          ),
        ),
      );

      expect(find.text('shown when no fields'), findsOneWidget);
    });

    testWidgets('a_pin_shows_the_invoker_external_label', (tester) async {
      await tester.pumpWidget(
        harness(
          Pin(
            placement: placement,
            toolLabel: 'hex → dec',
            pinKind: UpegPinKind.embed,
            // Mirrors `upeg_core::Invoker::label()` short form. The
            // production wire-up passes the string from `invokerDtoLabel`.
            invokerLabel: invokerDtoLabel(rust_tools.InvokerDto.external_),
          ),
        ),
      );

      expect(find.text('external'), findsOneWidget);
    });

    testWidgets('a_pin_exposes_the_tool_name_and_kind_to_screen_readers', (
      tester,
    ) async {
      final semantics = tester.ensureSemantics();
      try {
        await tester.pumpWidget(
          harness(
            Pin(
              placement: placement,
              toolLabel: '긴 한국어 도구 라벨',
              pinKind: UpegPinKind.inline,
              onTap: (_) {},
            ),
          ),
        );

        final label = i18nEn('a11y.pin.label', {
          'name': '긴 한국어 도구 라벨',
          'kind': 'INLINE',
        });
        expect(find.bySemanticsLabel(label), findsOneWidget);
        expect(
          tester.getSemantics(find.bySemanticsLabel(label)),
          isSemantics(
            label: label,
            hint: i18nEn('a11y.pin.hint_run'),
            isButton: true,
            isFocusable: true,
            hasEnabledState: true,
            isEnabled: true,
            hasTapAction: true,
            customActions: <CustomSemanticsAction>[
              CustomSemanticsAction(label: i18nEn('pin.menu.unpin')),
            ],
          ),
        );
      } finally {
        semantics.dispose();
      }
    });

    testWidgets('without_a_kind_the_label_falls_back_to_plain_toolid_text', (
      tester,
    ) async {
      final semantics = tester.ensureSemantics();
      try {
        await tester.pumpWidget(harness(const Pin(placement: placement)));

        expect(
          find.bySemanticsLabel(
            i18nEn('a11y.pin.label_plain', {'name': 'num.hex_to_decimal'}),
          ),
          findsOneWidget,
        );
      } finally {
        semantics.dispose();
      }
    });

    testWidgets(
      'an_exposed_interactive_body_reveals_the_pin_input_field_and_run_button_semantics',
      (tester) async {
        final semantics = tester.ensureSemantics();
        final robot = _PinInteractionRobot(tester, placement);
        try {
          await robot.pumpInteractivePin(onTap: () {});

          robot.expectInteractiveSemanticsVisible();
        } finally {
          semantics.dispose();
        }
      },
    );

    testWidgets(
      'a_result_is_summarized_in_the_semantics_value_and_announced_via_liveregion',
      (tester) async {
        final semantics = tester.ensureSemantics();
        try {
          await tester.pumpWidget(
            harness(
              Pin(
                placement: placement,
                toolLabel: 'eth gas',
                pinKind: UpegPinKind.live,
                outputResult: canonicalGasResult(),
              ),
            ),
          );

          final label = i18nEn('a11y.pin.label', {
            'name': 'eth gas',
            'kind': 'LIVE',
          });
          expect(
            tester.getSemantics(find.bySemanticsLabel(label)),
            isSemantics(
              label: label,
              value: i18nEn('a11y.pin.result_ok', {'preview': '32.5'}),
              isLiveRegion: true,
              isFocusable: true,
              hasEnabledState: true,
              customActions: <CustomSemanticsAction>[
                CustomSemanticsAction(label: i18nEn('pin.menu.unpin')),
              ],
            ),
          );
        } finally {
          semantics.dispose();
        }
      },
    );

    testWidgets(
      'an_error_result_exposes_error_and_its_first_line_in_the_semantics_value',
      (tester) async {
        final semantics = tester.ensureSemantics();
        try {
          await tester.pumpWidget(
            harness(
              const Pin(
                placement: placement,
                toolLabel: 'eth gas',
                pinKind: UpegPinKind.live,
                outputResult: rust_tools.CanonicalToolResult(
                  ok: false,
                  outputs: [],
                  error: rust_tools.CanonicalToolError(
                    code: 'provider_error',
                    message: 'rpc timeout\nsecond line ignored',
                  ),
                ),
              ),
            ),
          );

          final node = tester.getSemantics(
            find.bySemanticsLabel(
              i18nEn('a11y.pin.label', {'name': 'eth gas', 'kind': 'LIVE'}),
            ),
          );
          expect(
            node,
            isSemantics(
              label: i18nEn('a11y.pin.label', {
                'name': 'eth gas',
                'kind': 'LIVE',
              }),
              value: i18nEn('a11y.pin.result_error', {
                'preview': 'rpc timeout',
              }),
              isLiveRegion: true,
              isFocusable: true,
              hasEnabledState: true,
              customActions: <CustomSemanticsAction>[
                CustomSemanticsAction(label: i18nEn('pin.menu.unpin')),
              ],
            ),
          );
        } finally {
          semantics.dispose();
        }
      },
    );

    testWidgets('running_and_stale_states_are_exposed_in_the_semantics_value', (
      tester,
    ) async {
      final semantics = tester.ensureSemantics();
      try {
        await tester.pumpWidget(
          harness(
            Pin(
              placement: placement,
              toolLabel: 'eth gas',
              pinKind: UpegPinKind.live,
              outputResult: canonicalGasResult(),
              running: true,
              stale: true,
            ),
          ),
        );

        final node = tester.getSemantics(
          find.bySemanticsLabel(
            i18nEn('a11y.pin.label', {'name': 'eth gas', 'kind': 'LIVE'}),
          ),
        );
        final value =
            '${i18nEn('a11y.pin.running')}'
            ' · ${i18nEn('a11y.pin.result_ok', {'preview': '32.5'})}'
            ' · ${i18nEn('a11y.pin.stale')}';
        expect(
          node,
          isSemantics(
            label: i18nEn('a11y.pin.label', {
              'name': 'eth gas',
              'kind': 'LIVE',
            }),
            value: value,
            isLiveRegion: true,
            isFocusable: true,
            hasEnabledState: true,
            customActions: <CustomSemanticsAction>[
              CustomSemanticsAction(label: i18nEn('pin.menu.unpin')),
            ],
          ),
        );
      } finally {
        semantics.dispose();
      }
    });

    testWidgets(
      'context_menu_items_are_reachable_via_custom_semantics_actions',
      (tester) async {
        final semantics = tester.ensureSemantics();
        try {
          PlacementDto? opened;
          PlacementDto? colorEdited;
          await tester.pumpWidget(
            harness(
              Pin(
                placement: placement,
                toolLabel: 'hex → dec',
                pinKind: UpegPinKind.inline,
                onTap: (_) {},
                onOpenModal: (p) => opened = p,
                onEditColor: (p) => colorEdited = p,
              ),
            ),
          );

          final label = i18nEn('a11y.pin.label', {
            'name': 'hex → dec',
            'kind': 'INLINE',
          });
          final openAction = CustomSemanticsAction(
            label: i18nEn('pin.menu.open'),
          );
          final editColorAction = CustomSemanticsAction(
            label: i18nEn('pin.menu.edit_color'),
          );
          final unpinAction = CustomSemanticsAction(
            label: i18nEn('pin.menu.unpin'),
          );
          final node = tester.getSemantics(find.bySemanticsLabel(label));
          expect(
            node,
            isSemantics(
              label: label,
              hint: i18nEn('a11y.pin.hint_run'),
              isButton: true,
              isFocusable: true,
              hasEnabledState: true,
              isEnabled: true,
              hasTapAction: true,
              customActions: <CustomSemanticsAction>[
                openAction,
                editColorAction,
                unpinAction,
              ],
            ),
          );

          // Assistive-tech invocation: each custom action is wired to a
          // real handler.
          final owner = node.owner!;
          owner.performAction(
            node.id,
            SemanticsAction.customAction,
            CustomSemanticsAction.getIdentifier(openAction),
          );
          owner.performAction(
            node.id,
            SemanticsAction.customAction,
            CustomSemanticsAction.getIdentifier(editColorAction),
          );
          await tester.pumpAndSettle();

          expect(opened, placement);
          expect(colorEdited, placement);
        } finally {
          semantics.dispose();
        }
      },
    );

    testWidgets(
      'the_focus_ring_uses_the_focusring_token_regardless_of_pincoloroverride',
      (tester) async {
        await tester.pumpWidget(
          harness(
            const Pin(
              placement: placement,
              toolLabel: 'hex → dec',
              focused: true,
              pinColorOverride: '#FF5733',
            ),
          ),
        );

        final tokens = UpegTheme.darkTheme().extension<UpegTokens>()!;
        final ring = tester.widget<DecoratedBox>(find.byKey(pinFocusRingKey));
        final ringBorder = (ring.decoration as BoxDecoration).border! as Border;
        expect(ringBorder.top.color, tokens.focusRing);
        expect(ringBorder.top.color, isNot(const Color(0xFFFF5733)));
        expect(ringBorder.top.width, greaterThanOrEqualTo(2.0));

        // The pin's own border still uses the override color — the focus
        // indicator doesn't steal the color channel.
        final chrome = tester.widget<Container>(
          find.byKey(pinChromeContainerKey),
        );
        final chromeBorder =
            (chrome.decoration! as BoxDecoration).border! as Border;
        expect(chromeBorder.top.color, const Color(0xFFFF5733));
      },
    );

    testWidgets('an_unfocused_pin_has_no_focus_ring', (tester) async {
      await tester.pumpWidget(
        harness(const Pin(placement: placement, toolLabel: 'hex → dec')),
      );

      expect(find.byKey(pinFocusRingKey), findsNothing);
    });

    testWidgets(
      'interacting_with_body_input_and_run_never_opens_the_pin_only_a_header_tap_does',
      (tester) async {
        var pinTapCount = 0;
        final robot = _PinInteractionRobot(tester, placement);
        await robot.pumpInteractivePin(onTap: () => pinTapCount += 1);

        await robot.tapField();
        robot.expectPinTapCount(pinTapCount, 0);

        await robot.tapRunButton();
        robot.expectPinTapCount(pinTapCount, 0);

        await robot.tapHeader();
        robot.expectPinTapCount(pinTapCount, 1);
      },
    );

    testWidgets('a_canonical_output_renders_as_label_value_pairs', (
      tester,
    ) async {
      await tester.pumpWidget(
        harness(
          Pin(
            placement: PlacementDto(toolId: 'eth.gas', x: 0, y: 0, w: 1, h: 1),
            toolLabel: 'eth gas',
            pinKind: UpegPinKind.live,
            outputFields: const [
              rust_tools.OutputFieldDto(
                key: 'gwei',
                label: 'gwei',
                description: null,
                fieldType: rust_tools.OutputFieldType.number(),
              ),
              rust_tools.OutputFieldDto(
                key: 'delta_pct',
                label: 'delta_pct',
                description: null,
                fieldType: rust_tools.OutputFieldType.number(),
              ),
            ],
            outputResult: canonicalGasResult(),
          ),
        ),
      );

      expect(find.text('gwei'), findsOneWidget);
      expect(find.text('32.5'), findsOneWidget);
      expect(find.text('delta_pct'), findsOneWidget);
      expect(find.text('-1.2'), findsOneWidget);
    });

    testWidgets('without_a_canonical_output_only_labels_render', (
      tester,
    ) async {
      await tester.pumpWidget(
        harness(
          const Pin(
            placement: PlacementDto(toolId: 'eth.gas', x: 0, y: 0, w: 1, h: 1),
            toolLabel: 'eth gas',
            pinKind: UpegPinKind.live,
            outputFields: [
              rust_tools.OutputFieldDto(
                key: 'gwei',
                label: 'gwei',
                description: null,
                fieldType: rust_tools.OutputFieldType.number(),
              ),
            ],
          ),
        ),
      );

      expect(find.text('gwei'), findsOneWidget);
      expect(find.text('32.5'), findsNothing);
    });

    testWidgets('a_stale_pin_shows_a_small_dot_beside_the_kind', (
      tester,
    ) async {
      await tester.pumpWidget(
        harness(
          const Pin(
            placement: PlacementDto(toolId: 'eth.gas', x: 0, y: 0, w: 1, h: 1),
            toolLabel: 'eth gas',
            pinKind: UpegPinKind.live,
            stale: true,
          ),
        ),
      );

      expect(find.byKey(const Key('pin-stale-dot')), findsOneWidget);
    });

    testWidgets('a_running_pin_shows_the_indicator', (tester) async {
      await tester.pumpWidget(
        harness(
          const Pin(
            placement: placement,
            toolLabel: 'hex → dec',
            pinKind: UpegPinKind.action,
            running: true,
          ),
        ),
      );

      expect(find.byKey(const Key('pin-running-indicator')), findsOneWidget);
    });

    testWidgets('a_pin_not_running_has_no_indicator', (tester) async {
      await tester.pumpWidget(
        harness(
          const Pin(
            placement: placement,
            toolLabel: 'hex → dec',
            pinKind: UpegPinKind.action,
          ),
        ),
      );

      expect(find.byKey(const Key('pin-running-indicator')), findsNothing);
    });

    testWidgets(
      'a_running_inline_footer_on_a_narrow_pin_never_overflows_horizontally',
      (tester) async {
        final robot = _PinFooterRobot(tester, placement);

        await robot.pumpRunningInlinePin();

        robot.expectRunningIndicatorVisible();
        robot.expectNoFlutterException();
      },
    );

    testWidgets(
      'the_light_pin_footer_kind_invoker_and_stale_markers_contrast_with_the_real_background',
      (tester) async {
        for (final kind in UpegPinKind.values) {
          await tester.pumpWidget(
            ProviderScope(
              overrides: [...i18nTestOverrides],
              child: MaterialApp(
                theme: UpegTheme.lightTheme(),
                home: Scaffold(
                  body: SizedBox(
                    width: 220,
                    height: 150,
                    child: Pin(
                      placement: placement,
                      pinKind: kind,
                      invokerLabel: 'function',
                      stale: true,
                    ),
                  ),
                ),
              ),
            ),
          );

          final chrome = tester.widget<Container>(
            find.byKey(pinChromeContainerKey),
          );
          final background = (chrome.decoration! as BoxDecoration).color!;
          final kindColor = tester
              .widget<Text>(find.text(kind.label))
              .style!
              .color!;
          final invokerColor = tester
              .widget<Text>(find.text('function'))
              .style!
              .color!;
          final stale = tester.widget<Container>(
            find.byKey(const Key('pin-stale-dot')),
          );
          final staleColor = (stale.decoration! as BoxDecoration).color!;

          for (final foreground in <Color>[
            kindColor,
            invokerColor,
            staleColor,
          ]) {
            expect(
              colorContrastRatio(foreground, background),
              greaterThanOrEqualTo(minimumNormalTextContrastRatio),
              reason: '${kind.name} footer marker must remain readable',
            );
          }
        }
      },
    );

    testWidgets('a_pin_with_a_pincoloroverride_recolors_its_border', (
      tester,
    ) async {
      await tester.pumpWidget(
        harness(
          const Pin(
            placement: placement,
            toolLabel: 'hex → dec',
            pinColorOverride: '#FF5733',
          ),
        ),
      );

      expect(find.text('NUM.HEX_TO_DECIMAL'), findsOneWidget);
    });

    testWidgets('a_restored_result_renders_the_last_run_timestamp_badge', (
      tester,
    ) async {
      await tester.pumpWidget(
        harness(
          Pin(
            placement: placement,
            toolLabel: 'eth gas',
            pinKind: UpegPinKind.live,
            outputResult: canonicalGasResult(),
            restoredAt: DateTime.now().subtract(const Duration(minutes: 5)),
          ),
        ),
      );

      expect(find.byKey(pinRestoredBadgeKey), findsOneWidget);
      expect(
        find.text(i18nEn('pin.last_run.minutes_ago', {'minutes': '5'})),
        findsOneWidget,
      );
    });

    testWidgets('a_fresh_result_has_no_timestamp_badge', (tester) async {
      await tester.pumpWidget(
        harness(
          Pin(
            placement: placement,
            toolLabel: 'eth gas',
            pinKind: UpegPinKind.live,
            outputResult: canonicalGasResult(),
          ),
        ),
      );

      expect(find.byKey(pinRestoredBadgeKey), findsNothing);
    });

    testWidgets('a_truncated_result_preview_carries_an_ellipsis', (
      tester,
    ) async {
      await tester.pumpWidget(
        harness(
          const Pin(
            placement: placement,
            toolLabel: 'echo',
            pinKind: UpegPinKind.inline,
            outputResult: rust_tools.CanonicalToolResult(
              ok: true,
              primaryOutputId: 'value',
              outputs: [
                rust_tools.CanonicalOutputEntry(
                  id: 'value',
                  label: 'Value',
                  kind: 'string',
                  value: rust_tools.CanonicalOutputValue.string(
                    value: 'partial output',
                  ),
                ),
              ],
            ),
            restoredAt: null,
            outputTruncated: true,
          ),
        ),
      );

      expect(find.text('partial output…'), findsOneWidget);
      expect(find.text('partial output'), findsNothing);
    });

    testWidgets(
      'a_restored_result_exposes_the_restored_state_and_relative_time_in_the_semantics_value',
      (tester) async {
        final semantics = tester.ensureSemantics();
        try {
          await tester.pumpWidget(
            harness(
              Pin(
                placement: placement,
                toolLabel: 'eth gas',
                pinKind: UpegPinKind.live,
                outputResult: canonicalGasResult(),
                restoredAt: DateTime.now().subtract(const Duration(minutes: 5)),
              ),
            ),
          );

          final label = i18nEn('a11y.pin.label', {
            'name': 'eth gas',
            'kind': 'LIVE',
          });
          final expectedValue = [
            i18nEn('a11y.pin.result_ok', {'preview': '32.5'}),
            i18nEn('a11y.pin.restored'),
            i18nEn('pin.last_run.minutes_ago', {'minutes': '5'}),
          ].join(' · ');
          expect(
            tester.getSemantics(find.bySemanticsLabel(label)),
            isSemantics(label: label, value: expectedValue, isLiveRegion: true),
          );
        } finally {
          semantics.dispose();
        }
      },
    );
  });
}

class _PinInteractionRobot {
  const _PinInteractionRobot(this.tester, this.placement);

  final WidgetTester tester;
  final PlacementDto placement;

  Future<void> pumpInteractivePin({required VoidCallback onTap}) async {
    await tester.pumpWidget(
      ProviderScope(
        overrides: [...i18nTestOverrides],
        child: MaterialApp(
          theme: UpegTheme.darkTheme(),
          home: Scaffold(
            body: SizedBox(
              width: 240,
              height: 180,
              child: Pin(
                placement: placement,
                toolLabel: 'interactive tool',
                showMoveHandle: true,
                exposeBodySemantics: true,
                onTap: (_) => onTap(),
                bodyOverride: Column(
                  children: [
                    const TextField(
                      decoration: InputDecoration(labelText: 'input value'),
                    ),
                    FilledButton(onPressed: () {}, child: const Text('Run')),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }

  void expectInteractiveSemanticsVisible() {
    expect(
      find.bySemanticsLabel(
        i18nEn('a11y.pin.label_plain', {'name': 'interactive tool'}),
      ),
      findsOneWidget,
    );
    expect(find.bySemanticsLabel('input value'), findsOneWidget);
    expect(find.bySemanticsLabel('Run'), findsOneWidget);
    expect(
      tester.getSemantics(find.byKey(pinMoveHandleKey)),
      matchesSemantics(tooltip: pinMoveTooltip),
    );
  }

  Future<void> tapField() async {
    await tester.tap(find.byType(TextField));
    await tester.pump();
  }

  Future<void> tapRunButton() async {
    await tester.tap(find.widgetWithText(FilledButton, 'Run'));
    await tester.pump();
  }

  Future<void> tapHeader() async {
    await tester.tap(find.text(placement.toolId.toUpperCase()));
    await tester.pump();
  }

  void expectPinTapCount(int actual, int expected) {
    expect(actual, expected);
  }
}

class _PinFooterRobot {
  const _PinFooterRobot(this.tester, this.placement);

  static const double _productionU1PinWidth = 168;
  static const double _productionU1PinHeight = 150;

  final WidgetTester tester;
  final PlacementDto placement;

  Future<void> pumpRunningInlinePin() async {
    await tester.pumpWidget(
      ProviderScope(
        overrides: [...i18nTestOverrides],
        child: MaterialApp(
          theme: UpegTheme.darkTheme(),
          home: Scaffold(
            body: SizedBox(
              width: _productionU1PinWidth,
              height: _productionU1PinHeight,
              child: Pin(
                placement: placement,
                toolLabel: 'inline tool',
                pinKind: UpegPinKind.inline,
                invokerLabel: 'function',
                running: true,
              ),
            ),
          ),
        ),
      ),
    );
  }

  void expectRunningIndicatorVisible() {
    expect(find.byKey(const Key('pin-running-indicator')), findsOneWidget);
  }

  void expectNoFlutterException() {
    expect(tester.takeException(), isNull);
  }
}
