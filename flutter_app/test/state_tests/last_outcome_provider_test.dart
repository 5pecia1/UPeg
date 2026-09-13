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
    test('record는_fresh_상태로_기록하고_write_through를_정확히_한_번_호출한다', () {
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

    test('실패_결과도_스토어에_write_through된다', () {
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

    test('선택된_보드가_없으면_캐시만_갱신하고_영속화는_생략한다', () {
      final calls = <_PersistCall>[];
      final container = _makeContainer(persistCalls: calls);

      container.read(lastOutcomeProvider.notifier).record(toolId, _okResult);

      expect(container.read(lastOutcomeProvider)[toolId], isA<FreshOutcome>());
      expect(calls, isEmpty);
    });

    test('write_through_실패는_예외없이_캐시를_유지한다', () {
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

    test('보드_전환시_저장된_결과가_restored_상태로_복원된다', () {
      final container = _makeContainer(
        load: (boardKey) => boardKey == 'dev' ? [storedRow] : const [],
      );
      // Provider가 살아 있는 상태에서 보드가 바뀌는 시나리오.
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
        reason: 'updated_at 타임스탬프가 배지 렌더용으로 보존되어야 한다',
      );
    });

    test('재시작_시뮬레이션_provider_재생성_후_로드하면_restored로_복원된다', () async {
      // 세션 1: 결과 기록 → "store"에 영속.
      final store = <String, List<LastOutcomeDto>>{};
      final session1 = _makeContainer(
        persistCalls: null,
        load: (boardKey) => store[boardKey] ?? const [],
      );
      session1
          .read(currentBoardKeyProvider.notifier)
          .select(BoardKey.parse('dev'));
      store['dev'] = [storedRow];

      // 세션 2(재시작): 보드가 이미 선택된 상태에서 provider가 처음
      // 만들어진다 — build의 microtask 초기 hydration 경로.
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
    });

    test('세션_fresh_결과는_hydration이_덮어쓰지_않는다', () {
      final container = _makeContainer(load: (_) => [storedRow]);
      container.read(lastOutcomeProvider.notifier).record(toolId, _errorResult);

      container
          .read(lastOutcomeProvider.notifier)
          .hydrateForBoard(BoardKey.parse('dev'));

      final cached = container.read(lastOutcomeProvider)[toolId];
      expect(cached, isA<FreshOutcome>());
      expect(cached?.result, _errorResult);
    });

    test('hydration_로드_실패는_기존_캐시를_그대로_둔다', () {
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
    test('clear는_해당_tool의_캐시만_제거한다', () {
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
