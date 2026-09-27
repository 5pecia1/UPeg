import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/dispatch_stream.dart';
import 'package:upeg/src/rust/api/tools.dart' as rust;
import 'package:upeg/src/rust/api/tools/input_field.dart' as rust;
import 'package:upeg/src/state/dispatch_stream_provider.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';
import 'package:upeg/src/widgets/inline/generic_inline_pin_body.dart';

import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/tool_fixture.dart';

rust.ToolDto _manualPairTool() => fixtureToolDto(
  id: 'text.pair',
  description: 'Join two strings.',
  source: const rust.SourceDto.manual(),
  inputFields: const <rust.InputFieldDto>[
    rust.InputFieldDto(
      key: 'left',
      label: 'Left',
      fieldType: rust.InputFieldType.text(),
      required_: true,
    ),
    rust.InputFieldDto(
      key: 'right',
      label: 'Right',
      fieldType: rust.InputFieldType.text(),
      required_: true,
    ),
  ],
  outputFields: const <rust.OutputFieldDto>[
    rust.OutputFieldDto(
      key: 'result',
      label: 'Joined',
      description: null,
      fieldType: rust.OutputFieldType.text(),
    ),
  ],
);

final class _DispatchCall {
  const _DispatchCall({required this.pinKey, required this.args});

  final PinKey pinKey;
  final ToolArgs args;
}

final class _PinKeyRecordingDispatch {
  final List<_DispatchCall> calls = <_DispatchCall>[];

  Stream<DispatchStreamEventDto> call({
    PinKey? pinKey,
    required ToolId toolId,
    required ToolArgs args,
    required bool approve,
    required DispatchRunId runId,
  }) async* {
    if (pinKey == null) {
      throw StateError('Inline dispatch must identify its pin placement.');
    }
    calls.add(_DispatchCall(pinKey: pinKey, args: args));
    final values = args.toJsonObject();
    final output =
        '${pinKey.$1.value}/${pinKey.$2.value}: '
        '${values['left']}|${values['right']}';
    yield DispatchStreamEventDto.done(
      result: rust.CanonicalToolResult(
        ok: true,
        primaryOutputId: 'result',
        outputs: <rust.CanonicalOutputEntry>[
          rust.CanonicalOutputEntry(
            id: 'result',
            label: 'Joined',
            kind: 'string',
            value: rust.CanonicalOutputValue.string(value: output),
          ),
        ],
      ),
    );
  }
}

Widget _twoPinHarness({
  required rust.ToolDto tool,
  required DispatchStreamFn dispatch,
  required PinKey firstPinKey,
  required PinKey secondPinKey,
}) => ProviderScope(
  overrides: [
    ...i18nTestOverrides,
    dispatchStreamFnProvider.overrideWithValue(dispatch),
  ],
  child: MaterialApp(
    theme: UpegTheme.darkTheme(),
    home: Scaffold(
      body: Column(
        children: <Widget>[
          SizedBox(
            width: 240,
            height: 180,
            child: GenericInlinePinBody(tool: tool, pinKey: firstPinKey),
          ),
          SizedBox(
            width: 240,
            height: 180,
            child: GenericInlinePinBody(tool: tool, pinKey: secondPinKey),
          ),
        ],
      ),
    ),
  ),
);

final class _DuplicateInlinePinRobot {
  const _DuplicateInlinePinRobot(this.tester);

  final WidgetTester tester;

  Future<void> fillAndRun({
    required int cardIndex,
    required String left,
    required String right,
  }) async {
    await tester.enterText(
      find.byKey(const Key('field-left')).at(cardIndex),
      left,
    );
    await tester.pump();
    await tester.enterText(
      find.byKey(const Key('field-right')).at(cardIndex),
      right,
    );
    await tester.pump();
    await tester.tap(find.byKey(inlineRunButtonKey).at(cardIndex));
    await tester.pumpAndSettle();
  }

  void expectCallsForPins({
    required _PinKeyRecordingDispatch dispatch,
    required PinKey firstPinKey,
    required PinKey secondPinKey,
  }) {
    expect(dispatch.calls, hasLength(2));
    expect(dispatch.calls[0].pinKey, firstPinKey);
    expect(dispatch.calls[0].args.toJsonObject(), <String, Object?>{
      'left': 'first',
      'right': 'card',
    });
    expect(dispatch.calls[1].pinKey, secondPinKey);
    expect(dispatch.calls[1].args.toJsonObject(), <String, Object?>{
      'left': 'second',
      'right': 'card',
    });
  }

  void expectCardOutput({required int cardIndex, required String value}) {
    expect(
      find.descendant(
        of: find.byType(GenericInlinePinBody).at(cardIndex),
        matching: find.text(value),
      ),
      findsOneWidget,
    );
  }
}

void main() {
  testWidgets(
    'should dispatch each duplicate tool with its own pin key when run from inline cards',
    (tester) async {
      final tool = _manualPairTool();
      final firstPinKey = (BoardKey.parse('board-dev'), PinId.parse('pin-one'));
      final secondPinKey = (
        BoardKey.parse('board-dev'),
        PinId.parse('pin-two'),
      );
      final dispatch = _PinKeyRecordingDispatch();
      final robot = _DuplicateInlinePinRobot(tester);

      await tester.pumpWidget(
        _twoPinHarness(
          tool: tool,
          dispatch: dispatch.call,
          firstPinKey: firstPinKey,
          secondPinKey: secondPinKey,
        ),
      );

      await robot.fillAndRun(cardIndex: 0, left: 'first', right: 'card');
      await robot.fillAndRun(cardIndex: 1, left: 'second', right: 'card');

      robot.expectCallsForPins(
        dispatch: dispatch,
        firstPinKey: firstPinKey,
        secondPinKey: secondPinKey,
      );
      robot.expectCardOutput(
        cardIndex: 0,
        value: 'board-dev/pin-one: first|card',
      );
      robot.expectCardOutput(
        cardIndex: 1,
        value: 'board-dev/pin-two: second|card',
      );
    },
  );
}
