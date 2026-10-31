/// Widget tests for K04 — `showHoles` propagation to the pegboard
/// dots painter. Toggling `tweaks.showHoles` must flip the
/// `_PegboardDotsPainter.showHoles` field; the painter must skip
/// drawing entirely when the flag is `false`.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tweaks.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/tweaks_provider.dart';
import 'package:upeg/src/widgets/board_canvas.dart';

import '../test_helpers/i18n_test_catalog.dart';

const TweaksDto _holesOn = TweaksDto(
  theme: 'Dark',
  accent: 'Green',
  showHoles: true,
  locale: 'En',
  localHttpHost: false,
);

const TweaksDto _holesOff = TweaksDto(
  theme: 'Dark',
  accent: 'Green',
  showHoles: false,
  locale: 'En',
  localHttpHost: false,
);

const LayoutSnapshotDto _emptySnapshot = LayoutSnapshotDto(
  boardKey: 'dev',
  boardCols: 6,
  placements: <PlacementDto>[],
);

Widget _harness({required TweaksDto tweaks}) {
  return ProviderScope(
    overrides: [
      ...i18nTestOverrides,
      tweaksLoaderProvider.overrideWith(
        (ref) =>
            () => tweaks,
      ),
      toolsLoaderProvider.overrideWith(
        (ref) =>
            () => const <ToolDto>[],
      ),
    ],
    child: MaterialApp(
      home: Scaffold(
        body: debugBoardCanvasGrid(snapshot: _emptySnapshot, onPinTap: (_) {}),
      ),
    ),
  );
}

PegboardDotsPainter _painterFor(WidgetTester tester) {
  final paint = tester.widget<CustomPaint>(
    find.descendant(
      of: find.byType(BoardCanvasPegboardForTesting),
      matching: find.byType(CustomPaint),
    ),
  );
  return paint.painter! as PegboardDotsPainter;
}

/// Minimal mock canvas that records `drawCircle` invocations so the
/// pure painter test can assert "zero draws when showHoles=false".
class _CallCountCanvas implements Canvas {
  int drawCircleCalls = 0;
  final centers = <Offset>[];
  final radii = <double>[];
  final colors = <Color>[];

  @override
  void drawCircle(Offset c, double radius, Paint paint) {
    drawCircleCalls++;
    centers.add(c);
    radii.add(radius);
    colors.add(paint.color);
  }

  @override
  void noSuchMethod(Invocation invocation) {}
}

void main() {
  group('PegboardDotsPainter', () {
    test('PegboardDotsPainter_paint_is_a_no_op_when_showHoles_is_false', () {
      final painter = PegboardDotsPainter(
        hole: const Color(0xFFAAAAAA),
        showHoles: false,
      );
      final canvas = _CallCountCanvas();
      painter.paint(canvas, const Size(64, 64));
      expect(canvas.drawCircleCalls, 0);
    });

    test('PegboardDotsPainter_draws_dots_when_showHoles_is_true', () {
      final painter = PegboardDotsPainter(
        hole: const Color(0xFFAAAAAA),
        showHoles: true,
      );
      final canvas = _CallCountCanvas();
      painter.paint(canvas, const Size(240, 220));
      // A visible slot-intersection pattern must paint at least one dot.
      expect(canvas.drawCircleCalls, greaterThan(0));
    });

    test('sparse_holes_keep_the_padded_grid_origin', () {
      final painter = PegboardDotsPainter(
        hole: const Color(0xFFD0CEC7),
        showHoles: true,
        gridOriginX: 24,
        gridOriginY: 24,
      );
      final canvas = _CallCountCanvas();
      painter.paint(canvas, const Size(400, 360));

      expect(canvas.centers, contains(const Offset(20, 20)));
      expect(canvas.centers, contains(const Offset(196, 20)));
      expect(canvas.centers, contains(const Offset(20, 178)));
      expect(canvas.centers, isNot(contains(const Offset(64, 20))));
      expect(canvas.centers, isNot(contains(const Offset(20, 59.5))));
      expect(canvas.radii, everyElement(PegboardDotsPainter.holeRadius));
      expect(canvas.colors.map((color) => color.toARGB32()).toSet(), {
        0xFFD0CEC7,
      });
    });

    test('PegboardDotsPainter_repaints_when_showHoles_changes', () {
      final a = PegboardDotsPainter(
        hole: const Color(0xFFAAAAAA),
        showHoles: true,
      );
      final b = PegboardDotsPainter(
        hole: const Color(0xFFAAAAAA),
        showHoles: false,
      );
      expect(b.shouldRepaint(a), isTrue);
    });
  });

  group('BoardCanvas showHoles wiring', () {
    testWidgets(
      'BoardCanvas_sets_painter_showHoles_false_when_showHoles_is_false',
      (tester) async {
        await tester.pumpWidget(_harness(tweaks: _holesOff));
        await tester.pumpAndSettle();
        final painter = _painterFor(tester);
        expect(painter.showHoles, isFalse);
      },
    );

    testWidgets(
      'BoardCanvas_sets_painter_showHoles_true_when_showHoles_is_true',
      (tester) async {
        await tester.pumpWidget(_harness(tweaks: _holesOn));
        await tester.pumpAndSettle();
        final painter = _painterFor(tester);
        expect(painter.showHoles, isTrue);
      },
    );
  });
}
