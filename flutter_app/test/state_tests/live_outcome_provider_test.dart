import 'dart:async';

import 'package:fake_async/fake_async.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/live_outcome_provider.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

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

ProviderContainer _container({
  required ToolDto tool,
  required FutureOr<CanonicalToolResult> Function({
    required ToolId toolId,
    required ToolArgs args,
  })
  dispatch,
}) {
  final container = ProviderContainer(
    overrides: [
      toolByIdProvider(ToolId.parse(tool.id)).overrideWith((ref) => tool),
      liveDispatchToolFnProvider.overrideWithValue(({
        required toolId,
        required args,
      }) async {
        return dispatch(toolId: toolId, args: args);
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
      dispatch: ({required toolId, required args}) {
        calls.add(toolId.value);
        return _ok('never');
      },
    );

    final state = container.read(liveOutcomeProvider(ToolId.parse(tool.id)));
    expect(state, isA<LiveOutcomePending>());
    expect(calls, isEmpty);
  });

  test('timer_source_pin_dispatches_on_every_interval', () {
    fakeAsync((async) {
      var counter = 0;
      final tool = fixtureToolDto(
        id: 'test.ticker',
        pinKind: PinKindDto.live,
        source: SourceDto.timer(intervalMs: BigInt.from(100)),
      );
      final container = _container(
        tool: tool,
        dispatch: ({required toolId, required args}) {
          counter++;
          return _ok('$counter', structured: '{"v":$counter}');
        },
      );

      final id = ToolId.parse(tool.id);
      container.listen(liveOutcomeProvider(id), (_, _) {});
      expect(
        container.read(liveOutcomeProvider(id)),
        isA<LiveOutcomePending>(),
      );

      async.elapse(Duration.zero);
      async.flushMicrotasks();
      expect(container.read(liveOutcomeProvider(id)), isA<LiveOutcomeFresh>());
      expect(counter, 1);

      async.elapse(const Duration(milliseconds: 350));
      async.flushMicrotasks();
      // 1 cold + 3 periodic ticks at 100/200/300 ms.
      expect(counter, 4);
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
      final tool = fixtureToolDto(
        id: 'test.staler',
        pinKind: PinKindDto.live,
        source: SourceDto.timer(intervalMs: BigInt.from(100)),
      );
      final container = _container(
        tool: tool,
        dispatch: ({required toolId, required args}) =>
            outcomes[idx < outcomes.length ? idx++ : outcomes.length - 1],
      );

      final id = ToolId.parse(tool.id);
      container.listen(liveOutcomeProvider(id), (_, _) {});

      async.elapse(Duration.zero); // cold: v1
      async.flushMicrotasks();
      async.elapse(const Duration(milliseconds: 100)); // v2
      async.flushMicrotasks();
      async.elapse(const Duration(milliseconds: 100)); // err → stale(v2)
      async.flushMicrotasks();

      final state = container.read(liveOutcomeProvider(id));
      expect(state, isA<LiveOutcomeStale>());
      final stale = state as LiveOutcomeStale;
      expect(stale.lastOutcome.primaryOutputText, '{"v":2}');
      expect(stale.errorMessage, 'boom');
    });
  });

  test('first_dispatch_failure_stays_pending', () {
    fakeAsync((async) {
      final tool = fixtureToolDto(
        id: 'test.coldfail',
        pinKind: PinKindDto.live,
        source: SourceDto.timer(intervalMs: BigInt.from(100)),
      );
      final container = _container(
        tool: tool,
        dispatch: ({required toolId, required args}) => _err('cold boom'),
      );

      final id = ToolId.parse(tool.id);
      container.listen(liveOutcomeProvider(id), (_, _) {});
      async.elapse(Duration.zero);
      async.flushMicrotasks();
      expect(
        container.read(liveOutcomeProvider(id)),
        isA<LiveOutcomePending>(),
      );
    });
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
          toolByIdProvider(ToolId.parse(tool.id)).overrideWith((ref) => tool),
          liveDispatchToolFnProvider.overrideWithValue(({
            required toolId,
            required args,
          }) async {
            counter++;
            return _ok('');
          }),
        ],
      );

      final id = ToolId.parse(tool.id);
      final sub = container.listen(liveOutcomeProvider(id), (_, _) {});
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
          toolByIdProvider(ToolId.parse(tool.id)).overrideWith((ref) => tool),
          liveDispatchToolFnProvider.overrideWithValue(({
            required toolId,
            required args,
          }) async {
            counter++;
            return _ok('late');
          }),
        ],
      );

      final id = ToolId.parse(tool.id);
      container.listen(liveOutcomeProvider(id), (_, _) {});
      container.dispose();
      async.elapse(Duration.zero);
      async.flushMicrotasks();

      expect(counter, 0);
    });
  });

  test('future_completing_after_dispose_does_not_update_state', () {
    fakeAsync((async) {
      var calls = 0;
      final completer = Completer<CanonicalToolResult>();
      final observed = <LiveOutcomeState>[];
      final tool = fixtureToolDto(
        id: 'test.future-after-dispose',
        pinKind: PinKindDto.live,
        source: SourceDto.timer(intervalMs: BigInt.from(100)),
      );
      final container = _container(
        tool: tool,
        dispatch: ({required toolId, required args}) {
          calls++;
          return completer.future;
        },
      );

      final id = ToolId.parse(tool.id);
      final sub = container.listen(liveOutcomeProvider(id), (_, next) {
        observed.add(next);
      });
      async.elapse(Duration.zero);
      expect(calls, 1);
      expect(completer.isCompleted, isFalse);

      sub.close();
      async.flushMicrotasks();
      completer.complete(_ok('late'));
      async.flushMicrotasks();

      expect(observed.whereType<LiveOutcomeFresh>(), isEmpty);
    });
  });

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
        dispatch: ({required toolId, required args}) {
          final completer = Completer<CanonicalToolResult>();
          completers.add(completer);
          return completer.future;
        },
      );

      final id = ToolId.parse(tool.id);
      container.listen(liveOutcomeProvider(id), (_, _) {});
      async.elapse(Duration.zero);
      async.elapse(const Duration(milliseconds: 350));
      expect(completers, hasLength(1));

      completers.single.complete(_ok('first'));
      async.flushMicrotasks();
      async.elapse(const Duration(milliseconds: 100));

      expect(completers, hasLength(2));
    });
  });
}
