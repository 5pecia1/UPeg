import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/dispatch_stream.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/capability_provider.dart';
import 'package:upeg/src/state/dispatch_stream_provider.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/state/last_outcome_provider.dart';
import 'package:upeg/src/state/inline_draft_provider.dart';
import 'package:upeg/src/state/focused_pin_provider.dart';
import 'package:upeg/src/state/diagnostics_provider.dart';
import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

import '../shared/fake_pin_persistence.dart';
import '../shared/controlled_clipboard_writer.dart';
import '../shared/fake_clipboard_diagnostics.dart';
import '../test_helpers/dispatch_stream_fixture.dart';
import '../test_helpers/fake_keyboard_resolver.dart';
import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/modal_persistence_robot.dart';
import '../test_helpers/pegboard_selection_overrides.dart';
import '../test_helpers/tool_fixture.dart';

final _board = BoardKey.parse('dev');
final _pin = (_board, PinId.parse('pin-a'));
final _genericTool = fixtureToolDto(
  id: 'fixture.echo',
  label: 'Echo',
  inputFields: const [
    InputFieldDto(
      key: 'value',
      label: 'Value',
      fieldType: InputFieldType.text(),
      required_: true,
    ),
  ],
);
const _result = CanonicalToolResult(
  ok: true,
  primaryOutputId: 'answer',
  outputs: [
    CanonicalOutputEntry(
      id: 'answer',
      label: 'Answer',
      kind: 'string',
      value: CanonicalOutputValue.string(value: 'saved output'),
    ),
  ],
);
ToolArgs _args(String text) => ToolArgs.fromJsonObject({'value': text});

/// Drafts reach storage after a short deadline. A test that asserts on
/// storage, restarts from it, or ends right after an edit flushes first, as
/// the quit path does; a save timer owned by the test container would
/// otherwise still be pending when the widget tree is torn down.
void _flushDrafts(ProviderContainer container) =>
    container.read(inlineDraftProvider).flushAll();

ProviderContainer _container(
  FakePinPersistence storage, {
  ToolDto? tool,
  StubDispatch? dispatch,
  DispatchStreamFn? streamDispatch,
  ClipboardWriter? clipboard,
  DiagnosticsApi? diagnostics,
  bool web = false,
  ToolkitDispatch? webDispatch,
  AddPinMutator? addPin,
  CancelDispatchFn? cancel,
}) {
  final container = ProviderContainer(
    overrides: [
      ...pegboardSelectionOverrides(),
      ...i18nTestOverrides,
      fakeKeyboardResolverOverride,
      ...storage.overrides,
      toolsLoaderProvider.overrideWithValue(() => [tool ?? _genericTool]),
      isWasmRuntimeProvider.overrideWithValue(web),
      dispatchStreamFnProvider.overrideWithValue(
        streamDispatch ??
            stubDispatchStream(
              dispatch ??
                  ({required toolId, required args, required approve}) async =>
                      _result,
            ),
      ),
      if (clipboard != null)
        clipboardWriterProvider.overrideWithValue(clipboard),
      if (diagnostics != null)
        diagnosticsApiProvider.overrideWithValue(diagnostics),
      if (webDispatch != null)
        toolkitDispatchProvider.overrideWithValue(webDispatch),
      if (addPin != null) addPinMutatorProvider.overrideWithValue(addPin),
      if (cancel != null) cancelDispatchFnProvider.overrideWithValue(cancel),
    ],
  );
  container.read(currentBoardKeyProvider.notifier).select(_board);
  return container;
}

