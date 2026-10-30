import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/last_outcomes.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/last_outcome_provider.dart';
import 'package:upeg/src/state/inline_draft_provider.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';
import '../test_helpers/pegboard_selection_overrides.dart';

const _ok = CanonicalToolResult(ok: true, outputs: []);
const _failure = CanonicalToolResult(ok: false, outputs: []);
final _board = BoardKey.parse('dev');
final _first = (_board, PinId.parse('pin-a'));
final _second = (_board, PinId.parse('pin-b'));
final _otherBoard = BoardKey.parse('other');
final _otherPin = (_otherBoard, PinId.parse('pin-c'));
final _tool = ToolId.parse('text.pair');

LastOutcomeDto _row(
  String pinId,
  CanonicalToolResult result, {
  String? argsJson,
  int at = 7,
}) => LastOutcomeDto(
  pinId: pinId,
  toolId: _tool.value,
  argsJson: argsJson,
  result: result,
  truncated: true,
  updatedAtMs: at,
);

ProviderContainer makeContainer({
  List<LastOutcomeDto> Function(String)? load,
  void Function(String, String, String, CanonicalToolResult, String?)? persist,
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
          argsJson,
        }) => persist?.call(boardKey, pinId, toolId, result, argsJson),
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
      persist: (b, p, t, result, _) => writes.add(('$b/$p/$t', result)),
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
      persist: (b, p, t, result, _) => writes.add(('$b/$p/$t', result)),
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
      persist: (_, _, _, _, _) => throw StateError('store unavailable'),
    );
    container.read(lastOutcomeProvider.notifier).record(_first, _tool, _ok);
    expect(container.read(pinLastOutcomeProvider(_first)), isA<FreshOutcome>());
  });

  test('outcome captures the run args separately from the edited draft', () {
    String? savedArgsJson;
    final container = makeContainer(
      persist: (_, _, _, _, argsJson) => savedArgsJson = argsJson,
    );
    final drafts = InlineDraftStore(load: (_) => const []);
    addTearDown(drafts.dispose);
    final runArgs = ToolArgs.fromJsonObject(const <String, Object?>{
      'value': 'the input that ran',
    });
    final currentDraft = ToolArgs.fromJsonObject(const <String, Object?>{
      'value': 'newer unfinished edit',
    });
    drafts.set(_first, runArgs, toolId: _tool);

    container
        .read(lastOutcomeProvider.notifier)
        .record(_first, _tool, _ok, args: runArgs);
    drafts.set(_first, currentDraft, toolId: _tool);

    final fresh =
        container.read(pinLastOutcomeProvider(_first))! as FreshOutcome;
    expect(fresh.args, runArgs);
    expect(drafts.read(_first, toolId: _tool), currentDraft);
    expect(savedArgsJson, runArgs.encodeJson());
  });

  test('failed outcome persistence can be retried with its run args', () {
    var shouldFail = true;
    String? savedArgsJson;
    final runArgs = ToolArgs.fromJsonObject(const <String, Object?>{
      'input': 'unfinished numeric text: -',
    });
    final container = makeContainer(
      persist: (_, _, _, _, argsJson) {
        if (shouldFail) throw StateError('store unavailable');
        savedArgsJson = argsJson;
      },
    );
    final notifier = container.read(lastOutcomeProvider.notifier);

    expect(notifier.record(_first, _tool, _ok, args: runArgs), isFalse);
    final failed =
        container.read(pinLastOutcomeProvider(_first))! as FreshOutcome;
    expect(failed.persisted, isFalse);
    expect(failed.args, runArgs);

    shouldFail = false;
    notifier.retry(_first, _tool);

    final retried =
        container.read(pinLastOutcomeProvider(_first))! as FreshOutcome;
    expect(retried.persisted, isTrue);
    expect(retried.args, runArgs);
    expect(savedArgsJson, runArgs.encodeJson());
  });

  test('a stale earlier run cannot overwrite a newer completion', () {
    final container = makeContainer();
    final notifier = container.read(lastOutcomeProvider.notifier);
    final olderToken = notifier.beginRun(_first);
    final newerToken = notifier.beginRun(_first);

    expect(
      notifier.record(_first, _tool, _failure, runToken: newerToken),
      isTrue,
    );
    expect(notifier.record(_first, _tool, _ok, runToken: olderToken), isFalse);
    expect(container.read(pinLastOutcomeProvider(_first))!.result, _failure);
  });

  test('clearing a pin invalidates its in-flight completion', () {
    final container = makeContainer();
    final notifier = container.read(lastOutcomeProvider.notifier);
    final runToken = notifier.beginRun(_first);

    notifier.clear(_first);

    expect(notifier.record(_first, _tool, _ok, runToken: runToken), isFalse);
    expect(container.read(pinLastOutcomeProvider(_first)), isNull);
  });

  test('clearing a board invalidates its runs and preserves other boards', () {
    final container = makeContainer();
    final notifier = container.read(lastOutcomeProvider.notifier);
    final runToken = notifier.beginRun(_first);
    notifier.record(_otherPin, _tool, _failure);

    notifier.clearBoard(_board);

    expect(notifier.record(_first, _tool, _ok, runToken: runToken), isFalse);
    expect(container.read(pinLastOutcomeProvider(_first)), isNull);
    expect(container.read(pinLastOutcomeProvider(_otherPin))!.result, _failure);
  });

  test('hydrates valid board rows as restored while fresh entries win', () {
    const restoredArgsJson = '{"input":"-"}';
    final row = _row('pin-a', _ok, argsJson: restoredArgsJson);
    final container = makeContainer(load: (_) => [row]);
    container.read(lastOutcomeProvider.notifier).hydrateForBoard(_board);
    final restored =
        container.read(pinLastOutcomeProvider(_first)) as RestoredOutcome;
    expect(restored.result, _ok);
    expect(
      restored.args,
      ToolArgs.fromJsonObject(const <String, Object?>{'input': '-'}),
    );
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
