/// F03 — Right-click on a [Pin] shows an "unpin" context menu that
/// dispatches through the central pegboard mutation path.
///
/// The retired Dioxus surface bound TogglePin to a context-menu
/// click on the pin tile. Flutter preserves that affordance through
/// the [Pin] secondary-tap handler and the canvas-level unpin seam
/// (inventory row F03).
library;

import 'package:flutter/material.dart';
import 'package:flutter/gestures.dart' show kSecondaryButton;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/pegboard_mutations_provider.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/pin.dart';

import '../test_helpers/fake_binding_catalog.dart';
import '../test_helpers/i18n_test_catalog.dart';

const _placement = PlacementDto(
  toolId: 'num.hex_to_decimal',
  x: 0,
  y: 0,
  w: 1,
  h: 1,
);

/// 사용자 span override가 있는 placement — "reset size" 항목이 노출되는
/// 전제 조건.
const _resizedPlacement = PlacementDto(
  toolId: 'num.hex_to_decimal',
  x: 0,
  y: 0,
  w: 2,
  h: 1,
  spanCols: 2,
  spanRows: 1,
);

Widget _harness({
  PinMutator? unpin,
  ClearPinSpanMutator? clearSpan,
  String? boardKey,
  PinTapCallback? onTap,
  PinTapCallback? onOpenModal,
  PlacementDto placement = _placement,
}) {
  return ProviderScope(
    overrides: [
      ...i18nTestOverrides,
      fakeBindingCatalogOverride,
      if (unpin != null) unpinToolMutatorProvider.overrideWithValue(unpin),
      if (clearSpan != null)
        clearPinSpanMutatorProvider.overrideWithValue(clearSpan),
      if (boardKey != null)
        currentBoardKeyProvider.overrideWith(
          () => _SeededCurrentBoard(boardKey),
        ),
    ],
    child: MaterialApp(
      theme: UpegTheme.darkTheme(),
      home: Scaffold(
        body: SizedBox(
          width: 220,
          height: 120,
          child: Pin(
            placement: placement,
            toolLabel: 'hex → dec',
            onTap: onTap,
            onOpenModal: onOpenModal,
          ),
        ),
      ),
    ),
  );
}

class _SeededCurrentBoard extends CurrentBoardNotifier {
  _SeededCurrentBoard(this._seed);
  final String _seed;
  @override
  BoardKey? build() {
    super.build();
    return BoardKey.parse(_seed);
  }
}

