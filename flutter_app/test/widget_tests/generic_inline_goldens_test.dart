/// Deterministic geometry/state coverage for the production generic-inline
/// pin surface.
///
/// These goldens intentionally render through
/// `_AbsoluteGrid -> _PinWithLabel -> Pin -> GenericInlinePinBody` via
/// [debugBoardCanvasGrid]. Flutter's Ahem test font makes the images stable
/// across hosts, but it cannot judge Korean glyph shape or natural CJK line
/// breaking. Real-Chrome visual QA owns those readability checks.
library;

import 'dart:async';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/capability_provider.dart';
import 'package:upeg/src/state/live_outcome_provider.dart' show LiveDispatchFn;
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/board_canvas.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';
import 'package:upeg/src/widgets/inline/generic_inline_pin_body.dart';
import 'package:upeg/src/widgets/pin.dart';

import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/tool_fixture.dart';
import '../test_helpers/dispatch_stream_fixture.dart';

const String _boardId = 'golden-inline';
const String _manualToolId = 'golden.manual_text';
const String _compactToolId = 'golden.compact_fields';
const String _resultId = 'result';
const String _resultLabel = '결과';
const String _validInput = '입력 완료';
const String _u1KoreanResult = '완료된 한국어 결과';
const String _longKoreanResult = '새로운 결과를 매우 긴 한국어 문장으로 표시하여 한 줄 말줄임을 검증한다';

const Size _viewportSize = Size(800, 600);
const Size _u1PinSize = Size(UpegSizing.pinCellWidth, boardCanvasPinCellHeight);
const Size _u2PinSize = Size(
  UpegSizing.pinCellWidth * 2 + UpegSizing.pinGap,
  boardCanvasPinCellHeight,
);

const Key _textFieldKey = Key('field-value');
const Key _selectFieldKey = Key('field-choice');
const Key _booleanFieldKey = Key('field-enabled');
const Key _dateTimeFieldKey = Key('field-when');
const Key _multiOptionsFieldKey = Key('field-colors');

const String _u1EmptyGolden =
    '../goldens/generic_inline_u1_manual_empty_dark.png';
const String _u1LoadingGolden =
    '../goldens/generic_inline_u1_manual_loading_dark.png';
const String _u1SuccessGolden =
    '../goldens/generic_inline_u1_manual_success_dark.png';
const String _u2SuccessGolden =
    '../goldens/generic_inline_u2_manual_success_light.png';
const String _u2CompactGolden =
    '../goldens/generic_inline_u2_compact_fields_light.png';

ToolDto _manualTextTool({required PegboardUnitsDto pegboardUnits}) {
  return fixtureToolDto(
    id: _manualToolId,
    toolkit: 'golden',
    label: '한국어 인라인 실행',
    description: '입력과 결과를 핀 안에서 표시한다',
    pegboardUnits: pegboardUnits,
    source: const SourceDto.manual(),
    inputFields: const <InputFieldDto>[
      InputFieldDto(
        key: 'value',
        label: '필수 입력',
        fieldType: InputFieldType.text(),
        required_: true,
      ),
    ],
    outputFields: const <OutputFieldDto>[
      OutputFieldDto(
        key: _resultId,
        label: _resultLabel,
        description: null,
        fieldType: OutputFieldType.text(),
      ),
    ],
  );
}

final ToolDto _compactFieldsTool = fixtureToolDto(
  id: _compactToolId,
  toolkit: 'golden',
  label: '비텍스트 입력',
  description: '모든 compact 제어를 핀 안에서 표시한다',
  pegboardUnits: PegboardUnitsDto.u2,
  source: const SourceDto.static_(),
  inputFields: const <InputFieldDto>[
    InputFieldDto(
      key: 'choice',
      label: '선택',
      fieldType: InputFieldType.select(
        options: <ChoiceOptionDto>[
          ChoiceOptionDto(value: '하나', label: '하나'),
          ChoiceOptionDto(value: '둘', label: '둘'),
        ],
      ),
      required_: false,
    ),
    InputFieldDto(
      key: 'enabled',
      label: '활성화',
      fieldType: InputFieldType.boolean(),
      required_: false,
    ),
    InputFieldDto(
      key: 'when',
      label: '날짜',
      fieldType: InputFieldType.dateTime(),
      required_: false,
    ),
    InputFieldDto(
      key: 'colors',
      label: '색상',
      fieldType: InputFieldType.multiOptions(
        options: <ChoiceOptionDto>[
          ChoiceOptionDto(value: '빨강', label: '빨강'),
          ChoiceOptionDto(value: '파랑', label: '파랑'),
        ],
      ),
      required_: false,
    ),
  ],
);

CanonicalToolResult _successResult(String value) {
  return CanonicalToolResult(
    ok: true,
    primaryOutputId: _resultId,
    outputs: <CanonicalOutputEntry>[
      CanonicalOutputEntry(
        id: _resultId,
        label: _resultLabel,
        kind: 'string',
        value: CanonicalOutputValue.string(value: value),
      ),
    ],
  );
}

