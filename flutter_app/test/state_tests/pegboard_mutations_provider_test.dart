import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/keyboard.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/last_outcome_provider.dart';
import 'package:upeg/src/state/layout_provider.dart';
import 'package:upeg/src/state/pegboard_mutations_provider.dart';

final _board = BoardKey.parse('dev');
final _first = (_board, PinId.parse('first-pin'));
final _second = (_board, PinId.parse('second-pin'));
final _tool = ToolId.parse('text.pair');

ProviderContainer makeContainer(List<Override> overrides) {
  final container = ProviderContainer(
    overrides: [
      lastOutcomePersistProvider.overrideWithValue(
        ({
          required boardKey,
          required pinId,
          required toolId,
          required result,
        }) {},
      ),
      lastOutcomeLoadProvider.overrideWithValue((_) => const []),
      ...overrides,
    ],
  );
  addTearDown(container.dispose);
  return container;
}

void main() {
  test(
    'move reorder and span mutations forward exact pin and refresh',
    () async {
      final calls = <String>[];
      final container = makeContainer([
        movePinMutatorProvider.overrideWithValue(
          (b, p, x, y) => calls.add('move:${p.value}/$x/$y'),
        ),
        reorderPinMutatorProvider.overrideWithValue(
          (b, p, d) => calls.add('order:${p.value}'),
        ),
        setPinSpanMutatorProvider.overrideWithValue(
          (b, p, c, r) => calls.add('span:${p.value}/$c/$r'),
        ),
        clearPinSpanMutatorProvider.overrideWithValue(
          (b, p) => calls.add('clear:${p.value}'),
        ),
      ]);
      final mutations = container.read(pegboardMutationsProvider);
      await mutations.move(_second, anchorX: 3, anchorY: 2);
      await mutations.reorder(_second, OrderDirectionDto.next);
      await mutations.setSpan(_second, cols: 3, rows: 2);
      await mutations.clearSpan(_second);
      expect(calls, [
        'move:second-pin/3/2',
        'order:second-pin',
        'span:second-pin/3/2',
        'clear:second-pin',
      ]);
      expect(container.read(layoutRevisionProvider(_board)), 4);
    },
  );

  test('failed mutations do not bump revision', () async {
    final container = makeContainer([
      movePinMutatorProvider.overrideWithValue(
        (_, _, _, _) => throw StateError('bad move'),
      ),
    ]);
    await container
        .read(pegboardMutationsProvider)
        .move(_first, anchorX: 99, anchorY: 0);
    expect(container.read(layoutRevisionProvider(_board)), 0);
  });

  test('remove clears only its successful pin outcome', () async {
    final removed = <String>[];
    final container = makeContainer([
      removePinMutatorProvider.overrideWithValue(
        (b, p) => removed.add(p.value),
      ),
    ]);
    const result = CanonicalToolResult(ok: true, outputs: []);
    container.read(lastOutcomeProvider.notifier)
      ..record(_first, _tool, result)
      ..record(_second, _tool, result);
    await container.read(pegboardMutationsProvider).remove(_first);
    expect(removed, ['first-pin']);
    expect(container.read(pinLastOutcomeProvider(_first)), isNull);
    expect(
      container.read(pinLastOutcomeProvider(_second)),
      isA<FreshOutcome>(),
    );
  });

  test('failed remove retains both outcomes and revision', () async {
    final container = makeContainer([
      removePinMutatorProvider.overrideWithValue(
        (_, _) => throw StateError('bad remove'),
      ),
    ]);
    const result = CanonicalToolResult(ok: true, outputs: []);
    container.read(lastOutcomeProvider.notifier)
      ..record(_first, _tool, result)
      ..record(_second, _tool, result);
    await container.read(pegboardMutationsProvider).remove(_first);
    expect(container.read(pinLastOutcomeProvider(_first)), isA<FreshOutcome>());
    expect(
      container.read(pinLastOutcomeProvider(_second)),
      isA<FreshOutcome>(),
    );
    expect(container.read(layoutRevisionProvider(_board)), 0);
  });
}