void main() {
  testWidgets(
    'should restore drafts and prior results without running the tool',
    (tester) async {
      final storage = FakePinPersistence();
      storage.saveDraft(_pin, ToolId.parse(_genericTool.id), _args('original'));
      storage.saveOutcome(
        boardKey: _board.value,
        pinId: _pin.$2.value,
        toolId: _genericTool.id,
        result: _result,
        argsJson: _args('original').encodeJson(),
      );
      var runs = 0;
      final container = _container(
        storage,
        dispatch: ({required toolId, required args, required approve}) async {
          runs++;
          return _result;
        },
      );
      addTearDown(container.dispose);
      final robot = ModalPersistenceRobot(tester);
      await robot.pumpScreen(
        container: container,
        tool: _genericTool,
        pinKey: _pin,
      );

      robot.expectField('original');
      robot.expectRestoredVisible();
      robot.expectResultVisible('saved output');
      robot.expectStaleNotVisible();
      expect(runs, 0);

      await robot.editField('edited');

      robot.expectStaleVisible();
      robot.expectResultVisible('saved output');
      _flushDrafts(container);
      expect(
        ToolArgs.tryDecodeObject(storage.drafts[_pin]!.argsJson),
        _args('edited'),
      );
      expect(
        ToolArgs.tryDecodeObject(storage.outcomes[_pin]!.argsJson!),
        _args('original'),
      );
      expect(runs, 0);
      await robot.close();
    },
  );

  testWidgets(
    'should retain edited drafts and results in a new app container',
    (tester) async {
      final storage = FakePinPersistence();
      final first = _container(storage);
      addTearDown(first.dispose);
      final robot = ModalPersistenceRobot(tester);
      await robot.pumpScreen(
        container: first,
        tool: _genericTool,
        pinKey: _pin,
      );
      await robot.editField('ran with this');
      await robot.run();
      robot.expectStaleNotVisible();
      await robot.editField('next draft');
      await robot.close();
      _flushDrafts(first);

      final second = _container(storage);
      addTearDown(second.dispose);
      await robot.pumpScreen(
        container: second,
        tool: _genericTool,
        pinKey: _pin,
      );

      robot.expectField('next draft');
      robot.expectResultVisible('saved output');
      robot.expectRestoredVisible();
      robot.expectStaleVisible();
      await robot.close();
    },
  );

  testWidgets('should restore invalid numeric text as an editable draft', (
    tester,
  ) async {
    final storage = FakePinPersistence();
    final tool = fixtureToolDto(
      id: 'fixture.number',
      inputFields: const [
        InputFieldDto(
          key: 'value',
          label: 'Number',
          fieldType: InputFieldType.number(),
          required_: true,
        ),
      ],
    );
    storage.saveDraft(_pin, ToolId.parse(tool.id), _args('12-'));
    final container = _container(storage, tool: tool);
    addTearDown(container.dispose);
    final robot = ModalPersistenceRobot(tester);
    await robot.pumpScreen(container: container, tool: tool, pinKey: _pin);

    robot.expectField('12-');
    await robot.close();
  });

  testWidgets(
    'should persist raw bespoke edits and restore them after reopening',
    (tester) async {
      final storage = FakePinPersistence();
      final tool = fixtureToolDto(
        id: 'num.hex_to_decimal',
        inputFields: const [
          InputFieldDto(
            key: 'input',
            label: 'Hex',
            fieldType: InputFieldType.text(),
            required_: true,
          ),
        ],
      );
      final first = _container(storage, tool: tool);
      addTearDown(first.dispose);
      final robot = ModalPersistenceRobot(tester);
      await robot.pumpScreen(container: first, tool: tool, pinKey: _pin);
      await robot.editField(' 0xff ', key: 'hex-to-dec-input');
      await tester.tap(find.byKey(const Key('hex-to-dec-run-btn')));
      await tester.pumpAndSettle();
      expect(
        ToolArgs.tryDecodeObject(storage.outcomes[_pin]!.argsJson!),
        ToolArgs.fromJsonObject(const {'input': ' 0xff '}),
      );
      robot.expectStaleNotVisible();
      await robot.close();
      _flushDrafts(first);

      final second = _container(storage, tool: tool);
      addTearDown(second.dispose);
      await robot.pumpScreen(container: second, tool: tool, pinKey: _pin);

      robot.expectField(' 0xff ', key: 'hex-to-dec-input');
      robot.expectStaleNotVisible();
      await robot.close();
    },
  );

  testWidgets(
    'should copy current draft and frozen result into each newly pinned placement',
    (tester) async {
      final storage = FakePinPersistence();
      var placements = 0;
      final container = _container(
        storage,
        addPin: (_, _) => 'new-${++placements}',
      );
      addTearDown(container.dispose);
      final robot = ModalPersistenceRobot(tester);
      await robot.pumpScreen(container: container, tool: _genericTool);
      await robot.editField('run inputs');
      await robot.run();
      await robot.editField('first draft');
      await robot.pin();
      await robot.editField('second draft');
      await robot.pin();
      await robot.editField('latest draft');
      _flushDrafts(container);

      final first = (_board, PinId.parse('new-1'));
      final second = (_board, PinId.parse('new-2'));
      expect(placements, 2);
      expect(
        ToolArgs.tryDecodeObject(storage.drafts[first]!.argsJson),
        _args('second draft'),
      );
      expect(
        ToolArgs.tryDecodeObject(storage.drafts[second]!.argsJson),
        _args('latest draft'),
      );
      expect(
        ToolArgs.tryDecodeObject(storage.outcomes[first]!.argsJson!),
        _args('run inputs'),
      );
      expect(
        ToolArgs.tryDecodeObject(storage.outcomes[second]!.argsJson!),
        _args('run inputs'),
      );
      await robot.close();
    },
  );

  testWidgets('should persist stream completion even after the modal closes', (
    tester,
  ) async {
    final storage = FakePinPersistence();
    final completion = Completer<CanonicalToolResult>();
    final container = _container(
      storage,
      dispatch: ({required toolId, required args, required approve}) =>
          completion.future,
    );
    addTearDown(container.dispose);
    final robot = ModalPersistenceRobot(tester);
    await robot.pumpScreen(
      container: container,
      tool: _genericTool,
      pinKey: _pin,
    );
    await robot.editField('captured inputs');
    await robot.run(settle: false);
    await robot.close();
    completion.complete(_result);
    await tester.pump();

    expect(storage.outcomes[_pin]?.result, _result);
    expect(
      ToolArgs.tryDecodeObject(storage.outcomes[_pin]!.argsJson!),
      _args('captured inputs'),
    );
    _flushDrafts(container);
  });

  testWidgets('should keep a cancelled native run pending until it completes', (
    tester,
  ) async {
    final storage = FakePinPersistence();
    final completion = Completer<CanonicalToolResult>();
    var cancelCalls = 0;
    final container = _container(
      storage,
      dispatch: ({required toolId, required args, required approve}) =>
          completion.future,
      cancel: ({required runId}) {
        cancelCalls++;
        return true;
      },
    );
    addTearDown(container.dispose);
    final robot = ModalPersistenceRobot(tester);
    await robot.pumpScreen(
      container: container,
      tool: _genericTool,
      pinKey: _pin,
    );
    await robot.editField('inputs');
    await robot.run(settle: false);
    robot.expectCancelVisible();
    await robot.cancel();

    robot.expectCancelRequested();
    robot.expectResultNotVisible('saved output');
    expect(cancelCalls, 1);
    completion.complete(_result);
    await tester.pumpAndSettle();
    robot.expectCancelNotVisible();
    robot.expectResultVisible('saved output');
    await robot.close();
    _flushDrafts(container);
  });

  testWidgets(
    'should omit cancel for web dispatch and persist its frozen inputs',
    (tester) async {
      final storage = FakePinPersistence();
      final completion = Completer<CanonicalToolResult>();
      final container = _container(
        storage,
        web: true,
        webDispatch:
            ({
              required toolId,
              required args,
              boardKey,
              pinId,
              required approve,
            }) => completion.future,
      );
      addTearDown(container.dispose);
      final robot = ModalPersistenceRobot(tester);
      await robot.pumpScreen(
        container: container,
        tool: _genericTool,
        pinKey: _pin,
      );
      await robot.editField('web inputs');
      await robot.run(settle: false);

      robot.expectCancelNotVisible();
      await robot.editField('later inputs');
      completion.complete(_result);
      await tester.pumpAndSettle();
      robot.expectStaleVisible();
      expect(
        ToolArgs.tryDecodeObject(storage.outcomes[_pin]!.argsJson!),
        _args('web inputs'),
      );
      await robot.close();
      _flushDrafts(container);
    },
  );
  testWidgets(
    'should report a failed placement without saving drafts into an absent pin',
    (tester) async {
      final storage = FakePinPersistence();
      final container = _container(
        storage,
        addPin: (_, _) => throw StateError('Store unavailable'),
      );
      addTearDown(container.dispose);
      final robot = ModalPersistenceRobot(tester);
      await robot.pumpScreen(container: container, tool: _genericTool);
      await robot.editField('draft');
      robot.expectPinFailureNotVisible();

      await robot.pin();

      robot.expectPinFailureVisible();
      expect(storage.drafts, isEmpty);
      expect(storage.outcomes, isEmpty);
      await robot.close();
    },
  );

  testWidgets('should recover the run controls after a web dispatch throws', (
    tester,
  ) async {
    final storage = FakePinPersistence();
    var attempts = 0;
    final container = _container(
      storage,
      web: true,
      webDispatch:
          ({
            required toolId,
            required args,
            boardKey,
            pinId,
            required approve,
          }) async {
            if (++attempts == 1) throw StateError('Dispatch unavailable');
            return _result;
          },
    );
    addTearDown(container.dispose);
    final robot = ModalPersistenceRobot(tester);
    await robot.pumpScreen(
      container: container,
      tool: _genericTool,
      pinKey: _pin,
    );
    await robot.editField('retry inputs');

    await robot.run();

    robot.expectRunEnabled();
    robot.expectCancelNotVisible();
    await robot.run();
    robot.expectResultVisible('saved output');
    expect(attempts, 2);
    await robot.close();
    _flushDrafts(container);
  });
  testWidgets('should run with Enter when focus is outside a text editor', (
    tester,
  ) async {
    final storage = FakePinPersistence();
    var runs = 0;
    final container = _container(
      storage,
      dispatch: ({required toolId, required args, required approve}) async {
        runs++;
        return _result;
      },
    );
    addTearDown(container.dispose);
    final robot = ModalPersistenceRobot(tester);
    await robot.pumpScreen(
      container: container,
      tool: _genericTool,
      initialInput: _args('ready'),
    );

    await robot.pressEnterOutsideEditor();

    expect(runs, 1);
    robot.expectResultVisible('saved output');
    await robot.close();
  });

  testWidgets(
    'should keep a newer shared result when an older modal run finishes late',
    (tester) async {
      final storage = FakePinPersistence();
      final completion = Completer<CanonicalToolResult>();
      final container = _container(
        storage,
        dispatch: ({required toolId, required args, required approve}) =>
            completion.future,
      );
      addTearDown(container.dispose);
      final robot = ModalPersistenceRobot(tester);
      await robot.pumpScreen(
        container: container,
        tool: _genericTool,
        pinKey: _pin,
      );
      await robot.editField('old inputs');
      await robot.run(settle: false);
      final recorder = container.read(lastOutcomeProvider.notifier);
      final newerRun = recorder.beginRun(_pin);
      const newerResult = CanonicalToolResult(
        ok: true,
        primaryOutputId: 'answer',
        outputs: [
          CanonicalOutputEntry(
            id: 'answer',
            label: 'Answer',
            kind: 'string',
            value: CanonicalOutputValue.string(value: 'newest output'),
          ),
        ],
      );
      recorder.record(
        _pin,
        ToolId.parse(_genericTool.id),
        newerResult,
        args: _args('new inputs'),
        runToken: newerRun,
      );
      await tester.pump();

      completion.complete(_result);
      await tester.pumpAndSettle();

      robot.expectResultVisible('newest output');
      robot.expectResultNotVisible('saved output');
      expect(storage.outcomes[_pin]!.result, newerResult);
      expect(
        ToolArgs.tryDecodeObject(storage.outcomes[_pin]!.argsJson!),
        _args('new inputs'),
      );
      await robot.close();
      _flushDrafts(container);
    },
  );
  testWidgets(
    'legacy restored result keeps unknown input history through clone and copy',
    (tester) async {
      final storage = FakePinPersistence();
      storage.saveDraft(_pin, ToolId.parse(_genericTool.id), _args('draft'));
      storage.saveOutcome(
        boardKey: _board.value,
        pinId: _pin.$2.value,
        toolId: _genericTool.id,
        result: _result,
      );
      final writer = ControlledClipboardWriter();
      final container = _container(
        storage,
        clipboard: writer,
        addPin: (_, _) => 'legacy-clone',
      );
      addTearDown(container.dispose);
      final robot = ModalPersistenceRobot(tester);
      await robot.pumpScreen(
        container: container,
        tool: _genericTool,
        pinKey: _pin,
      );
      robot.expectRestoredVisible();
      robot.expectStaleNotVisible();
      expect(
        find.text('Input used for this result is unavailable'),
        findsOneWidget,
      );

      await robot.editField('changed');

      robot.expectStaleNotVisible();
      expect(
        find.text('Input used for this result is unavailable'),
        findsOneWidget,
      );
      expect(storage.outcomes[_pin]!.argsJson, isNull);
      await robot.pin();
      final clone = (_board, PinId.parse('legacy-clone'));
      expect(container.read(focusedPinProvider), clone.$2);
      expect(storage.outcomes[clone]?.result, _result);
      expect(storage.outcomes[clone]?.argsJson, isNull);
      await robot.pressF2();
      expect(writer.writes, ['saved output']);
      writer.succeed();
      await tester.pump();
      await tester.pump();
      expect(
        find.text('Copied result; its input history is unavailable'),
        findsWidgets,
      );
      await robot.close();
      _flushDrafts(container);
    },
  );

  testWidgets('should close only the old modal when project state changes', (
    tester,
  ) async {
    final storage = FakePinPersistence();
    final container = _container(storage);
    addTearDown(container.dispose);
    final robot = ModalPersistenceRobot(tester);
    await robot.pumpScreen(
      container: container,
      tool: _genericTool,
      pinKey: _pin,
    );
    await robot.openSettingsRoute();

    container.read(lastOutcomeProvider.notifier).resetMemory();
    container.read(inlineDraftProvider).resetMemory();
    await tester.pumpAndSettle();

    robot.expectSettingsRouteVisible();
    robot.expectModalNotVisible();
  });
  for (final bespoke in [false, true]) {
    testWidgets(
      'should lock ${bespoke ? "bespoke" : "generic"} editing after a draft load failure until retry restores the original',
      (tester) async {
        final storage = FakePinPersistence();
        final tool = fixtureToolDto(
          id: bespoke ? 'num.hex_to_decimal' : _genericTool.id,
          inputFields: [
            InputFieldDto(
              key: bespoke ? 'input' : 'value',
              label: 'Input',
              fieldType: const InputFieldType.text(),
              required_: false,
            ),
          ],
        );
        final original = ToolArgs.fromJsonObject({
          bespoke ? 'input' : 'value': 'ff',
        });
        storage.saveDraft(_pin, ToolId.parse(tool.id), original);
        storage.failDraftLoads = true;
        var runs = 0;
        final container = _container(
          storage,
          tool: tool,
          dispatch: ({required toolId, required args, required approve}) async {
            runs++;
            return _result;
          },
        );
        addTearDown(container.dispose);
        final robot = ModalPersistenceRobot(tester);
        final fieldKey = bespoke ? 'hex-to-dec-input' : 'field-value';
        await robot.pumpScreen(container: container, tool: tool, pinKey: _pin);

        await robot.tryFocusBlockedField(key: fieldKey);
        await robot.pressF1OutsideEditor();
        expect(runs, 0);
        expect(
          ToolArgs.tryDecodeObject(storage.drafts[_pin]!.argsJson),
          original,
        );
        await robot.retrySave();
        await robot.tryFocusBlockedField(key: fieldKey);
        expect(
          ToolArgs.tryDecodeObject(storage.drafts[_pin]!.argsJson),
          original,
        );

        storage.failDraftLoads = false;
        await robot.retrySave();

        robot.expectField('ff', key: fieldKey);
        robot.expectEditingAllowed(key: fieldKey);
        await robot.editField('aa', key: fieldKey);
        _flushDrafts(container);
        expect(
          ToolArgs.tryDecodeObject(storage.drafts[_pin]!.argsJson),
          ToolArgs.fromJsonObject({bespoke ? 'input' : 'value': 'aa'}),
        );
        expect(runs, 0);
        await robot.close();
      },
    );
  }
  testWidgets(
    'should keep unpinned results when pin shortcuts are pressed during a run',
    (tester) async {
      final storage = FakePinPersistence();
      final completion = Completer<CanonicalToolResult>();
      var pins = 0;
      final container = _container(
        storage,
        dispatch: ({required toolId, required args, required approve}) =>
            completion.future,
        addPin: (_, _) => 'created-${++pins}',
      );
      addTearDown(container.dispose);
      final robot = ModalPersistenceRobot(tester);
      await robot.pumpScreen(container: container, tool: _genericTool);
      await robot.editField('run inputs');
      await robot.run(settle: false);

      robot.expectPinDisabled();
      await robot.pin(settle: false);
      await robot.pressF3();
      expect(pins, 0);
      completion.complete(_result);
      await tester.pumpAndSettle();

      robot.expectResultVisible('saved output');
      await robot.pin();
      expect(pins, 1);
      final pin = (_board, PinId.parse('created-1'));
      expect(storage.outcomes[pin]?.result, _result);
      expect(
        ToolArgs.tryDecodeObject(storage.outcomes[pin]!.argsJson!),
        _args('run inputs'),
      );
      await robot.close();
      _flushDrafts(container);
    },
  );

  for (final diagnostic in [false, true]) {
    for (final fails in [false, true]) {
      testWidgets(
        'F2 copies ${diagnostic ? "visible error" : "result"} ${fails ? "failure" : "success"} feedback only after the write finishes',
        (tester) async {
          final storage = FakePinPersistence();
          final writer = ControlledClipboardWriter();
          final diagnostics = FakeClipboardDiagnostics();
          const failureResult = CanonicalToolResult(
            ok: false,
            outputs: [],
            error: CanonicalToolError(code: 'failed', message: 'tool error'),
          );
          final container = _container(
            storage,
            clipboard: writer,
            diagnostics: diagnostics,
            streamDispatch:
                ({
                  pinKey,
                  required toolId,
                  required args,
                  required approve,
                  required runId,
                }) async* {
                  diagnostics.runId = runId.value;
                  yield DispatchStreamEventDto.done(
                    result: diagnostic ? failureResult : _result,
                  );
                },
          );
          addTearDown(container.dispose);
          final robot = ModalPersistenceRobot(tester);
          await robot.pumpScreen(container: container, tool: _genericTool);
          await robot.editField('inputs');
          await robot.run();
          if (diagnostic) {
            expect(find.text('diagnostics.detail_title'), findsOneWidget);
          }

          await robot.pressF2();

          robot.expectCopiedNotVisible();
          expect(writer.writes, [diagnostic ? 'tool error' : 'saved output']);
          if (fails) {
            writer.fail();
          } else {
            writer.succeed();
          }
          await tester.pumpAndSettle();
          if (fails) {
            robot.expectCopiedNotVisible();
            robot.expectCopyFailureVisible();
          } else {
            robot.expectCopiedVisible();
          }
          expect(diagnostics.debugExports, isEmpty);
          await robot.close();
        },
      );
    }
  }
}
