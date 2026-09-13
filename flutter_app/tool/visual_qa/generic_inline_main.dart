import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/capability_provider.dart';
import 'package:upeg/src/state/inline_draft_provider.dart';
import 'package:upeg/src/state/live_outcome_provider.dart'
    show LiveDispatchFn, liveDispatchToolFnProvider;
import 'package:upeg/src/state/tweaks_provider.dart' show showHolesProvider;
import 'package:upeg/src/state/accent.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/board_canvas.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';
import 'package:upeg/src/widgets/inline/generic_inline_pin_body.dart';

const String _boardId = 'visual-qa-inline';
const String _resultId = 'result';
const String _defaultScenarioName = 'u1_empty';
const double _boardOriginInset = UpegSizing.pegboardPadding;
const Duration _manualTriggerDelay = Duration(milliseconds: 300);

const String _validInput = '입력 완료';
const String _u1Result = '새로운 결과를 매우 긴 한국어 문장으로 표시하여 한 줄 말줄임을 검증한다';
const String _u2Result = '사용자가 입력한 긴 한국어 문장을 처리한 최종 결과가 자연스럽게 한 줄로 표시된다';

const InputFieldDto _manualInputField = InputFieldDto(
  key: 'value',
  label: '필수 입력',
  fieldType: InputFieldType.text(),
  required_: true,
);

const OutputFieldDto _shortOutputField = OutputFieldDto(
  key: _resultId,
  label: '결과',
  fieldType: OutputFieldType.text(),
);

const OutputFieldDto _longOutputField = OutputFieldDto(
  key: _resultId,
  label: '매우 긴 한국어 결과 레이블',
  fieldType: OutputFieldType.text(),
);

enum _VisualScenario {
  u1Empty,
  u1Loading,
  u1Success,
  u2Success,
  u2Compact;

  static _VisualScenario parse(String? raw) => switch (raw) {
    'u1_loading' => _VisualScenario.u1Loading,
    'u1_success' => _VisualScenario.u1Success,
    'u2_success' => _VisualScenario.u2Success,
    'u2_compact' => _VisualScenario.u2Compact,
    _ => _VisualScenario.u1Empty,
  };

  bool get usesDarkTheme => switch (this) {
    _VisualScenario.u1Empty ||
    _VisualScenario.u1Loading ||
    _VisualScenario.u1Success => true,
    _VisualScenario.u2Success || _VisualScenario.u2Compact => false,
  };

  bool get startsManualRun => switch (this) {
    _VisualScenario.u1Loading ||
    _VisualScenario.u1Success ||
    _VisualScenario.u2Success => true,
    _VisualScenario.u1Empty || _VisualScenario.u2Compact => false,
  };

  bool get keepsDispatchPending => this == _VisualScenario.u1Loading;

  int get widthUnits => switch (this) {
    _VisualScenario.u1Empty ||
    _VisualScenario.u1Loading ||
    _VisualScenario.u1Success => 1,
    _VisualScenario.u2Success || _VisualScenario.u2Compact => 2,
  };

  int get heightUnits => widthUnits;

  double get pinWidth =>
      widthUnits * UpegSizing.pinCellWidth +
      (widthUnits - 1) * UpegSizing.pinGap;

  double get pinHeight =>
      // This target measures the test-exposed production board geometry.
      // ignore: invalid_use_of_visible_for_testing_member
      heightUnits * boardCanvasPinCellHeight +
      (heightUnits - 1) * UpegSizing.pinGap;
}

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  final scenario = _VisualScenario.parse(
    Uri.base.queryParameters['scenario'] ?? _defaultScenarioName,
  );
  runApp(_VisualQaApp(scenario: scenario));
}

final class _VisualQaApp extends StatelessWidget {
  const _VisualQaApp({required this.scenario});

  final _VisualScenario scenario;

