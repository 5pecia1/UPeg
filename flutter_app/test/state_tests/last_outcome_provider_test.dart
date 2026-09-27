import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/last_outcomes.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/last_outcome_provider.dart';
import '../test_helpers/pegboard_selection_overrides.dart';

const _ok = CanonicalToolResult(ok: true, outputs: []);
const _failure = CanonicalToolResult(ok: false, outputs: []);
final _board = BoardKey.parse('dev');
final _first = (_board, PinId.parse('pin-a'));
final _second = (_board, PinId.parse('pin-b'));
final _otherBoard = BoardKey.parse('other');
final _otherPin = (_otherBoard, PinId.parse('pin-c'));
final _tool = ToolId.parse('text.pair');

LastOutcomeDto _row(String pinId, CanonicalToolResult result, {int at = 7}) =>
    LastOutcomeDto(
      pinId: pinId,
      toolId: _tool.value,
      result: result,
      truncated: true,
      updatedAtMs: at,
    );

ProviderContainer makeContainer({
  List<LastOutcomeDto> Function(String)? load,
  void Function(String, String, String, CanonicalToolResult)? persist,
}) {
  final container = ProviderContainer(
    overrides: [
      ...pegboardSelectionOverrides(),
      lastOutcomeLoadProvider.overrideWithValue(load ?? (_) => const []),
      lastOutcomePersistProvider.overrideWithValue(
        ({
          required boardKey,
          required pinId,
          required toolId,
          required result,
        }) => persist?.call(boardKey, pinId, toolId, result),
      ),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

void main() {
  test('record persists both duplicate pins and clear isolates a sibling', () {
    final writes = <(String, CanonicalToolResult)>[];
    final container = makeContainer(
      persist: (b, p, t, result) => writes.add(('$b/$p/$t', result)),
    );
    final notifier = container.read(lastOutcomeProvider.notifier)
      ..record(_first, _tool, _ok)
      ..record(_second, _tool, _failure);
    expect(container.read(pinLastOutcomeProvider(_first)), isA<FreshOutcome>());
    expect(container.read(pinLastOutcomeProvider(_second))!.result, _failure);
    notifier.clear(_first);
    expect(container.read(pinLastOutcomeProvider(_first)), isNull);
    expect(
      container.read(pinLastOutcomeProvider(_second)),
      isA<FreshOutcome>(),
    );
    expect(writes, [
      ('dev/pin-a/text.pair', _ok),
      ('dev/pin-b/text.pair', _failure),
    ]);
  });

  test('record replaces only its own pin and persists failure outcomes', () {
    final writes = <(String, CanonicalToolResult)>[];
    final container = makeContainer(
      persist: (b, p, t, result) => writes.add(('$b/$p/$t', result)),
    );
    container.read(lastOutcomeProvider.notifier)
      ..record(_first, _tool, _ok)
      ..record(_second, _tool, _ok)
      ..record(_first, _tool, _failure);

    expect(container.read(pinLastOutcomeProvider(_first))!.result, _failure);
    expect(container.read(pinLastOutcomeProvider(_second))!.result, _ok);
    expect(writes, [
      ('dev/pin-a/text.pair', _ok),
      ('dev/pin-b/text.pair', _ok),
      ('dev/pin-a/text.pair', _failure),
    ]);
  });

  test('persistence failure leaves the fresh result in memory', () {
    final container = makeContainer(
      persist: (_, _, _, _) => throw StateError('store unavailable'),
    );
    container.read(lastOutcomeProvider.notifier).record(_first, _tool, _ok);
    expect(container.read(pinLastOutcomeProvider(_first)), isA<FreshOutcome>());
  });

  test('hydrates valid board rows as restored while fresh entries win', () {
    final row = _row('pin-a', _ok);
    final container = makeContainer(load: (_) => [row]);
    container.read(lastOutcomeProvider.notifier).hydrateForBoard(_board);
    final restored =
        container.read(pinLastOutcomeProvider(_first)) as RestoredOutcome;
    expect(restored.result, _ok);
    expect(restored.truncated, isTrue);
    expect(restored.updatedAt.millisecondsSinceEpoch, 7);
    container
        .read(lastOutcomeProvider.notifier)
        .record(_first, _tool, _failure);
    container.read(lastOutcomeProvider.notifier).hydrateForBoard(_board);
    expect(container.read(pinLastOutcomeProvider(_first))!.result, _failure);
  });

  test(
    'board hydration skips malformed rows and preserves other-board state',
    () {
      final good = _row('pin-b', _ok);
      final bad = _row(' padded ', _ok);
      final container = makeContainer(
        load: (board) => switch (board) {
          'dev' => [good, bad],
          'other' => [_row('pin-c', _failure)],
          _ => const [],
        },
      );
      container.read(lastOutcomeProvider.notifier).hydrateForBoard(_board);
      expect(
        container.read(pinLastOutcomeProvider(_second)),
        isA<RestoredOutcome>(),
      );
      expect(container.read(lastOutcomeProvider).length, 1);
      container.read(lastOutcomeProvider.notifier).hydrateForBoard(_otherBoard);
      expect(
        container.read(pinLastOutcomeProvider(_second)),
        isA<RestoredOutcome>(),
      );
      expect(
        container.read(pinLastOutcomeProvider(_otherPin))!.result,
        _failure,
      );
    },
  );

  test('board switching keeps session-fresh outcomes on both boards', () {
    final container = makeContainer(
      load: (board) => board == 'dev'
          ? [_row('pin-a', _failure)]
          : [_row('pin-c', _failure)],
    );
    final notifier = container.read(lastOutcomeProvider.notifier)
      ..record(_first, _tool, _ok)
      ..record(_otherPin, _tool, _ok);
    notifier.hydrateForBoard(_board);
    notifier.hydrateForBoard(_otherBoard);
    expect(container.read(pinLastOutcomeProvider(_first))!.result, _ok);
    expect(container.read(pinLastOutcomeProvider(_otherPin))!.result, _ok);
  });

  test('an empty complete snapshot removes only that board restored rows', () {
    var rows = <LastOutcomeDto>[_row('pin-a', _ok)];
    final container = makeContainer(load: (_) => rows);
    final notifier = container.read(lastOutcomeProvider.notifier);
    notifier.hydrateForBoard(_board);
    notifier.record(_second, _tool, _failure);
    notifier.record(_otherPin, _tool, _ok);
    rows = [];
    notifier.hydrateForBoard(_board);
    expect(container.read(pinLastOutcomeProvider(_first)), isNull);
    expect(
      container.read(pinLastOutcomeProvider(_second)),
      isA<FreshOutcome>(),
    );
    expect(
      container.read(pinLastOutcomeProvider(_otherPin)),
      isA<FreshOutcome>(),
    );
  });

  test('load failure preserves cached outcomes', () {
    var fail = false;
    final container = makeContainer(
      load: (_) =>
          fail ? throw StateError('store unavailable') : [_row('pin-a', _ok)],
    );
    final notifier = container.read(lastOutcomeProvider.notifier);
    notifier.hydrateForBoard(_board);
    fail = true;
    notifier.hydrateForBoard(_board);
    expect(
      container.read(pinLastOutcomeProvider(_first)),
      isA<RestoredOutcome>(),
    );
  });

  test('selected board hydrates when the notifier is first built', () async {
    final container = makeContainer(load: (_) => [_row('pin-a', _ok)]);
    container.read(currentBoardKeyProvider.notifier).select(_board);
    container.read(lastOutcomeProvider);
    await Future<void>.delayed(Duration.zero);
    expect(
      container.read(pinLastOutcomeProvider(_first)),
      isA<RestoredOutcome>(),
    );
  });

  test('a board selection hydrates while the notifier is alive', () {
    final container = makeContainer(load: (_) => [_row('pin-a', _ok)]);
    container.read(lastOutcomeProvider);
    container.read(currentBoardKeyProvider.notifier).select(_board);
    expect(
      container.read(pinLastOutcomeProvider(_first)),
      isA<RestoredOutcome>(),
    );
  });
}
