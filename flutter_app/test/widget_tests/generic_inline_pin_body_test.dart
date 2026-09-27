import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/tools.dart' as rust;
import 'package:upeg/src/rust/api/tools/input_field.dart' as rust;
import 'package:upeg/src/state/inline_draft_provider.dart';
import 'package:upeg/src/state/live_outcome_provider.dart' show LiveDispatchFn;
import 'package:upeg/src/state/pin_provider.dart' show PinKey;
import 'package:upeg/src/state/running_tools_provider.dart';
import 'package:upeg/src/state/accent.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart' show ToolArgs;
import 'package:upeg/src/widgets/inline/generic_inline_pin_body.dart';

import '../test_helpers/tool_fixture.dart';
import '../test_helpers/color_contrast.dart';
import '../test_helpers/dispatch_stream_fixture.dart';
import '../test_helpers/i18n_test_catalog.dart';

/// A two-required-input pure function tool (like text.diff): the generic
/// form renders both fields by type; the body runs on-change once both
/// validate.
rust.ToolDto _pairTool({
  rust.SourceDto source = const rust.SourceDto.userInput(),
}) => fixtureToolDto(
  id: 'text.pair',
  description: 'Join two strings.',
  source: source,
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

/// Fake dispatcher that joins the two inputs, recording each dispatched
/// (left, right) pair for assertions.
LiveDispatchFn _joinDispatch(List<String> calls) {
  return ({
    PinKey? pinKey,
    required ToolId toolId,
    required ToolArgs args,
  }) async {
    final map = args.toJsonObject();
    final joined = '${map['left']}|${map['right']}';
    calls.add(joined);
    return rust.CanonicalToolResult(
      ok: true,
      primaryOutputId: 'result',
      outputs: <rust.CanonicalOutputEntry>[
        rust.CanonicalOutputEntry(
          id: 'result',
          label: 'Joined',
          kind: 'string',
          value: rust.CanonicalOutputValue.string(value: joined),
        ),
      ],
    );
  };
}

Widget _harness(
  rust.ToolDto tool, {
  required LiveDispatchFn dispatch,
  InlineDraftStore? draftStore,
  BoardKey? boardKey,
  double height = 200,
  ThemeData? theme,
}) {
  final pinKey = (boardKey ?? BoardKey.parse('dev'), PinId.parse(tool.id));
  return ProviderScope(
    overrides: [
      // The in-flight strip labels its Cancel affordance through `t()`,
      // so the FRB i18n seam needs the test catalog here too.
      ...i18nTestOverrides,
      ...dispatchOverrides(dispatch),
      if (draftStore != null) inlineDraftProvider.overrideWithValue(draftStore),
    ],
    child: MaterialApp(
      theme: theme ?? UpegTheme.darkTheme(),
      home: Scaffold(
        body: SizedBox(
          width: 240,
          height: height,
          child: GenericInlinePinBody(tool: tool, pinKey: pinKey),
        ),
      ),
    ),
  );
}

rust.CanonicalToolResult _success(String value) => rust.CanonicalToolResult(
  ok: true,
  primaryOutputId: 'result',
  outputs: <rust.CanonicalOutputEntry>[
    rust.CanonicalOutputEntry(
      id: 'result',
      label: 'Joined',
      kind: 'string',
      value: rust.CanonicalOutputValue.string(value: value),
    ),
  ],
);

final class _DeferredDispatch {
  final List<ToolArgs> calls = <ToolArgs>[];
  final List<Completer<rust.CanonicalToolResult>> pending =
      <Completer<rust.CanonicalToolResult>>[];

  Future<rust.CanonicalToolResult> call({
    PinKey? pinKey,
    required ToolId toolId,
    required ToolArgs args,
  }) {
    calls.add(args);
    final completer = Completer<rust.CanonicalToolResult>();
    pending.add(completer);
    return completer.future;
  }

  void complete(int index, rust.CanonicalToolResult result) {
    pending[index].complete(result);
  }
}

final class _InlineBodyRobot {
  const _InlineBodyRobot(this.tester);

  final WidgetTester tester;

  Future<void> fillPair(String left, String right) async {
    await tester.enterText(find.byKey(const Key('field-left')), left);
    await tester.pump();
    await tester.enterText(find.byKey(const Key('field-right')), right);
    await tester.pump();
  }

  Future<void> changeRight(String right) async {
    await tester.enterText(find.byKey(const Key('field-right')), right);
    await tester.pump();
  }

  Future<void> waitForDebounce() async {
    await tester.pump(inlineRunDebounce + const Duration(milliseconds: 20));
    await tester.pump();
  }

  void expectNoAutoDispatch(List<Object?> calls) {
    expect(calls, isEmpty);
  }

  void expectHint(String text) {
    expect(find.text(text), findsOneWidget);
  }

  void expectOutputVisible(String value) {
    expect(find.textContaining('Joined'), findsOneWidget);
    expect(find.text(value).hitTestable(), findsOneWidget);
  }

  void expectOutputHidden(String value) {
    expect(find.text(value), findsNothing);
  }

  void expectLoading({required bool visible}) {
    expect(
      find.byKey(inlineLoadingIndicatorKey),
      visible ? findsOneWidget : findsNothing,
    );
  }

  void expectManualTargetIsAccessible() {
    final size = tester.getSize(find.byKey(inlineRunButtonKey));
    expect(size.width, greaterThanOrEqualTo(48));
    expect(size.height, greaterThanOrEqualTo(48));
  }

  void expectRunningState({required bool running}) {
    final element = tester.element(find.byType(GenericInlinePinBody));
    final container = ProviderScope.containerOf(element);
    final pinKey = tester
        .widget<GenericInlinePinBody>(find.byType(GenericInlinePinBody))
        .pinKey;
    expect(container.read(runningToolsProvider).contains(pinKey), running);
  }
}

void main() {
  group('GenericInlinePinBody', () {
    testWidgets(
      'the_run_enabled_and_disabled_colors_contrast_in_every_light_theme_accent',
      (tester) async {
        final robot = _InlineBodyRobot(tester);
        final tool = _pairTool(source: const rust.SourceDto.manual());
        for (final accent in Accent.values) {
          await tester.pumpWidget(
            _harness(
              tool,
              dispatch: _joinDispatch(<String>[]),
              theme: UpegTheme.forAccent(accent, brightness: Brightness.light),
            ),
          );
          await tester.pump();
          await robot.fillPair('left', 'right');

          final button = tester.widget<FilledButton>(
            find.byKey(inlineRunButtonKey),
          );
          expect(button.onPressed, isNotNull);
          final style = button.style!;
          for (final states in <Set<WidgetState>>[
            <WidgetState>{},
            <WidgetState>{WidgetState.hovered},
            <WidgetState>{WidgetState.focused},
            <WidgetState>{WidgetState.pressed},
            <WidgetState>{WidgetState.disabled},
          ]) {
            final foreground = style.foregroundColor!.resolve(states)!;
            final baseBackground = style.backgroundColor!.resolve(states)!;
            final overlay = style.overlayColor?.resolve(states);
            final background = overlay == null
                ? baseBackground
                : Color.alphaBlend(overlay, baseBackground);
            expect(
              colorContrastRatio(foreground, background),
              greaterThanOrEqualTo(minimumNormalTextContrastRatio),
              reason:
                  '${accent.name} Run ${states.isEmpty ? 'enabled' : 'disabled'}',
            );
          }

          await tester.pumpWidget(const SizedBox.shrink());
          await tester.pump();
        }
      },
    );

    testWidgets(
      'a_light_theme_success_result_label_contrasts_with_the_pin_background',
      (tester) async {
        final calls = <String>[];
        final robot = _InlineBodyRobot(tester);
        await tester.pumpWidget(
          _harness(
            _pairTool(),
            dispatch: _joinDispatch(calls),
            theme: UpegTheme.lightTheme(),
          ),
        );
        await tester.pump();
        await robot.fillPair('a', 'b');
        await robot.waitForDebounce();

        final label = tester.widget<Text>(find.text('Joined: '));
        final foreground = label.style!.color!;
        final background = UpegTheme.lightTheme()
            .extension<UpegTokens>()!
            .surface;
        expect(
          colorContrastRatio(foreground, background),
          greaterThanOrEqualTo(minimumNormalTextContrastRatio),
        );
      },
    );

    testWidgets('filling_every_required_field_runs_and_renders_the_output', (
      tester,
    ) async {
      final calls = <String>[];
      final robot = _InlineBodyRobot(tester);
      await tester.pumpWidget(
        _harness(_pairTool(), dispatch: _joinDispatch(calls)),
      );
      await tester.pump(); // first-frame validation
      robot.expectHint(inlineUserInputSourceHint);

      await tester.enterText(find.byKey(const Key('field-left')), 'a');
      await tester.pump();
      await tester.enterText(find.byKey(const Key('field-right')), 'b');
      await tester.pump(inlineRunDebounce + const Duration(milliseconds: 20));
      await tester.pump(); // flush async dispatch → setState

      expect(calls, <String>['a|b']);
      expect(find.text('a|b'), findsOneWidget);
    });

    testWidgets('a_missing_required_field_does_not_run', (tester) async {
      final calls = <String>[];
      await tester.pumpWidget(
        _harness(_pairTool(), dispatch: _joinDispatch(calls)),
      );
      await tester.pump();

      // Only the first field is filled; `right` is still required-empty.
      await tester.enterText(find.byKey(const Key('field-left')), 'a');
      await tester.pump(inlineRunDebounce + const Duration(milliseconds: 20));
      await tester.pump();

      expect(calls, isEmpty);
    });

    testWidgets('fields_render_by_type_so_both_input_boxes_are_drawn', (
      tester,
    ) async {
      final calls = <String>[];
      await tester.pumpWidget(
        _harness(_pairTool(), dispatch: _joinDispatch(calls)),
      );
      await tester.pump();

      // Generic form renders one keyed field per input — no template.
      expect(find.byKey(const Key('field-left')), findsOneWidget);
      expect(find.byKey(const Key('field-right')), findsOneWidget);
      // Real EditableTexts, so the board yields plain-letter keys to typing.
      expect(find.byType(EditableText), findsNWidgets(2));
    });

    testWidgets(
      'a_manual_source_never_auto_runs_and_only_runs_via_the_run_button',
      (tester) async {
        final calls = <String>[];
        await tester.pumpWidget(
          _harness(
            _pairTool(source: const rust.SourceDto.manual()),
            dispatch: _joinDispatch(calls),
          ),
        );
        await tester.pump();

        // Filling every field does NOT auto-run when the tool declares a
        // manual source.
        await tester.enterText(find.byKey(const Key('field-left')), 'a');
        await tester.enterText(find.byKey(const Key('field-right')), 'b');
        await tester.pump(inlineRunDebounce + const Duration(milliseconds: 20));
        await tester.pump();
        expect(calls, isEmpty);

        // The explicit Run button dispatches.
        await tester.tap(find.byKey(inlineRunButtonKey));
        await tester.pump();
        expect(calls, <String>['a|b']);
      },
    );

    testWidgets('typing_records_a_draft_used_to_seed_the_modal', (
      tester,
    ) async {
      final store = InlineDraftStore();
      final PinKey pinKey = (BoardKey.parse('dev'), PinId.parse('text.pair'));
      await tester.pumpWidget(
        _harness(
          _pairTool(),
          dispatch: _joinDispatch(<String>[]),
          draftStore: store,
        ),
      );
      await tester.pump();

      await tester.enterText(find.byKey(const Key('field-left')), 'a');
      await tester.enterText(find.byKey(const Key('field-right')), 'b');
      await tester.pump();

      final draft = store.read(pinKey);
      expect(draft, isNotNull);
      expect(draft!.toJsonObject()['left'], 'a');
      expect(draft.toJsonObject()['right'], 'b');
    });

    testWidgets('an_existing_draft_restores_the_fields', (tester) async {
      final PinKey pinKey = (BoardKey.parse('dev'), PinId.parse('text.pair'));
      final store = InlineDraftStore()
        ..set(
          pinKey,
          ToolArgs.fromJsonObject(const <String, Object?>{
            'left': 'x',
            'right': 'y',
          }),
        );
      await tester.pumpWidget(
        _harness(
          _pairTool(),
          dispatch: _joinDispatch(<String>[]),
          draftStore: store,
        ),
      );
      await tester.pump();

      expect(find.text('x'), findsOneWidget);
      expect(find.text('y'), findsOneWidget);
    });

    testWidgets('a_shortcut_source_does_not_auto_run_on_input_changes', (
      tester,
    ) async {
      final calls = <String>[];
      final robot = _InlineBodyRobot(tester);
      await tester.pumpWidget(
        _harness(
          _pairTool(
            source: const rust.SourceDto.shortcut(keys: 'Ctrl+Shift+P'),
          ),
          dispatch: _joinDispatch(calls),
        ),
      );
      await tester.pump();

      await robot.fillPair('a', 'b');
      await robot.waitForDebounce();

      robot.expectNoAutoDispatch(calls);
      robot.expectHint(inlineShortcutSourceHint('Ctrl+Shift+P'));
    });

    testWidgets('a_static_source_does_not_auto_run_on_input_changes', (
      tester,
    ) async {
      final calls = <String>[];
      final robot = _InlineBodyRobot(tester);
      await tester.pumpWidget(
        _harness(
          _pairTool(source: const rust.SourceDto.static_()),
          dispatch: _joinDispatch(calls),
        ),
      );
      await tester.pump();

      await robot.fillPair('a', 'b');
      await robot.waitForDebounce();

      robot.expectNoAutoDispatch(calls);
      robot.expectHint(inlineStaticSourceHint);
    });

    testWidgets('a_source_change_cancels_a_pending_debounced_run', (
      tester,
    ) async {
      final calls = <String>[];
      final robot = _InlineBodyRobot(tester);
      await tester.pumpWidget(
        _harness(_pairTool(), dispatch: _joinDispatch(calls)),
      );
      await tester.pump();
      robot.expectHint(inlineUserInputSourceHint);
      await robot.fillPair('a', 'b');

      await tester.pumpWidget(
        _harness(
          _pairTool(source: const rust.SourceDto.manual()),
          dispatch: _joinDispatch(calls),
        ),
      );
      await robot.waitForDebounce();

      robot.expectNoAutoDispatch(calls);
    });

    testWidgets('unmounting_the_pin_body_clears_its_board_draft', (
      tester,
    ) async {
      final store = InlineDraftStore();
      final PinKey pinKey = (BoardKey.parse('dev'), PinId.parse('text.pair'));
      final robot = _InlineBodyRobot(tester);
      await tester.pumpWidget(
        _harness(
          _pairTool(source: const rust.SourceDto.manual()),
          dispatch: _joinDispatch(<String>[]),
          draftStore: store,
        ),
      );
      await tester.pump();
      await robot.fillPair('a', 'b');
      expect(store.read(pinKey), isNotNull);

      await tester.pumpWidget(const SizedBox.shrink());
      await tester.pump();

      expect(store.read(pinKey), isNull);
    });

    testWidgets(
      'invalidating_the_input_while_running_discards_the_previous_result',
      (tester) async {
        final dispatch = _DeferredDispatch();
        final robot = _InlineBodyRobot(tester);
        await tester.pumpWidget(_harness(_pairTool(), dispatch: dispatch.call));
        await tester.pump();
        await robot.fillPair('a', 'b');
        await robot.waitForDebounce();
        expect(dispatch.calls, hasLength(1));

        await robot.changeRight('');
        dispatch.complete(0, _success('a|b'));
        await tester.pump();

        robot.expectOutputHidden('a|b');
        expect(dispatch.calls, hasLength(1));
      },
    );

    testWidgets(
      'valid_input_changed_while_running_reruns_once_with_the_latest_values',
      (tester) async {
        final dispatch = _DeferredDispatch();
        final robot = _InlineBodyRobot(tester);
        await tester.pumpWidget(_harness(_pairTool(), dispatch: dispatch.call));
        await tester.pump();
        await robot.fillPair('a', 'b');
        await robot.waitForDebounce();

        await robot.changeRight('c');
        await robot.waitForDebounce();
        await robot.changeRight('d');
        await robot.waitForDebounce();
        expect(dispatch.calls, hasLength(1));

        dispatch.complete(0, _success('a|b'));
        await tester.pump();
        expect(dispatch.calls, hasLength(2));
        expect(dispatch.calls.last.toJsonObject()['right'], 'd');

        dispatch.complete(1, _success('a|d'));
        await tester.pumpAndSettle();
        robot.expectOutputVisible('a|d');
      },
    );

    testWidgets(
      'a_timer_source_does_not_double_run_while_a_slow_run_is_in_flight',
      (tester) async {
        final dispatch = _DeferredDispatch();
        final robot = _InlineBodyRobot(tester);
        await tester.pumpWidget(
          _harness(
            _pairTool(
              source: rust.SourceDto.timer(intervalMs: BigInt.from(250)),
            ),
            dispatch: dispatch.call,
          ),
        );
        await tester.pump();
        robot.expectHint(inlineTimerSourceHint(BigInt.from(250)));
        await robot.fillPair('a', 'b');
        await robot.waitForDebounce();
        await tester.pump(const Duration(milliseconds: 500));

        expect(dispatch.calls, hasLength(1));
        dispatch.complete(0, _success('a|b'));
        await tester.pump();
      },
    );

    testWidgets(
      'a_timer_firing_before_the_debounce_does_not_rerun_the_same_input',
      (tester) async {
        final dispatch = _DeferredDispatch();
        final robot = _InlineBodyRobot(tester);
        await tester.pumpWidget(
          _harness(
            _pairTool(
              source: rust.SourceDto.timer(intervalMs: BigInt.from(50)),
            ),
            dispatch: dispatch.call,
          ),
        );
        await tester.pump();
        await robot.fillPair('a', 'b');

        await tester.pump(const Duration(milliseconds: 60));
        expect(dispatch.calls, hasLength(1));
        await tester.pump(const Duration(milliseconds: 80));
        expect(dispatch.calls, hasLength(1));

        dispatch.complete(0, _success('a|b'));
        await tester.pump();
        expect(dispatch.calls, hasLength(1));
      },
    );

    testWidgets(
      'a_fast_timer_run_does_not_overlap_the_same_inputs_debounced_run',
      (tester) async {
        final calls = <String>[];
        final robot = _InlineBodyRobot(tester);
        await tester.pumpWidget(
          _harness(
            _pairTool(
              source: rust.SourceDto.timer(intervalMs: BigInt.from(100)),
            ),
            dispatch: _joinDispatch(calls),
          ),
        );
        await tester.pump();
        await robot.fillPair('a', 'b');

        await tester.pump(const Duration(milliseconds: 140));
        await tester.pump();

        expect(calls, <String>['a|b']);
      },
    );

    testWidgets(
      'a_u64_max_timer_interval_shows_a_safe_hint_without_repeating',
      (tester) async {
        const rustUnsigned64BitWidth = 64;
        final rustUnsigned64Max =
            (BigInt.one << rustUnsigned64BitWidth) - BigInt.one;
        final calls = <String>[];
        final robot = _InlineBodyRobot(tester);
        await tester.pumpWidget(
          _harness(
            _pairTool(
              source: rust.SourceDto.timer(intervalMs: rustUnsigned64Max),
            ),
            dispatch: _joinDispatch(calls),
          ),
        );
        await tester.pump();

        await robot.fillPair('a', 'b');
        await tester.pump(const Duration(milliseconds: 10));

        expect(calls, isEmpty);
        expect(tester.takeException(), isNull);
        robot.expectHint(inlineInvalidTimerSourceHint);
        expect(find.textContaining(rustUnsigned64Max.toString()), findsNothing);
      },
    );

    testWidgets('a_timer_tick_during_a_run_reruns_once_with_the_latest_input', (
      tester,
    ) async {
      final dispatch = _DeferredDispatch();
      final robot = _InlineBodyRobot(tester);
      await tester.pumpWidget(
        _harness(
          _pairTool(source: rust.SourceDto.timer(intervalMs: BigInt.from(200))),
          dispatch: dispatch.call,
        ),
      );
      await tester.pump();
      await robot.fillPair('a', 'b');
      await robot.waitForDebounce();
      expect(dispatch.calls, hasLength(1));

      await robot.changeRight('c');
      await tester.pump(const Duration(milliseconds: 70));
      expect(dispatch.calls, hasLength(1));

      dispatch.complete(0, _success('a|b'));
      await tester.pump();
      expect(dispatch.calls, hasLength(2));
      expect(dispatch.calls.last.toJsonObject()['right'], 'c');

      dispatch.complete(1, _success('a|c'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 60));
      expect(dispatch.calls, hasLength(2));
    });

    testWidgets(
      'a_run_exception_is_replaced_with_a_safe_message_and_clears_the_running_state',
      (tester) async {
        final robot = _InlineBodyRobot(tester);
        await tester.pumpWidget(
          _harness(
            _pairTool(),
            dispatch: ({pinKey, required toolId, required args}) async {
              throw Exception('/home/user/secret.txt');
            },
          ),
        );
        await tester.pump();
        await robot.fillPair('a', 'b');
        await robot.waitForDebounce();
        await tester.pump();

        expect(find.text(inlineSafeDispatchErrorMessage), findsOneWidget);
        expect(find.textContaining('/home/user'), findsNothing);
        robot.expectLoading(visible: false);
        robot.expectRunningState(running: false);
      },
    );

    testWidgets(
      'a_backend_failures_detail_path_is_never_exposed_in_the_compact_body',
      (tester) async {
        final robot = _InlineBodyRobot(tester);
        await tester.pumpWidget(
          _harness(
            _pairTool(),
            dispatch: ({pinKey, required toolId, required args}) async =>
                const rust.CanonicalToolResult(
                  ok: false,
                  outputs: <rust.CanonicalOutputEntry>[],
                  error: rust.CanonicalToolError(
                    code: 'io',
                    message: 'failed at /home/user/secret.txt',
                  ),
                ),
          ),
        );
        await tester.pump();
        await robot.fillPair('a', 'b');
        await robot.waitForDebounce();

        expect(find.text(inlineSafeDispatchErrorMessage), findsOneWidget);
        expect(find.textContaining('/home/user'), findsNothing);
      },
    );

    testWidgets('a_slow_run_shows_loading_and_blocks_manual_run_reentry', (
      tester,
    ) async {
      final dispatch = _DeferredDispatch();
      final robot = _InlineBodyRobot(tester);
      await tester.pumpWidget(
        _harness(
          _pairTool(source: const rust.SourceDto.manual()),
          dispatch: dispatch.call,
        ),
      );
      await tester.pump();
      await robot.fillPair('a', 'b');
      robot.expectManualTargetIsAccessible();

      await tester.tap(find.byKey(inlineRunButtonKey));
      await tester.pump();
      robot.expectLoading(visible: true);
      robot.expectRunningState(running: true);
      await tester.tap(find.byKey(inlineRunButtonKey));
      await tester.pump();
      expect(dispatch.calls, hasLength(1));

      dispatch.complete(0, _success('a|b'));
      await tester.pumpAndSettle();
      robot.expectLoading(visible: false);
      robot.expectRunningState(running: false);
    });

    testWidgets(
      'the_result_label_and_value_are_kept_and_auto_revealed_on_a_short_pin',
      (tester) async {
        final robot = _InlineBodyRobot(tester);
        await tester.pumpWidget(
          _harness(
            _pairTool(),
            dispatch: _joinDispatch(<String>[]),
            height: 100,
          ),
        );
        await tester.pump();
        await robot.fillPair('한글', '결과');
        await robot.waitForDebounce();
        await tester.pumpAndSettle();

        robot.expectOutputVisible('한글|결과');
        final value = tester.widget<Text>(
          find.byKey(inlineOutputValueKey('result')),
        );
        expect(value.maxLines, 1);
      },
    );
  });

  group('tools without inputs', () {
    // `id.uuid_v7` is the shape that used to need a bespoke form: no
    // inputs, so nothing to trigger an on-change run.
    rust.ToolDto zeroInputTool() => fixtureToolDto(
      id: 'id.uuid_v7',
      description: 'Generate a fresh time-ordered UUID v7.',
      inputFields: const <rust.InputFieldDto>[],
      outputFields: const <rust.OutputFieldDto>[],
    );

    testWidgets('a_tool_without_inputs_can_run_directly_via_the_run_button', (
      tester,
    ) async {
      const generated = '01900000-0000-7000-8000-000000000abc';
      var calls = 0;
      await tester.pumpWidget(
        _harness(
          zeroInputTool(),
          dispatch:
              ({pinKey, required ToolId toolId, required ToolArgs args}) async {
                calls += 1;
                expect(args.isEmpty, isTrue);
                return _success(generated);
              },
        ),
      );
      await tester.pump();

      expect(find.byKey(inlineRunButtonKey), findsOneWidget);
      // Nothing ran on mount — a generator should not fire on every
      // board rebuild.
      expect(calls, 0);

      await tester.tap(find.byKey(inlineRunButtonKey));
      await tester.pumpAndSettle();

      expect(calls, 1);
      expect(find.text(generated), findsOneWidget);
    });

    testWidgets('a_tool_without_inputs_hides_the_runs_on_input_hint', (
      tester,
    ) async {
      await tester.pumpWidget(
        _harness(
          zeroInputTool(),
          dispatch:
              ({
                pinKey,
                required ToolId toolId,
                required ToolArgs args,
              }) async => _success('x'),
        ),
      );
      await tester.pump();

      expect(find.text(inlineUserInputSourceHint), findsNothing);
    });
  });
}
