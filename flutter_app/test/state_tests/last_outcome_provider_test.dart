/// Unit tests for the write-through + hydrating [`lastOutcomeProvider`].
///
/// Covers the persistence contract added with store schema v3:
/// `record()` writes through to the FRB `recordLastOutcome` seam exactly
/// once per call (ok and error results alike), and board selection
/// hydrates persisted rows back as [RestoredOutcome] without ever
/// overwriting a session-fresh entry.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/last_outcomes.dart' show LastOutcomeDto;
import 'package:upeg/src/rust/api/tools.dart' as rust_tools;
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/last_outcome_provider.dart';

import '../test_helpers/pegboard_selection_overrides.dart';

const _okResult = rust_tools.CanonicalToolResult(
  ok: true,
  primaryOutputId: 'value',
  outputs: [
    rust_tools.CanonicalOutputEntry(
      id: 'value',
      label: 'Value',
      kind: 'string',
      value: rust_tools.CanonicalOutputValue.string(value: '42'),
    ),
  ],
);

const _errorResult = rust_tools.CanonicalToolResult(
  ok: false,
  outputs: [],
  error: rust_tools.CanonicalToolError(code: 'E_FAIL', message: 'boom'),
);

final class _PersistCall {
  const _PersistCall(this.boardKey, this.toolId, this.result);
  final String boardKey;
  final String toolId;
  final rust_tools.CanonicalToolResult result;
}