  @override
  Widget build(BuildContext context) {
    final tool = _toolFor(scenario);
    final boardKey = BoardKey.parse(_boardId);
    final toolId = ToolId.parse(tool.id);
    final pinKey = (boardKey, toolId);
    final draftStore = InlineDraftStore();
    final initialArgs = _initialArgsFor(scenario);
    if (initialArgs != null) draftStore.set(pinKey, initialArgs);

    return ProviderScope(
      overrides: [
        toolsLoaderProvider.overrideWithValue(() => <ToolDto>[tool]),
        liveDispatchToolFnProvider.overrideWithValue(
          _dispatchFor(scenario, tool),
        ),
        inlineDraftProvider.overrideWithValue(draftStore),
        isWasmRuntimeProvider.overrideWithValue(false),
        showHolesProvider.overrideWithValue(true),
      ],
      child: MaterialApp(
        debugShowCheckedModeBanner: false,
        theme: UpegTheme.forAccent(
          Accent.green,
          brightness: scenario.usesDarkTheme
              ? Brightness.dark
              : Brightness.light,
        ),
        home: Scaffold(
          body: _ManualRunTrigger(
            enabled: scenario.startsManualRun,
            child: ClipRect(
              child: OverflowBox(
                alignment: Alignment.topLeft,
                minWidth: scenario.pinWidth + _boardOriginInset,
                maxWidth: scenario.pinWidth + _boardOriginInset,
                minHeight: scenario.pinHeight + _boardOriginInset,
                maxHeight: scenario.pinHeight + _boardOriginInset,
                child: Transform.translate(
                  offset: const Offset(-_boardOriginInset, -_boardOriginInset),
                  // This target is a QA harness for the production renderer.
                  // ignore: invalid_use_of_visible_for_testing_member
                  child: debugBoardCanvasGrid(
                    snapshot: LayoutSnapshotDto(
                      boardKey: _boardId,
                      boardCols: scenario.widthUnits,
                      placements: <PlacementDto>[
                        PlacementDto(
                          toolId: tool.id,
                          x: 0,
                          y: 0,
                          w: scenario.widthUnits,
                          h: scenario.heightUnits,
                        ),
                      ],
                    ),
                    boardKey: boardKey,
                    onPinTap: (_) {},
                    onOpenModal: (_) {},
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

final class _ManualRunTrigger extends StatefulWidget {
  const _ManualRunTrigger({required this.enabled, required this.child});

  final bool enabled;
  final Widget child;

  @override
  State<_ManualRunTrigger> createState() => _ManualRunTriggerState();
}

final class _ManualRunTriggerState extends State<_ManualRunTrigger> {
  Timer? _timer;

  @override
  void initState() {
    super.initState();
    if (widget.enabled) {
      _timer = Timer(_manualTriggerDelay, _pressRunButton);
    }
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  void _pressRunButton() {
    if (!mounted) return;
    FilledButton? runButton;

    void visit(Element element) {
      final child = element.widget;
      if (child case FilledButton(key: inlineRunButtonKey)) {
        runButton = child;
        return;
      }
      element.visitChildren(visit);
    }

    context.visitChildElements(visit);
    runButton?.onPressed?.call();
  }

  @override
  Widget build(BuildContext context) => widget.child;
}

ToolDto _toolFor(_VisualScenario scenario) {
  if (scenario == _VisualScenario.u2Compact) return _compactTool;
  final isU2 = scenario == _VisualScenario.u2Success;
  return ToolDto(
    id: isU2 ? 'visual.manual_u2' : 'visual.manual_u1',
    toolkit: 'visual',
    label: isU2 ? '긴 한국어 레이블을 가진 인라인 실행 도구' : '한국어 인라인 실행',
    description: '입력과 결과를 핀 안에서 표시한다',
    tags: const <String>[],
    inputFields: const <InputFieldDto>[_manualInputField],
    outputFields: <OutputFieldDto>[isU2 ? _longOutputField : _shortOutputField],
    pinKind: PinKindDto.inline,
    invoker: InvokerDto.function,
    pegboardUnits: isU2 ? PegboardUnitsDto.u2 : PegboardUnitsDto.u1,
    source: const SourceDto.manual(),
    requiresApproval: false,
    approvalSurfaces: const <String>[],
  );
}

const ToolDto _compactTool = ToolDto(
  id: 'visual.compact_fields',
  toolkit: 'visual',
  label: '비텍스트 compact 입력',
  description: '비텍스트 입력 제어를 핀 안에서 표시한다',
  tags: <String>[],
  inputFields: <InputFieldDto>[
    InputFieldDto(
      key: 'choice',
      label: '선택 항목',
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
      label: '기능 활성화',
      fieldType: InputFieldType.boolean(),
      required_: false,
    ),
    InputFieldDto(
      key: 'when',
      label: '실행 날짜',
      fieldType: InputFieldType.dateTime(),
      required_: false,
    ),
    InputFieldDto(
      key: 'colors',
      label: '색상 선택',
      fieldType: InputFieldType.multiOptions(
        options: <ChoiceOptionDto>[
          ChoiceOptionDto(value: '빨강', label: '빨강'),
          ChoiceOptionDto(value: '파랑', label: '파랑'),
        ],
      ),
      required_: false,
    ),
  ],
  outputFields: <OutputFieldDto>[],
  pinKind: PinKindDto.inline,
  invoker: InvokerDto.function,
  pegboardUnits: PegboardUnitsDto.u2,
  source: SourceDto.static_(),
  requiresApproval: false,
  approvalSurfaces: <String>[],
);

ToolArgs? _initialArgsFor(_VisualScenario scenario) => switch (scenario) {
  _VisualScenario.u1Empty => null,
  _VisualScenario.u1Loading ||
  _VisualScenario.u1Success ||
  _VisualScenario.u2Success => ToolArgs.fromJsonObject(const <String, Object?>{
    'value': _validInput,
  }),
  _VisualScenario.u2Compact => ToolArgs.fromJsonObject(const <String, Object?>{
    'choice': '둘',
    'enabled': true,
    'when': '2026-07-15T09:30:00.000Z',
    'colors': <String>['빨강', '파랑'],
  }),
};

LiveDispatchFn _dispatchFor(_VisualScenario scenario, ToolDto tool) {
  if (scenario.keepsDispatchPending) {
    final pending = Completer<CanonicalToolResult>();
    return ({required toolId, required args}) => pending.future;
  }
  final value = scenario == _VisualScenario.u2Success ? _u2Result : _u1Result;
  return ({required toolId, required args}) async => CanonicalToolResult(
    ok: true,
    primaryOutputId: _resultId,
    outputs: <CanonicalOutputEntry>[
      CanonicalOutputEntry(
        id: _resultId,
        label: tool.outputFields.first.label,
        kind: 'string',
        value: CanonicalOutputValue.string(value: value),
      ),
    ],
  );
}
