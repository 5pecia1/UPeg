import 'dart:ui' show SemanticsAction;

import 'package:flutter/material.dart';
import 'package:flutter/semantics.dart' show SemanticsNode;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/focused_pin_provider.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/widgets/inline/generic_inline_pin_body.dart';

import 'board_canvas_harness.dart';

class PinFocusRobot {
  PinFocusRobot(this.tester);

  final WidgetTester tester;

  static final firstPin = PinId.parse('pin-a');
  static final secondPin = PinId.parse('pin-b');

  Future<void> pumpInlineBoard() async {
    await tester.pumpWidget(
      genericInlineHarness(
        tools: [
          genericInputTool(id: 'demo.first'),
          genericInputTool(id: 'demo.second'),
        ],
        snapshot: const LayoutSnapshotDto(
          boardKey: 'dev',
          boardCols: 6,
          placements: [
            PlacementDto(
              toolId: 'demo.first',
              pinId: 'pin-a',
              x: 0,
              y: 0,
              w: 1,
              h: 1,
            ),
            PlacementDto(
              toolId: 'demo.second',
              pinId: 'pin-b',
              x: 1,
              y: 0,
              w: 1,
              h: 1,
            ),
          ],
        ),
        dispatch: ({pinKey, required toolId, required args}) async =>
            const CanonicalToolResult(ok: true, outputs: []),
        onOpenModal: (_) {},
      ),
    );
    await tester.pumpAndSettle();
  }

  ProviderContainer get _container => ProviderScope.containerOf(
    tester.element(find.byType(GenericInlinePinBody).first),
  );

  Finder get _firstField => find.descendant(
    of: find.byKey(ValueKey<PinKey>((BoardKey.parse('dev'), firstPin))),
    matching: find.byKey(const Key('field-value')),
  );

  void focusSecondLogically() {
    _container.read(focusedPinProvider.notifier).focus(secondPin);
  }

  void expectNoFocusedPin() {
    expect(_container.read(focusedPinProvider), isNull);
  }

  void expectFirstFocused() {
    expect(_container.read(focusedPinProvider), firstPin);
  }

  void expectFirstPinOpenHint() {
    final semantics = tester.ensureSemantics();
    try {
      final node = tester.getSemantics(
        find.bySemanticsLabel('demo.first · INLINE pin'),
      );
      expect(node.getSemanticsData().hint, 'tap to open full view');
    } finally {
      semantics.dispose();
    }
  }

  void expectResizeTooltipOnlyOnHandles() {
    final semantics = tester.ensureSemantics();
    try {
      final resizeNodes = <SemanticsNode>[];
      void collect(SemanticsNode node) {
        if (node.getSemanticsData().tooltip == 'resize pin') {
          resizeNodes.add(node);
        }
        node.visitChildren((child) {
          collect(child);
          return true;
        });
      }

      collect(
        tester
            .binding
            .renderViews
            .first
            .owner!
            .semanticsOwner!
            .rootSemanticsNode!,
      );
      expect(resizeNodes, hasLength(2));
      for (final node in resizeNodes) {
        expect(
          node.getSemanticsData().hasAction(SemanticsAction.focus),
          isFalse,
        );
        expect(node.rect.width, 13);
      }
    } finally {
      semantics.dispose();
    }
  }

  Future<void> tapFirstField() async {
    await tester.tap(_firstField);
    await tester.pump();
  }

  Future<void> focusFirstFieldWithKeyboard() async {
    await tester.showKeyboard(_firstField);
    await tester.pump();
  }
}