void _useViewport(WidgetTester tester) {
  final view = tester.view
    ..devicePixelRatio = 1
    ..physicalSize = _viewportSize;
  addTearDown(() {
    view
      ..resetDevicePixelRatio()
      ..resetPhysicalSize();
  });
}

Widget _harness({
  required ToolDto tool,
  required ThemeData theme,
  required LiveDispatchFn dispatch,
  required int widthUnits,
  required int heightUnits,
}) {
  final snapshot = LayoutSnapshotDto(
    boardKey: _boardId,
    boardCols: 2,
    placements: <PlacementDto>[
      PlacementDto(toolId: tool.id, x: 0, y: 0, w: widthUnits, h: heightUnits),
    ],
  );
  return ProviderScope(
    overrides: [
      ...i18nTestOverrides,
      toolsLoaderProvider.overrideWith(
        (ref) =>
            () => <ToolDto>[tool],
      ),
      ...dispatchOverrides(dispatch),
      isWasmRuntimeProvider.overrideWithValue(false),
    ],
    child: MaterialApp(
      theme: theme,
      debugShowCheckedModeBanner: false,
      home: Scaffold(
        body: debugBoardCanvasGrid(
          snapshot: snapshot,
          onPinTap: (_) {},
          onOpenModal: (_) {},
        ),
      ),
    ),
  );
}

final class _DeferredDispatch {
  final Completer<CanonicalToolResult> completer =
      Completer<CanonicalToolResult>();
  int callCount = 0;

  Future<CanonicalToolResult> call({
    required ToolId toolId,
    required ToolArgs args,
  }) {
    callCount += 1;
    return completer.future;
  }
}

final class _GenericInlineGoldenRobot {
  const _GenericInlineGoldenRobot(this.tester);

  final WidgetTester tester;

  Finder get _pin => find.byType(Pin);

  Future<void> pumpSurface({
    required ToolDto tool,
    required ThemeData theme,
    required LiveDispatchFn dispatch,
    required int widthUnits,
    required int heightUnits,
  }) async {
    await tester.pumpWidget(
      _harness(
        tool: tool,
        theme: theme,
        dispatch: dispatch,
        widthUnits: widthUnits,
        heightUnits: heightUnits,
      ),
    );
    await tester.pumpAndSettle();
  }

  void expectExactPinSize(Size expected) {
    expect(_pin, findsOneWidget);
    expect(tester.getSize(_pin), expected);
  }

  void expectManualRun({required bool enabled}) {
    final button = tester.widget<FilledButton>(find.byKey(inlineRunButtonKey));
    expect(button.onPressed != null, enabled);
  }

  Future<void> enterValidInput() async {
    await tester.enterText(find.byKey(_textFieldKey), _validInput);
    await tester.pump();
  }

  Future<void> tapRunWithoutSettling() async {
    await tester.ensureVisible(find.byKey(inlineRunButtonKey));
    await tester.pump();
    await tester.tap(find.byKey(inlineRunButtonKey));
    await tester.pump();
  }

  Future<void> waitForResultReveal() => tester.pumpAndSettle();

  void expectLoading() {
    expect(find.byKey(inlineLoadingIndicatorKey), findsOneWidget);
  }

  void expectDispatchCount(_DeferredDispatch dispatch, int expected) {
    expect(dispatch.callCount, expected);
  }

  void expectSuccessVisible() {
    expect(find.text('$_resultLabel: '), findsOneWidget);
    expect(
      find.byKey(inlineOutputValueKey(_resultId)).hitTestable(),
      findsOneWidget,
    );
  }

  void expectCompactNonTextFields() {
    expect(find.byKey(_selectFieldKey), findsOneWidget);
    expect(find.byKey(_booleanFieldKey), findsOneWidget);
    expect(find.byKey(_dateTimeFieldKey), findsOneWidget);
    expect(find.byKey(_multiOptionsFieldKey), findsOneWidget);
    expect(find.byType(DropdownButtonFormField<String>), findsOneWidget);
    expect(find.byType(Switch), findsOneWidget);
    expect(find.byType(FilterChip), findsNWidgets(2));
    expect(find.byType(SwitchListTile), findsNothing);
  }

  Future<void> revealAndSelectBottomOption() async {
    final option = find.byKey(const Key('field-colors-chip-빨강'));
    await tester.ensureVisible(option);
    await tester.pumpAndSettle();
    expect(option.hitTestable(), findsOneWidget);
    await tester.tap(option);
    await tester.pumpAndSettle();
  }

  void expectBottomOptionSelected() {
    final chip = tester.widget<FilterChip>(
      find.byKey(const Key('field-colors-chip-빨강')),
    );
    expect(chip.selected, isTrue);
  }

  void expectNoFrameworkException() {
    expect(tester.takeException(), isNull);
  }