ProviderContainer _makeContainer({
  List<_PersistCall>? persistCalls,
  List<LastOutcomeDto> Function(String boardKey)? load,
}) {
  final container = ProviderContainer(
    overrides: [
      ...pegboardSelectionOverrides(),
      lastOutcomePersistProvider.overrideWithValue(
        ({required boardKey, required toolId, required result}) =>
            persistCalls?.add(_PersistCall(boardKey, toolId, result)),
      ),
      lastOutcomeLoadProvider.overrideWithValue(load ?? (_) => const []),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

void main() {
  final toolId = ToolId.parse('num.hex_to_decimal');

  group('lastOutcomeProvider write-through', () {
    test('record_stores_fresh_state_and_calls_write_through_exactly_once', () {
      final calls = <_PersistCall>[];
      final container = _makeContainer(persistCalls: calls);
      container
          .read(currentBoardKeyProvider.notifier)
          .select(BoardKey.parse('dev'));

      container.read(lastOutcomeProvider.notifier).record(toolId, _okResult);

      final cached = container.read(lastOutcomeProvider)[toolId];
      expect(cached, isA<FreshOutcome>());
      expect(cached?.result, _okResult);
      expect(calls, hasLength(1));
      expect(calls.single.boardKey, 'dev');
      expect(calls.single.toolId, toolId.value);
      expect(calls.single.result, _okResult);
    });

    test('error_results_are_written_through_to_the_store_too', () {
      final calls = <_PersistCall>[];
      final container = _makeContainer(persistCalls: calls);
      container
          .read(currentBoardKeyProvider.notifier)
          .select(BoardKey.parse('dev'));

      container.read(lastOutcomeProvider.notifier).record(toolId, _errorResult);

      expect(calls, hasLength(1));
      expect(calls.single.result.ok, isFalse);
      // The cache keeps the error too — rendering policy (ok-only
      // inline) belongs to consumers, not the cache.
      expect(container.read(lastOutcomeProvider)[toolId]?.result.ok, isFalse);
    });

    test('without_selected_board_updates_cache_only_and_skips_persistence', () {
      final calls = <_PersistCall>[];
      final container = _makeContainer(persistCalls: calls);

      container.read(lastOutcomeProvider.notifier).record(toolId, _okResult);

      expect(container.read(lastOutcomeProvider)[toolId], isA<FreshOutcome>());
      expect(calls, isEmpty);
    });

    test('write_through_failure_keeps_cache_without_throwing', () {
      final container = ProviderContainer(
        overrides: [
          ...pegboardSelectionOverrides(),
          lastOutcomePersistProvider.overrideWithValue(
            ({required boardKey, required toolId, required result}) =>
                throw StateError('store unavailable'),
          ),
          lastOutcomeLoadProvider.overrideWithValue((_) => const []),
        ],
      );
      addTearDown(container.dispose);
      container
          .read(currentBoardKeyProvider.notifier)
          .select(BoardKey.parse('dev'));

      container.read(lastOutcomeProvider.notifier).record(toolId, _okResult);

      expect(container.read(lastOutcomeProvider)[toolId], isA<FreshOutcome>());
    });
  });

  group('lastOutcomeProvider hydration', () {
    final storedRow = LastOutcomeDto(
      toolId: toolId.value,
      result: _okResult,
      truncated: true,
      updatedAtMs: DateTime.utc(2026, 7, 17).millisecondsSinceEpoch,
    );

    test('board_switch_restores_stored_results_as_restored_state', () {
      final container = _makeContainer(
        load: (boardKey) => boardKey == 'dev' ? [storedRow] : const [],
      );
      // Scenario: the board changes while the provider is alive.
      container.read(lastOutcomeProvider);
      container
          .read(currentBoardKeyProvider.notifier)
          .select(BoardKey.parse('dev'));

      final restored = container.read(lastOutcomeProvider)[toolId];
      expect(restored, isA<RestoredOutcome>());
      final outcome = restored! as RestoredOutcome;
      expect(outcome.result, _okResult);
      expect(outcome.truncated, isTrue);
      expect(
        outcome.updatedAt.toUtc(),
        DateTime.utc(2026, 7, 17),
        reason: 'updated_at timestamp must be preserved for badge rendering',
      );
    });

    test(
      'restart_simulation_restores_as_restored_after_provider_recreate_load',
      () async {
        // Session 1: record results → persisted to the "store".
        final store = <String, List<LastOutcomeDto>>{};
        final session1 = _makeContainer(
          persistCalls: null,
          load: (boardKey) => store[boardKey] ?? const [],
        );
        session1
            .read(currentBoardKeyProvider.notifier)
            .select(BoardKey.parse('dev'));
        store['dev'] = [storedRow];

        // Session 2 (restart): the provider is created for the first time
        // while a board is already selected — the microtask initial
        // hydration path in build.
        final session2 = _makeContainer(
          load: (boardKey) => store[boardKey] ?? const [],
        );
        session2
            .read(currentBoardKeyProvider.notifier)
            .select(BoardKey.parse('dev'));
        session2.read(lastOutcomeProvider);
        await Future<void>.delayed(Duration.zero);

        expect(
          session2.read(lastOutcomeProvider)[toolId],
          isA<RestoredOutcome>(),
        );
      },
    );

    test('hydration_does_not_overwrite_session_fresh_results', () {
      final container = _makeContainer(load: (_) => [storedRow]);
      container.read(lastOutcomeProvider.notifier).record(toolId, _errorResult);

      container
          .read(lastOutcomeProvider.notifier)
          .hydrateForBoard(BoardKey.parse('dev'));

      final cached = container.read(lastOutcomeProvider)[toolId];
      expect(cached, isA<FreshOutcome>());
      expect(cached?.result, _errorResult);
    });

    test('hydration_load_failure_leaves_existing_cache_untouched', () {
      final container = _makeContainer(
        load: (_) => throw StateError('store unavailable'),
      );
      container.read(lastOutcomeProvider.notifier).record(toolId, _okResult);

      container
          .read(lastOutcomeProvider.notifier)
          .hydrateForBoard(BoardKey.parse('dev'));

      expect(container.read(lastOutcomeProvider)[toolId], isA<FreshOutcome>());
    });
  });

  group('lastOutcomeProvider clear', () {
    test('clear_removes_only_the_given_tool_cache', () {
      final other = ToolId.parse('id.uuid_v7');
      final container = _makeContainer();
      final notifier = container.read(lastOutcomeProvider.notifier)
        ..record(toolId, _okResult)
        ..record(other, _okResult);

      notifier.clear(toolId);

      final state = container.read(lastOutcomeProvider);
      expect(state.containsKey(toolId), isFalse);
      expect(state.containsKey(other), isTrue);
    });
  });
}
