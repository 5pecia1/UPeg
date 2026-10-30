import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/state/inline_draft_provider.dart';
import 'package:upeg/src/state/last_outcome_provider.dart';
import 'package:upeg/src/state/live_outcome_provider.dart' show LiveDispatchFn;
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';
import 'package:upeg/src/widgets/inline/generic_inline_pin_body.dart';

import '../test_helpers/dispatch_stream_fixture.dart';
import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/pegboard_selection_overrides.dart';
import '../test_helpers/tool_fixture.dart';

final _pin = (BoardKey.parse('dev'), PinId.parse('fixture.inline_copy'));
final _tool = fixtureToolDto(
  id: 'fixture.inline_copy',
  source: const SourceDto.manual(),
  inputFields: const [
    InputFieldDto(
      key: 'left',
      label: 'Left',
      fieldType: InputFieldType.text(),
      required_: true,
    ),
    InputFieldDto(
      key: 'right',
      label: 'Right',
      fieldType: InputFieldType.text(),
      required_: true,
    ),
  ],
  outputFields: const [
    OutputFieldDto(
      key: 'result',
      label: 'Result',
      fieldType: OutputFieldType.text(),
    ),
  ],
);

CanonicalToolResult _result(String value) => CanonicalToolResult(
  ok: true,
  primaryOutputId: 'result',
  outputs: [
    CanonicalOutputEntry(
      id: 'result',
      label: 'Result',
      kind: 'string',
      value: CanonicalOutputValue.string(value: value),
    ),
  ],
);

class _Writer extends ClipboardWriter {
  final writes = <String>[];

  @override
  Future<void> write(String text) async => writes.add(text);
}

Widget _harness(LiveDispatchFn dispatch, _Writer writer) => ProviderScope(
  overrides: [
    ...i18nTestOverrides,
    ...pegboardSelectionOverrides(boardKey: 'dev'),
    ...dispatchOverrides(dispatch),
    inlineDraftProvider.overrideWithValue(InlineDraftStore()),
    lastOutcomeLoadProvider.overrideWithValue((_) => []),
    lastOutcomePersistProvider.overrideWithValue(
      ({
        required boardKey,
        required pinId,
        required toolId,
        required result,
        argsJson,
      }) {},
    ),
    clipboardWriterProvider.overrideWithValue(writer),
  ],
  child: MaterialApp(
    theme: UpegTheme.darkTheme(),
    home: Scaffold(
      body: SizedBox(
        width: 240,
        height: 200,
        child: GenericInlinePinBody(tool: _tool, pinKey: _pin),
      ),
    ),
  ),
);

Future<void> _fillAndRun(WidgetTester tester) async {
  await tester.enterText(find.byKey(const Key('field-left')), 'a');
  await tester.enterText(find.byKey(const Key('field-right')), 'b');
  await tester.pump();
  await tester.tap(find.byKey(inlineRunButtonKey));
  await tester.pump();
}

void main() {
  testWidgets('inline Copy uses the last result without running again', (
    tester,
  ) async {
    final calls = <String>[];
    final writer = _Writer();
    await tester.pumpWidget(
      _harness(({pinKey, required toolId, required args}) async {
        final value = '${args['left']}|${args['right']}';
        calls.add(value);
        return _result(value);
      }, writer),
    );
    await _fillAndRun(tester);
    expect(calls, ['a|b']);

    final copy = find.byKey(inlineCopyResultButtonKey);
    await tester.ensureVisible(copy);
    await tester.tap(copy);
    await tester.pump();
    expect(writer.writes, ['a|b']);
    expect(calls, ['a|b']);
    expect(
      tester.widget<Text>(find.byKey(inlineOutputValueKey('result'))).maxLines,
      1,
    );

    await tester.enterText(find.byKey(const Key('field-right')), 'new');
    await tester.pump();
    await tester.ensureVisible(copy);
    expect(find.text('Copy previous result'), findsOneWidget);
    await tester.tap(copy);
    await tester.pump();
    expect(writer.writes, ['a|b', 'a|b']);
    expect(calls, ['a|b']);
    await tester.pump();
    expect(find.text('Copied previous result'), findsOneWidget);
  });

  testWidgets('superseded inline run is not shown or offered for Copy', (
    tester,
  ) async {
    final completion = Completer<CanonicalToolResult>();
    var calls = 0;
    await tester.pumpWidget(
      _harness(({pinKey, required toolId, required args}) {
        calls++;
        return completion.future;
      }, _Writer()),
    );
    await _fillAndRun(tester);
    expect(calls, 1);

    final container = ProviderScope.containerOf(
      tester.element(find.byType(GenericInlinePinBody)),
    );
    container.read(lastOutcomeProvider.notifier).beginRun(_pin);
    completion.complete(_result('superseded'));
    await tester.pump();

    expect(container.read(pinLastOutcomeProvider(_pin)), isNull);
    expect(find.text('superseded'), findsNothing);
    expect(find.byKey(inlineCopyResultButtonKey), findsNothing);
  });

  testWidgets('unknown result inputs remain unknown when copied inline', (
    tester,
  ) async {
    final writer = _Writer();
    await tester.pumpWidget(
      _harness(
        ({pinKey, required toolId, required args}) async => _result('unrun'),
        writer,
      ),
    );
    final container = ProviderScope.containerOf(
      tester.element(find.byType(GenericInlinePinBody)),
    );
    container
        .read(lastOutcomeProvider.notifier)
        .record(_pin, ToolId.parse(_tool.id), _result('legacy'));
    await tester.pump();
    expect(
      find.text('Input used for this result is unavailable'),
      findsOneWidget,
    );
    final copy = find.byKey(inlineCopyResultButtonKey);
    await tester.ensureVisible(copy);
    await tester.tap(copy);
    await tester.pump();
    expect(writer.writes, ['legacy']);
    await tester.pump();
    expect(
      find.text('Copied result; its input history is unavailable'),
      findsOneWidget,
    );
  });

  testWidgets('failed inline result cannot copy hidden error text', (
    tester,
  ) async {
    final writer = _Writer();
    await tester.pumpWidget(
      _harness(
        ({pinKey, required toolId, required args}) async => _result('unused'),
        writer,
      ),
    );
    final container = ProviderScope.containerOf(
      tester.element(find.byType(GenericInlinePinBody)),
    );
    container
        .read(lastOutcomeProvider.notifier)
        .record(
          _pin,
          ToolId.parse(_tool.id),
          const CanonicalToolResult(
            ok: false,
            outputs: [],
            error: CanonicalToolError(
              code: 'failed',
              message: '/private/hidden-error.txt',
            ),
          ),
        );
    await tester.pump();
    expect(find.textContaining('/private/hidden-error.txt'), findsNothing);
    final copy = find.byKey(inlineCopyResultButtonKey);
    expect(tester.widget<TextButton>(copy).onPressed, isNull);
    expect(writer.writes, isEmpty);
  });
}