void main() {
  group('Pin context menu (F03)', () {
    testWidgets('Pin_우클릭은_unpin_메뉴_항목을_표시한다', (tester) async {
      await tester.pumpWidget(_harness(boardKey: 'dev'));
      await tester.pump();

      // 우클릭(secondary tap) 으로 context menu 를 띄운다.
      await tester.tapAt(
        tester.getCenter(find.byType(Pin)),
        buttons: kSecondaryButton,
      );
      await tester.pumpAndSettle();

      expect(find.text('unpin'), findsOneWidget);
    });

    testWidgets('Pin_unpin_메뉴_탭은_unpinTool을_호출한다', (tester) async {
      BoardKey? observedBoard;
      ToolId? observedTool;
      void recorder(BoardKey boardKey, ToolId toolId) {
        observedBoard = boardKey;
        observedTool = toolId;
      }

      await tester.pumpWidget(_harness(unpin: recorder, boardKey: 'dev'));
      await tester.pump();

      await tester.tapAt(
        tester.getCenter(find.byType(Pin)),
        buttons: kSecondaryButton,
      );
      await tester.pumpAndSettle();
      await tester.tap(find.text('unpin'));
      await tester.pumpAndSettle();

      expect(observedBoard, BoardKey.parse('dev'));
      expect(observedTool, ToolId.parse('num.hex_to_decimal'));
    });

    testWidgets('핀_우클릭은_확장_모달을_연다', (tester) async {
      PlacementDto? opened;
      var tapped = false;
      await tester.pumpWidget(
        _harness(
          boardKey: 'dev',
          onTap: (_) => tapped = true,
          onOpenModal: (placement) => opened = placement,
        ),
      );
      await tester.pump();

      // 우클릭(secondary tap) → context menu → "open" → 확장 모달 핸들러 호출.
      await tester.tapAt(
        tester.getCenter(find.byType(Pin)),
        buttons: kSecondaryButton,
      );
      await tester.pumpAndSettle();
      expect(find.text('open'), findsOneWidget);
      await tester.tap(find.text('open'));
      await tester.pumpAndSettle();

      expect(opened?.toolId, 'num.hex_to_decimal');
      // 우클릭 경로는 Run(onTap) 을 발동하지 않는다.
      expect(tapped, isFalse);
    });

    testWidgets('span_override가_있는_핀은_reset_size_메뉴를_보여준다', (tester) async {
      await tester.pumpWidget(
        _harness(boardKey: 'dev', placement: _resizedPlacement),
      );
      await tester.pump();

      await tester.tapAt(
        tester.getCenter(find.byType(Pin)),
        buttons: kSecondaryButton,
      );
      await tester.pumpAndSettle();

      expect(find.byKey(const Key('pin-context-reset-size')), findsOneWidget);
      expect(find.text(i18nEn('pin.menu.reset_size')), findsOneWidget);
    });

    testWidgets('span_override가_없는_핀은_reset_size_메뉴를_숨긴다', (tester) async {
      await tester.pumpWidget(_harness(boardKey: 'dev'));
      await tester.pump();

      await tester.tapAt(
        tester.getCenter(find.byType(Pin)),
        buttons: kSecondaryButton,
      );
      await tester.pumpAndSettle();

      expect(find.byKey(const Key('pin-context-reset-size')), findsNothing);
    });

    testWidgets('reset_size_메뉴_탭은_clearPinSpan을_호출한다', (tester) async {
      BoardKey? observedBoard;
      ToolId? observedTool;
      void recorder(BoardKey boardKey, ToolId toolId) {
        observedBoard = boardKey;
        observedTool = toolId;
      }

      await tester.pumpWidget(
        _harness(
          boardKey: 'dev',
          placement: _resizedPlacement,
          clearSpan: recorder,
        ),
      );
      await tester.pump();

      await tester.tapAt(
        tester.getCenter(find.byType(Pin)),
        buttons: kSecondaryButton,
      );
      await tester.pumpAndSettle();
      await tester.tap(find.text(i18nEn('pin.menu.reset_size')));
      await tester.pumpAndSettle();

      expect(observedBoard, BoardKey.parse('dev'));
      expect(observedTool, ToolId.parse('num.hex_to_decimal'));
    });

    testWidgets('컨텍스트_메뉴_항목은_대응_키_라벨을_병기한다', (tester) async {
      // 키 라벨은 하드코딩이 아니라 binding catalog 조회 결과다
      // (open→o, edit color→c, reset size→e, unpin→p).
      await tester.pumpWidget(
        _harness(
          boardKey: 'dev',
          placement: _resizedPlacement,
          onOpenModal: (_) {},
        ),
      );
      await tester.pump();

      await tester.tapAt(
        tester.getCenter(find.byType(Pin)),
        buttons: kSecondaryButton,
      );
      await tester.pumpAndSettle();

      Finder capInItem(String itemKey, String cap) => find.descendant(
        of: find.byKey(Key(itemKey)),
        matching: find.text(cap),
      );
      expect(capInItem('pin-context-open', 'o'), findsOneWidget);
      expect(capInItem('pin-context-edit-color', 'c'), findsOneWidget);
      expect(capInItem('pin-context-reset-size', 'e'), findsOneWidget);
      // unpin 항목은 위젯 키가 없다 — 메뉴 안에서 `p` 캡은 unpin 뿐이다.
      expect(find.text('p'), findsOneWidget);
    });

    testWidgets('핀_탭은_모달을_열지_않고_실행한다', (tester) async {
      PlacementDto? opened;
      var tapped = false;
      await tester.pumpWidget(
        _harness(
          boardKey: 'dev',
          onTap: (_) => tapped = true,
          onOpenModal: (placement) => opened = placement,
        ),
      );
      await tester.pump();

      await tester.tap(find.byType(Pin));
      await tester.pumpAndSettle();

      // Inline-first: 탭은 Run 만 하고 모달은 열지 않는다.
      expect(tapped, isTrue);
      expect(opened, isNull);
      expect(find.text('open'), findsNothing);
    });
  });
}
