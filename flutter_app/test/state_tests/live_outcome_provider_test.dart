import 'dart:async';

import 'package:fake_async/fake_async.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/platform/window_visibility.dart';
import 'package:upeg/src/rust/api/last_outcomes.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/live_outcome_provider.dart';
import 'package:upeg/src/state/last_outcome_provider.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

import '../test_helpers/fake_window_visibility_source.dart';
import '../test_helpers/tool_fixture.dart';

CanonicalToolResult _ok(String stdout, {String? structured}) =>
    CanonicalToolResult(
      ok: true,
      primaryOutputId: 'result',
      outputs: [
        CanonicalOutputEntry(
          id: 'result',
          label: 'Result',
          kind: structured == null ? 'string' : 'json',
          value: structured == null
              ? CanonicalOutputValue.string(value: stdout)
              : CanonicalOutputValue.json(value: structured),
        ),
      ],
    );

CanonicalToolResult _err(String message) => CanonicalToolResult(
  ok: false,
  outputs: const [],
  error: CanonicalToolError(code: 'dispatch_failed', message: message),
);

LivePinKey _livePin(ToolDto tool, {String pinId = 'pin'}) => (
  pinKey: (BoardKey.parse('test-board'), PinId.parse(pinId)),
  toolId: ToolId.parse(tool.id),
);

