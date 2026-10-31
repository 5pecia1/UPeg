import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/widgets/board_canvas.dart';

import 'board_canvas_harness.dart';

class BoardCanvasNavigationRobot {
  BoardCanvasNavigationRobot(this.tester);

  final WidgetTester tester;

  static const snapshot = LayoutSnapshotDto(
    boardKey: 'dev',
    boardCols: 6,
    placements: [
      PlacementDto(
        toolId: 'convert.base64_encode',
        pinId: 'base64',
        x: 1,
        y: 0,
        w: 1,
        h: 1,
      ),
      PlacementDto(
        toolId: 'convert.json_format',
        pinId: 'json',
        x: 2,
        y: 0,
        w: 1,
        h: 1,
      ),
    ],
  );

  static const wideSnapshot = LayoutSnapshotDto(
    boardKey: 'dev',
    boardCols: 6,
    placements: [
      PlacementDto(
        toolId: 'convert.base64_encode',
        pinId: 'first',
        x: 0,
        y: 0,
        w: 1,
        h: 1,
      ),
    ],
  );

  Future<void> pumpNarrowBoard() async {
    tester.view.physicalSize = const Size(390, 844);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    await tester.pumpWidget(boardCanvasHarness(snapshot: snapshot));
    await tester.pumpAndSettle();
  }

  Future<void> pumpWideBoard() async {
    tester.view.physicalSize = const Size(1440, 1000);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    await tester.pumpWidget(boardCanvasHarness(snapshot: wideSnapshot));
    await tester.pumpAndSettle();
  }

  void expectFirstTileCentered() {
    final first = find.byKey(
      ValueKey<PinKey>((BoardKey.parse('dev'), PinId.parse('first'))),
    );
    expect(tester.getTopLeft(first).dx, 196);
  }

  Finder get _jsonPin => find.byKey(
    ValueKey<PinKey>((BoardKey.parse('dev'), PinId.parse('json'))),
  );

  double get jsonLeft => tester.getTopLeft(_jsonPin).dx;

  void expectJsonClipped() => expect(jsonLeft, greaterThan(370));

  void expectJsonReached() => expect(jsonLeft, lessThan(300));

  Future<void> wheelOnBase64() async {
    tester.binding.handlePointerEvent(
      const PointerScrollEvent(
        position: Offset(210, 205),
        scrollDelta: Offset(450, 0),
      ),
    );
    await tester.pumpAndSettle();
  }

  void expectHorizontalScrollbarVisible() {
    final bar = tester.widget<Scrollbar>(
      find.byKey(boardCanvasHorizontalScrollbarKey),
    );
    expect(bar.thumbVisibility, isTrue);
    expect(bar.trackVisibility, isTrue);
  }

  Future<void> dragHorizontalScrollbarToEnd() async {
    final rect = tester.getRect(find.byKey(boardCanvasHorizontalScrollbarKey));
    await tester.dragFrom(
      Offset(rect.left + 50, rect.bottom - 1),
      Offset(rect.width - 100, 0),
    );
    await tester.pumpAndSettle();
  }
}
