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

    testWidgets('Pin은_toolId를_헤더_라벨로_그리고_label은_본문에_렌더한다', (tester) async {
      await tester.pumpWidget(
        harness(const Pin(placement: placement, toolLabel: 'hex → dec')),
      );

      // The Pin renders the canonical id uppercased in the header;
      // the human label sits in the body (layout inherited from the
      // retired Dioxus surface).
      expect(find.text('NUM.HEX_TO_DECIMAL'), findsOneWidget);
      expect(find.text('hex → dec'), findsOneWidget);
    });

    testWidgets('Pin은_label이_없으면_본문에_아무것도_표시하지_않는다', (tester) async {
      await tester.pumpWidget(harness(const Pin(placement: placement)));

      // Header still has the uppercased id.
      expect(find.text('NUM.HEX_TO_DECIMAL'), findsOneWidget);
      // Body label is omitted when the catalog hasn't loaded yet.
      expect(find.text('num.hex_to_decimal'), findsNothing);
    });

    testWidgets('Pin은_탭하면_onTap_콜백을_호출한다', (tester) async {
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

    testWidgets('Pin은_PinKind_Embed_뱃지를_표시한다', (tester) async {
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

    testWidgets('Pin은_outputFields가_있으면_각_field_라벨을_본문에_렌더한다', (tester) async {
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
    });

    testWidgets('Pin은_outputFields가_없으면_description으로_폴백한다', (tester) async {
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

    testWidgets('Pin은_invoker_External_라벨을_표시한다', (tester) async {
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

    testWidgets('Pin은_screen_reader에_툴_이름과_kind를_라벨로_노출한다', (tester) async {
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

    testWidgets('kind가_없으면_라벨은_toolId_기반_평문으로_폴백한다', (tester) async {
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

    testWidgets('대화형_본문을_노출하면_핀과_입력_필드와_실행_버튼의_의미가_모두_보인다', (tester) async {
      final semantics = tester.ensureSemantics();
      final robot = _PinInteractionRobot(tester, placement);
      try {
        await robot.pumpInteractivePin(onTap: () {});

        robot.expectInteractiveSemanticsVisible();
      } finally {
        semantics.dispose();
      }
    });

    testWidgets('결과가_있으면_semantics_value로_요약을_노출하고_liveRegion으로_공지한다', (
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
    });

    testWidgets('에러_결과는_semantics_value에_ERROR와_첫_줄_메시지를_노출한다', (tester) async {
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
            value: i18nEn('a11y.pin.result_error', {'preview': 'rpc timeout'}),
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

    testWidgets('실행_중과_stale은_semantics_value에_상태로_노출된다', (tester) async {
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

    testWidgets('컨텍스트_메뉴_항목은_custom_semantics_action으로도_도달_가능하다', (
      tester,
    ) async {
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

        // 보조기술 경유 호출: custom action 이 실제 핸들러로 배선돼 있다.
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
    });

    testWidgets('포커스_링은_pinColorOverride와_무관하게_focusRing_토큰으로_그린다', (
      tester,
    ) async {
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

      // 핀 자체 테두리는 여전히 override 색 — 포커스 표시가 색 채널을
      // 빼앗지 않는다.
      final chrome = tester.widget<Container>(
        find.byKey(pinChromeContainerKey),
      );
      final chromeBorder =
          (chrome.decoration! as BoxDecoration).border! as Border;
      expect(chromeBorder.top.color, const Color(0xFFFF5733));
    });

    testWidgets('포커스되지_않으면_포커스_링이_없다', (tester) async {
      await tester.pumpWidget(
        harness(const Pin(placement: placement, toolLabel: 'hex → dec')),
      );

      expect(find.byKey(pinFocusRingKey), findsNothing);
    });

    testWidgets('대화형_본문의_입력과_실행은_핀을_열지_않고_헤더_탭만_핀을_연다', (tester) async {
      var pinTapCount = 0;
      final robot = _PinInteractionRobot(tester, placement);
      await robot.pumpInteractivePin(onTap: () => pinTapCount += 1);

      await robot.tapField();
      robot.expectPinTapCount(pinTapCount, 0);

      await robot.tapRunButton();
      robot.expectPinTapCount(pinTapCount, 0);

      await robot.tapHeader();
      robot.expectPinTapCount(pinTapCount, 1);
    });

    testWidgets('canonical_output이_있으면_label_값_쌍으로_렌더한다', (tester) async {
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

    testWidgets('canonical_output이_없으면_라벨만_렌더한다', (tester) async {
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

    testWidgets('stale가_true이면_kind_옆에_작은_점이_표시된다', (tester) async {
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

    testWidgets('핀_실행_중에는_인디케이터가_보인다', (tester) async {
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

    testWidgets('핀_실행_중이_아니면_인디케이터가_없다', (tester) async {
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

    testWidgets('좁은 핀에서 실행 중인 Inline footer는 가로로 넘치지 않는다', (tester) async {
      final robot = _PinFooterRobot(tester, placement);

      await robot.pumpRunningInlinePin();

      robot.expectRunningIndicatorVisible();
      robot.expectNoFlutterException();
    });

    testWidgets('라이트 핀 footer의 kind와 invoker와 stale 표시는 실제 배경과 4.5 이상 대비된다', (
      tester,
    ) async {
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

        for (final foreground in <Color>[kindColor, invokerColor, staleColor]) {
          expect(
            colorContrastRatio(foreground, background),
            greaterThanOrEqualTo(minimumNormalTextContrastRatio),
            reason: '${kind.name} footer marker must remain readable',
          );
        }
      }
    });

    testWidgets('Pin은_pinColorOverride가_있으면_테두리_색상을_재정의한다', (tester) async {
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

    testWidgets('restored_결과는_지난_실행_타임스탬프_배지를_렌더한다', (tester) async {
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

    testWidgets('fresh_결과에는_타임스탬프_배지가_없다', (tester) async {
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

    testWidgets('truncated_결과의_프리뷰에는_말줄임이_붙는다', (tester) async {
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

    testWidgets('restored_결과는_semantics_value에_복원_상태와_상대_시각을_노출한다', (
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
    });
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