/// Plain `test()`s have no Flutter binding, so every container that reads the
/// Live provider fakes the window seam (or passes `null`, which never pauses).
ProviderContainer _container({
  required ToolDto tool,
  WindowVisibilitySource? windowSource,
  List<LastOutcomeDto> Function(String boardKey)? loadOutcomes,
  void Function(String, String, String, CanonicalToolResult, String?)?
  persistOutcome,
  required FutureOr<CanonicalToolResult> Function({
    PinKey? pinKey,
    required ToolId toolId,
    required ToolArgs args,
  })
  dispatch,
}) {
  final container = ProviderContainer(
    overrides: [
      windowVisibilitySourceProvider.overrideWithValue(windowSource),
      toolByIdProvider(ToolId.parse(tool.id)).overrideWith((ref) => tool),
      lastOutcomeLoadProvider.overrideWithValue(
        loadOutcomes ?? (_) => const <LastOutcomeDto>[],
      ),
      lastOutcomePersistProvider.overrideWithValue(
        ({
          required boardKey,
          required pinId,
          required toolId,
          required result,
          argsJson,
        }) => persistOutcome?.call(boardKey, pinId, toolId, result, argsJson),
      ),
      liveDispatchToolFnProvider.overrideWithValue(({
        PinKey? pinKey,
        required toolId,
        required args,
      }) async {
        return dispatch(pinKey: pinKey, toolId: toolId, args: args);
      }),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

void main() {
  test('userinput_source_pin_stays_pending_and_never_dispatches', () {
    final calls = <String>[];
    final tool = fixtureToolDto(id: 'test.userinput', pinKind: PinKindDto.live);
    final container = _container(
      tool: tool,
      dispatch: ({pinKey, required toolId, required args}) {
        calls.add(toolId.value);
        return _ok('never');
      },
    );

    final state = container.read(liveOutcomeProvider(_livePin(tool)));
    expect(state, isA<LiveOutcomePending>());
    expect(calls, isEmpty);
  });

  test('timer_source_pin_dispatches_on_every_interval', () {
    fakeAsync((async) {
      var counter = 0;
      final savedArgs = <String?>[];
      final tool = fixtureToolDto(
        id: 'test.ticker',
        pinKind: PinKindDto.live,
        source: SourceDto.timer(intervalMs: BigInt.from(100)),
      );
      final container = _container(
        tool: tool,
        persistOutcome: (_, _, _, _, argsJson) => savedArgs.add(argsJson),
        dispatch: ({pinKey, required toolId, required args}) {
          expect(args, ToolArgs.empty);
          counter++;
          return _ok('$counter', structured: '{"v":$counter}');
        },
      );

      final livePin = _livePin(tool);
      container.listen(liveOutcomeProvider(livePin), (_, _) {});
      expect(
        container.read(liveOutcomeProvider(livePin)),
        isA<LiveOutcomePending>(),
      );

      async.elapse(Duration.zero);
      async.flushMicrotasks();
      expect(
        container.read(liveOutcomeProvider(livePin)),
        isA<LiveOutcomeFresh>(),
      );
      expect(counter, 1);
      expect(savedArgs, ['{}']);

      async.elapse(const Duration(milliseconds: 350));
      async.flushMicrotasks();
      // 1 cold + 3 periodic ticks at 100/200/300 ms.
      expect(counter, 4);
      expect(savedArgs, ['{}', '{}', '{}', '{}']);
    });
  });

  test('saved_timer_result_shows_before_the_cold_start_poll', () {
    fakeAsync((async) {
      var calls = 0;
      final tool = fixtureToolDto(
        id: 'test.restored-timer',
        pinKind: PinKindDto.live,
        source: SourceDto.timer(intervalMs: BigInt.from(100)),
      );
      final container = _container(
        tool: tool,
        loadOutcomes: (_) => [
          LastOutcomeDto(
            pinId: 'pin',
            toolId: tool.id,
            argsJson: '{}',
            result: _ok('saved'),
            truncated: false,
            updatedAtMs: 123,
          ),
        ],
        dispatch: ({pinKey, required toolId, required args}) {
          calls++;
          return _ok('polled');
        },
      );
      final livePin = _livePin(tool);
      container.listen(liveOutcomeProvider(livePin), (_, _) {});
      async.flushMicrotasks();

      final beforePoll = container.read(liveOutcomeProvider(livePin));
      expect(beforePoll, isA<LiveOutcomeStale>());
      expect(
        (beforePoll as LiveOutcomeStale).lastOutcome.primaryOutputText,
        'saved',
      );
      expect(calls, 0);

      async.elapse(Duration.zero);
      async.flushMicrotasks();
      final afterPoll = container.read(liveOutcomeProvider(livePin));
      expect(afterPoll, isA<LiveOutcomeFresh>());
      expect(
        (afterPoll as LiveOutcomeFresh).outcome.primaryOutputText,
        'polled',
      );
      expect(calls, 1);
    });
  });

  test('dispatch_failure_keeps_last_success_and_marks_stale', () {
    fakeAsync((async) {
      final outcomes = <CanonicalToolResult>[
        _ok('v1', structured: '{"v":1}'),
        _ok('v2', structured: '{"v":2}'),
        _err('boom'),
      ];
      var idx = 0;
      final persisted = <CanonicalToolResult>[];
      final tool = fixtureToolDto(
        id: 'test.staler',
        pinKind: PinKindDto.live,
        source: SourceDto.timer(intervalMs: BigInt.from(100)),
      );
      final container = _container(
        tool: tool,
        persistOutcome: (_, _, _, result, _) => persisted.add(result),
        dispatch: ({pinKey, required toolId, required args}) =>
            outcomes[idx < outcomes.length ? idx++ : outcomes.length - 1],
      );

      final livePin = _livePin(tool);
      container.listen(liveOutcomeProvider(livePin), (_, _) {});

      async.elapse(Duration.zero); // cold: v1
      async.flushMicrotasks();
      async.elapse(const Duration(milliseconds: 100)); // v2
      async.flushMicrotasks();
      async.elapse(const Duration(milliseconds: 100)); // err → stale(v2)
      async.flushMicrotasks();

      final state = container.read(liveOutcomeProvider(livePin));
      expect(state, isA<LiveOutcomeStale>());
      final stale = state as LiveOutcomeStale;
      expect(stale.lastOutcome.primaryOutputText, '{"v":2}');
      expect(stale.errorMessage, 'boom');
      expect(persisted, [outcomes[0], outcomes[1]]);
    });
  });

  test('first_dispatch_failure_shows_failed', () {
    fakeAsync((async) {
      final tool = fixtureToolDto(
        id: 'test.coldfail',
        pinKind: PinKindDto.live,
        source: SourceDto.timer(intervalMs: BigInt.from(100)),
      );
      final container = _container(
        tool: tool,
        dispatch: ({pinKey, required toolId, required args}) =>
            _err('cold boom'),
      );

      final livePin = _livePin(tool);
      container.listen(liveOutcomeProvider(livePin), (_, _) {});
      expect(
        container.read(liveOutcomeProvider(livePin)),
        isA<LiveOutcomePending>(),
        reason: 'pending for one frame before the first dispatch lands',
      );
      async.elapse(Duration.zero);
      async.flushMicrotasks();

      final state = container.read(liveOutcomeProvider(livePin));
      expect(state, isA<LiveOutcomeFailed>());
      expect((state as LiveOutcomeFailed).errorMessage, 'cold boom');
    });
  });

  test('failed_pin_recovers_to_fresh_on_the_next_success', () {
    fakeAsync((async) {
      final outcomes = <CanonicalToolResult>[
        _err('cold boom'),
        _err('still boom'),
        _ok('v1', structured: '{"v":1}'),
        _err('boom again'),
      ];
      var idx = 0;
      final tool = fixtureToolDto(
        id: 'test.recover',
        pinKind: PinKindDto.live,
        source: SourceDto.timer(intervalMs: BigInt.from(100)),
      );
      final container = _container(
        tool: tool,
        dispatch: ({pinKey, required toolId, required args}) =>
            outcomes[idx < outcomes.length ? idx++ : outcomes.length - 1],
      );
      final livePin = _livePin(tool);
      container.listen(liveOutcomeProvider(livePin), (_, _) {});

      async.elapse(Duration.zero); // cold: failed
      async.flushMicrotasks();
      expect(
        container.read(liveOutcomeProvider(livePin)),
        isA<LiveOutcomeFailed>(),
      );

      async.elapse(const Duration(milliseconds: 100)); // failed again
      async.flushMicrotasks();
      final again = container.read(liveOutcomeProvider(livePin));
      expect(again, isA<LiveOutcomeFailed>());
      expect((again as LiveOutcomeFailed).errorMessage, 'still boom');

      async.elapse(const Duration(milliseconds: 100)); // success
      async.flushMicrotasks();
      expect(
        container.read(liveOutcomeProvider(livePin)),
        isA<LiveOutcomeFresh>(),
      );

      async.elapse(const Duration(milliseconds: 100)); // failure after success
      async.flushMicrotasks();
      expect(
        container.read(liveOutcomeProvider(livePin)),
        isA<LiveOutcomeStale>(),
        reason: 'once a success exists, later failures stay stale',
      );
    });
  });

  test('first_failure_after_a_saved_result_stays_stale', () {
    fakeAsync((async) {
      final tool = fixtureToolDto(
        id: 'test.saved-then-fail',
        pinKind: PinKindDto.live,
        source: SourceDto.timer(intervalMs: BigInt.from(100)),
      );
      final container = _container(
        tool: tool,
        loadOutcomes: (_) => [
          LastOutcomeDto(
            pinId: 'pin',
            toolId: tool.id,
            argsJson: '{}',
            result: _ok('saved'),
            truncated: false,
            updatedAtMs: 123,
          ),
        ],
        dispatch: ({pinKey, required toolId, required args}) =>
            _err('cold boom'),
      );
      final livePin = _livePin(tool);
      container.listen(liveOutcomeProvider(livePin), (_, _) {});

      async.elapse(Duration.zero);
      async.flushMicrotasks();

      final state = container.read(liveOutcomeProvider(livePin));
      expect(state, isA<LiveOutcomeStale>());
      expect(
        (state as LiveOutcomeStale).lastOutcome.primaryOutputText,
        'saved',
      );
      expect(state.errorMessage, 'cold boom');
    });
  });

  test('hidden_window_skips_ticks', () {
    fakeAsync((async) {
      var calls = 0;
      final source = FakeWindowVisibilitySource();
      final tool = fixtureToolDto(
        id: 'test.hidden',
        pinKind: PinKindDto.live,
        source: SourceDto.timer(intervalMs: BigInt.from(100)),
      );
      final container = _container(
        tool: tool,
        windowSource: source,
        dispatch: ({pinKey, required toolId, required args}) {
          calls++;
          return _ok('$calls');
        },
      );
      container.listen(liveOutcomeProvider(_livePin(tool)), (_, _) {});
      async.elapse(Duration.zero);
      async.flushMicrotasks();
      expect(calls, 1, reason: 'the shown window runs the cold-start fire');

      source.hide();
      async.elapse(const Duration(seconds: 3));
      async.flushMicrotasks();

      expect(calls, 1, reason: 'no dispatch while the window is hidden');
    });
  });

  test('minimized_window_skips_ticks', () {
    fakeAsync((async) {
      var calls = 0;
      final source = FakeWindowVisibilitySource();
      final tool = fixtureToolDto(
        id: 'test.minimized',
        pinKind: PinKindDto.live,
        source: SourceDto.timer(intervalMs: BigInt.from(100)),
      );
      final container = _container(
        tool: tool,
        windowSource: source,
        dispatch: ({pinKey, required toolId, required args}) {
          calls++;
          return _ok('$calls');
        },
      );
      container.listen(liveOutcomeProvider(_livePin(tool)), (_, _) {});
      async.elapse(Duration.zero);
      async.flushMicrotasks();

      source.minimize();
      async.elapse(const Duration(seconds: 3));
      async.flushMicrotasks();

      expect(calls, 1);
    });
  });

  test('reshow_fires_immediately', () {
    fakeAsync((async) {
      var calls = 0;
      final source = FakeWindowVisibilitySource();
      final tool = fixtureToolDto(
        id: 'test.reshow',
        pinKind: PinKindDto.live,
        source: SourceDto.timer(intervalMs: BigInt.from(100)),
      );
      final container = _container(
        tool: tool,
        windowSource: source,
        dispatch: ({pinKey, required toolId, required args}) {
          calls++;
          return _ok('$calls');
        },
      );
      // The window is already hidden when the Live pin appears.
      container.read(windowShownProvider);
      source.hide();
      final livePin = _livePin(tool);
      container.listen(liveOutcomeProvider(livePin), (_, _) {});
      async.elapse(const Duration(seconds: 1));
      async.flushMicrotasks();
      expect(calls, 0, reason: 'the cold-start fire is skipped while hidden');
      expect(
        container.read(liveOutcomeProvider(livePin)),
        isA<LiveOutcomePending>(),
      );

      source.show();
      async.flushMicrotasks();

      expect(calls, 1, reason: 'one dispatch at show, before any tick');
      expect(
        container.read(liveOutcomeProvider(livePin)),
        isA<LiveOutcomeFresh>(),
      );

      async.elapse(const Duration(milliseconds: 100));
      async.flushMicrotasks();
      expect(calls, 2, reason: 'ticks resume after the catch-up');
    });
  });

  test('maximize_after_minimize_fires_immediately', () {
    // Windows brings a minimized, maximized window back as `maximize`, not
    // `restore`; Live ticks must not stay paused for it.
    fakeAsync((async) {
      var calls = 0;
      final source = FakeWindowVisibilitySource();
      final tool = fixtureToolDto(
        id: 'test.maximize',
        pinKind: PinKindDto.live,
        source: SourceDto.timer(intervalMs: BigInt.from(100)),
      );
      final container = _container(
        tool: tool,
        windowSource: source,
        dispatch: ({pinKey, required toolId, required args}) {
          calls++;
          return _ok('$calls');
        },
      );
      container.listen(liveOutcomeProvider(_livePin(tool)), (_, _) {});
      async.elapse(Duration.zero);
      async.flushMicrotasks();
      expect(calls, 1);

      source.minimize();
      async.elapse(const Duration(seconds: 3));
      async.flushMicrotasks();
      expect(calls, 1, reason: 'no dispatch while minimized');

      source.maximize();
      async.flushMicrotasks();
      expect(calls, 2, reason: 'one dispatch when the window is back');

      async.elapse(const Duration(milliseconds: 100));
      async.flushMicrotasks();
      expect(calls, 3, reason: 'ticks resume after the catch-up');
    });
  });

  test('non_timer_pin_never_reads_the_window_state', () {
    final source = FakeWindowVisibilitySource();
    final tool = fixtureToolDto(id: 'test.manual', pinKind: PinKindDto.live);
    final container = _container(
      tool: tool,
      windowSource: source,
      dispatch: ({pinKey, required toolId, required args}) => _ok('never'),
    );

    expect(
      container.read(liveOutcomeProvider(_livePin(tool))),
      isA<LiveOutcomePending>(),
    );
    expect(source.listeners, isEmpty);
  });

  test('provider_dispose_cancels_timer_and_stops_dispatching', () {
    fakeAsync((async) {
      var counter = 0;
      final tool = fixtureToolDto(
        id: 'test.disposer',
        pinKind: PinKindDto.live,
        source: SourceDto.timer(intervalMs: BigInt.from(100)),
      );
      final container = ProviderContainer(
        overrides: [
          windowVisibilitySourceProvider.overrideWithValue(null),
          toolByIdProvider(ToolId.parse(tool.id)).overrideWith((ref) => tool),
          lastOutcomeLoadProvider.overrideWithValue((_) => const []),
          lastOutcomePersistProvider.overrideWithValue(
            ({
              required boardKey,
              required pinId,
              required toolId,
              required result,
              argsJson,
            }) {},
          ),
          liveDispatchToolFnProvider.overrideWithValue(({
            PinKey? pinKey,
            required toolId,
            required args,
          }) async {
            counter++;
            return _ok('');
          }),
        ],
      );

      final livePin = _livePin(tool);
      final sub = container.listen(liveOutcomeProvider(livePin), (_, _) {});
      async.elapse(Duration.zero);
      async.flushMicrotasks();
      async.elapse(const Duration(milliseconds: 100));
      async.flushMicrotasks();
      expect(counter, 2); // cold + 1 tick

      sub.close();
      async.flushMicrotasks();
      // autoDispose disposes the provider when the last subscriber leaves.
      async.elapse(const Duration(milliseconds: 1000));
      expect(counter, 2, reason: 'timer must cancel on autoDispose');

      container.dispose();
    });
  });

  test('dispose_before_cold_timer_runs_never_dispatches', () {
    fakeAsync((async) {
      var counter = 0;
      final tool = fixtureToolDto(
        id: 'test.cold-dispose',
        pinKind: PinKindDto.live,
        source: SourceDto.timer(intervalMs: BigInt.from(100)),
      );
      final container = ProviderContainer(
        overrides: [
          windowVisibilitySourceProvider.overrideWithValue(null),
          toolByIdProvider(ToolId.parse(tool.id)).overrideWith((ref) => tool),
          lastOutcomeLoadProvider.overrideWithValue((_) => const []),
          lastOutcomePersistProvider.overrideWithValue(
            ({
              required boardKey,
              required pinId,
              required toolId,
              required result,
              argsJson,
            }) {},
          ),
          liveDispatchToolFnProvider.overrideWithValue(({
            PinKey? pinKey,
            required toolId,
            required args,
          }) async {
            counter++;
            return _ok('late');
          }),
        ],
      );

      final livePin = _livePin(tool);
      container.listen(liveOutcomeProvider(livePin), (_, _) {});
      container.dispose();
      async.elapse(Duration.zero);
      async.flushMicrotasks();

      expect(counter, 0);
    });
  });

  test(
    'future_completing_after_dispose_updates_shared_cache_not_live_state',
    () {
      fakeAsync((async) {
        var calls = 0;
        final savedArgs = <String?>[];
        final completer = Completer<CanonicalToolResult>();
        final observed = <LiveOutcomeState>[];
        final tool = fixtureToolDto(
          id: 'test.future-after-dispose',
          pinKind: PinKindDto.live,
          source: SourceDto.timer(intervalMs: BigInt.from(100)),
        );
        final container = _container(
          tool: tool,
          persistOutcome: (_, _, _, _, argsJson) => savedArgs.add(argsJson),
          dispatch: ({pinKey, required toolId, required args}) {
            calls++;
            return completer.future;
          },
        );

        final livePin = _livePin(tool);
        final sub = container.listen(liveOutcomeProvider(livePin), (_, next) {
          observed.add(next);
        });
        async.elapse(Duration.zero);
        expect(calls, 1);
        expect(completer.isCompleted, isFalse);

        sub.close();
        container.invalidate(liveOutcomeProvider(livePin));
        async.flushMicrotasks();
        completer.complete(_ok('late'));
        async.flushMicrotasks();

        expect(observed.whereType<LiveOutcomeFresh>(), isEmpty);
        expect(savedArgs, ['{}']);
        final saved =
            container.read(pinLastOutcomeProvider(livePin.pinKey))!
                as FreshOutcome;
        expect(saved.result.primaryOutputText, 'late');
        expect(saved.args, ToolArgs.empty);
      });
    },
  );

  test('a newer shared run makes an older timer result stale', () {
    fakeAsync((async) {
      var persistCalls = 0;
      final completer = Completer<CanonicalToolResult>();
      final tool = fixtureToolDto(
        id: 'test.live-order',
        pinKind: PinKindDto.live,
        source: SourceDto.timer(intervalMs: BigInt.from(100)),
      );
      final container = _container(
        tool: tool,
        persistOutcome: (_, _, _, _, _) => persistCalls++,
        dispatch: ({pinKey, required toolId, required args}) =>
            completer.future,
      );
      final livePin = _livePin(tool);
      container.listen(liveOutcomeProvider(livePin), (_, _) {});
      async.elapse(Duration.zero);
      async.flushMicrotasks();

      final last = container.read(lastOutcomeProvider.notifier);
      final newerToken = last.beginRun(livePin.pinKey);
      last.record(
        livePin.pinKey,
        livePin.toolId,
        _ok('newer user result'),
        args: ToolArgs.fromJsonObject(const <String, Object?>{'value': 1}),
        runToken: newerToken,
      );
      final persistCallsAfterNewer = persistCalls;

      completer.complete(_ok('older timer result'));
      async.flushMicrotasks();

      expect(
        container
            .read(pinLastOutcomeProvider(livePin.pinKey))!
            .result
            .primaryOutputText,
        'newer user result',
      );
      expect(persistCalls, persistCallsAfterNewer);
      expect(
        container.read(liveOutcomeProvider(livePin)),
        isA<LiveOutcomePending>(),
      );
    });
  });

  test(
    'reset_run_tokens_after_context_change_rejects_a_late_disposed_save',
    () {
      fakeAsync((async) {
        var persistCalls = 0;
        final completer = Completer<CanonicalToolResult>();
        final tool = fixtureToolDto(
          id: 'test.live-project-reset',
          pinKind: PinKindDto.live,
          source: SourceDto.timer(intervalMs: BigInt.from(100)),
        );
        final container = _container(
          tool: tool,
          persistOutcome: (_, _, _, _, _) => persistCalls++,
          dispatch: ({pinKey, required toolId, required args}) =>
              completer.future,
        );
        final livePin = _livePin(tool);
        final subscription = container.listen(
          liveOutcomeProvider(livePin),
          (_, _) {},
        );
        async.elapse(Duration.zero);
        expect(completer.isCompleted, isFalse);

        subscription.close();
        async.flushMicrotasks();
        container.read(lastOutcomeProvider.notifier).resetMemory();
        completer.complete(_ok('old project result'));
        async.flushMicrotasks();

        expect(persistCalls, 0);
        expect(container.read(pinLastOutcomeProvider(livePin.pinKey)), isNull);
      });
    },
  );

  test('skips_overlapping_ticks_while_previous_dispatch_is_in_flight', () {
    fakeAsync((async) {
      final completers = <Completer<CanonicalToolResult>>[];
      final tool = fixtureToolDto(
        id: 'test.overlap',
        pinKind: PinKindDto.live,
        source: SourceDto.timer(intervalMs: BigInt.from(100)),
      );
      final container = _container(
        tool: tool,
        dispatch: ({pinKey, required toolId, required args}) {
          final completer = Completer<CanonicalToolResult>();
          completers.add(completer);
          return completer.future;
        },
      );

      final livePin = _livePin(tool);
      container.listen(liveOutcomeProvider(livePin), (_, _) {});
      async.elapse(Duration.zero);
      async.elapse(const Duration(milliseconds: 350));
      expect(completers, hasLength(1));

      completers.single.complete(_ok('first'));
      async.flushMicrotasks();
      async.elapse(const Duration(milliseconds: 100));

      expect(completers, hasLength(2));
    });
  });

  test('timer dispatches are keyed by placement, even for one tool', () async {
    final toolId = ToolId.parse('clock.now');
    final first = (BoardKey.parse('dev'), PinId.parse('pin-a'));
    final second = (BoardKey.parse('dev'), PinId.parse('pin-b'));
    final calls = <PinKey>[];
    final tool = fixtureToolDto(
      id: toolId.value,
      pinKind: PinKindDto.live,
      source: SourceDto.timer(intervalMs: BigInt.from(10000)),
    );
    final container = ProviderContainer(
      overrides: [
        windowVisibilitySourceProvider.overrideWithValue(null),
        lastOutcomeLoadProvider.overrideWithValue((_) => const []),
        lastOutcomePersistProvider.overrideWithValue(
          ({
            required boardKey,
            required pinId,
            required toolId,
            required result,
            argsJson,
          }) {},
        ),
        toolsLoaderProvider.overrideWith(
          (_) =>
              () => [tool],
        ),
        liveDispatchToolFnProvider.overrideWithValue(({
          PinKey? pinKey,
          required toolId,
          required ToolArgs args,
        }) async {
          calls.add(pinKey!);
          return CanonicalToolResult(ok: true, outputs: const []);
        }),
      ],
    );
    addTearDown(container.dispose);
    await container.read(toolsProvider.future);
    container.listen(
      liveOutcomeProvider((pinKey: first, toolId: toolId)),
      (_, _) {},
    );
    container.listen(
      liveOutcomeProvider((pinKey: second, toolId: toolId)),
      (_, _) {},
    );
    await Future<void>.delayed(const Duration(milliseconds: 20));
    expect(calls, containsAll(<PinKey>[first, second]));
  });
}