  Future<void> capture(String path) async {
    final pinElement = _pin.evaluate().single;
    final target = pinElement.renderObject!;
    RenderObject boundary = target;
    while (!boundary.isRepaintBoundary) {
      boundary = boundary.parent!;
    }
    final layer = boundary.debugLayer! as OffsetLayer;
    final pinBounds = MatrixUtils.transformRect(
      target.getTransformTo(boundary),
      target.paintBounds,
    );
    final ui.Image image = await layer.toImage(pinBounds, pixelRatio: 1);
    try {
      await expectLater(image, matchesGoldenFile(path));
    } finally {
      image.dispose();
    }
  }
}

void main() {
  group('Generic inline pin goldens', () {
    testWidgets(
      '다크 테마 U1에서 필수 입력이 비어 있으면 Run이 비활성화된다',
      tags: <String>['golden'],
      (tester) async {
        _useViewport(tester);
        final robot = _GenericInlineGoldenRobot(tester);

        await robot.pumpSurface(
          tool: _manualTextTool(pegboardUnits: PegboardUnitsDto.u1),
          theme: UpegTheme.darkTheme(),
          dispatch: ({required toolId, required args}) async =>
              _successResult('unused'),
          widthUnits: 1,
          heightUnits: 1,
        );

        robot.expectExactPinSize(_u1PinSize);
        robot.expectManualRun(enabled: false);
        robot.expectNoFrameworkException();
        await robot.capture(_u1EmptyGolden);
      },
    );

    testWidgets(
      '다크 테마 U1에서 Manual 실행 중에는 loading 상태가 보인다',
      tags: <String>['golden'],
      (tester) async {
        _useViewport(tester);
        final deferred = _DeferredDispatch();
        final robot = _GenericInlineGoldenRobot(tester);

        await robot.pumpSurface(
          tool: _manualTextTool(pegboardUnits: PegboardUnitsDto.u1),
          theme: UpegTheme.darkTheme(),
          dispatch: deferred.call,
          widthUnits: 1,
          heightUnits: 1,
        );
        await robot.enterValidInput();
        await robot.tapRunWithoutSettling();

        robot.expectExactPinSize(_u1PinSize);
        robot.expectManualRun(enabled: false);
        robot.expectLoading();
        robot.expectDispatchCount(deferred, 1);
        robot.expectNoFrameworkException();
        await robot.capture(_u1LoadingGolden);

        deferred.completer.complete(_successResult('cleanup'));
        await tester.pump();
        await tester.pump(inlineOutputRevealDuration);
      },
    );

    testWidgets(
      '다크 테마 U1에서 성공 결과를 label과 함께 자동으로 드러낸다',
      tags: <String>['golden'],
      (tester) async {
        _useViewport(tester);
        final robot = _GenericInlineGoldenRobot(tester);

        await robot.pumpSurface(
          tool: _manualTextTool(pegboardUnits: PegboardUnitsDto.u1),
          theme: UpegTheme.darkTheme(),
          dispatch: ({required toolId, required args}) async =>
              _successResult(_u1KoreanResult),
          widthUnits: 1,
          heightUnits: 1,
        );
        await robot.enterValidInput();
        await robot.tapRunWithoutSettling();
        await robot.waitForResultReveal();

        robot.expectExactPinSize(_u1PinSize);
        robot.expectSuccessVisible();
        robot.expectNoFrameworkException();
        await robot.capture(_u1SuccessGolden);
      },
    );

    testWidgets(
      '라이트 테마 U2에서 성공 결과를 label과 함께 자동으로 드러낸다',
      tags: <String>['golden'],
      (tester) async {
        _useViewport(tester);
        final robot = _GenericInlineGoldenRobot(tester);

        await robot.pumpSurface(
          tool: _manualTextTool(pegboardUnits: PegboardUnitsDto.u2),
          theme: UpegTheme.lightTheme(),
          dispatch: ({required toolId, required args}) async =>
              _successResult(_longKoreanResult),
          widthUnits: 2,
          heightUnits: 1,
        );
        await robot.enterValidInput();
        await robot.tapRunWithoutSettling();
        await robot.waitForResultReveal();

        robot.expectExactPinSize(_u2PinSize);
        robot.expectSuccessVisible();
        robot.expectNoFrameworkException();
        await robot.capture(_u2SuccessGolden);
      },
    );

    testWidgets(
      '라이트 테마 U2에서 비텍스트 compact 입력이 overflow 없이 렌더된다',
      tags: <String>['golden'],
      (tester) async {
        _useViewport(tester);
        final robot = _GenericInlineGoldenRobot(tester);

        await robot.pumpSurface(
          tool: _compactFieldsTool,
          theme: UpegTheme.lightTheme(),
          dispatch: ({required toolId, required args}) async =>
              _successResult('unused'),
          widthUnits: 2,
          heightUnits: 1,
        );

        robot.expectExactPinSize(_u2PinSize);
        robot.expectCompactNonTextFields();
        await robot.revealAndSelectBottomOption();
        robot.expectBottomOptionSelected();
        robot.expectNoFrameworkException();
        await robot.capture(_u2CompactGolden);
      },
    );
  });
}
