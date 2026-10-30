import 'dart:async';

import 'package:fake_async/fake_async.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pin_drafts.dart' as frb;
import 'package:upeg/src/state/inline_draft_provider.dart';
import 'package:upeg/src/state/pin_provider.dart' show PinKey;
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

final _board = BoardKey.parse('dev');
final _first = (_board, PinId.parse('pin-a'));
final _second = (_board, PinId.parse('pin-b'));
final _otherBoard = (BoardKey.parse('other'), PinId.parse('pin-a'));
final _tool = ToolId.parse('text.pair');

frb.PinDraftDto _row(PinKey pin, ToolId tool, ToolArgs args) => frb.PinDraftDto(
  pinId: pin.$2.value,
  toolId: tool.value,
  argsJson: args.encodeJson(),
);

ToolArgs _text(String left) =>
    ToolArgs.fromJsonObject(<String, Object?>{'left': left});

/// Remembers every save timer a store starts so tests can see which ones are
/// still armed. Used inside `fakeAsync`, where the timers run on the fake clock.
final class _TimerLog {
  final timers = <Timer>[];

  int get active => timers.where((timer) => timer.isActive).length;

  Timer call(Duration duration, void Function() callback) {
    final timer = Timer(duration, callback);
    timers.add(timer);
    return timer;
  }
}

void main() {
  group('InlineDraftStore', () {
    test('restores saved raw drafts across store instances per pin', () {
      final persisted = <PinKey, frb.PinDraftDto>{};
      void save(PinKey pin, ToolId tool, ToolArgs args) {
        persisted[pin] = _row(pin, tool, args);
      }

      final malformedDraft = ToolArgs.fromJsonObject(const <String, Object?>{
        'integer': '-',
        'date': '2026-02-31T12:00:00Z',
      });
      final secondDraft = ToolArgs.fromJsonObject(const <String, Object?>{
        'left': 'separate pin',
      });
      final firstSession = InlineDraftStore(
        load: (board) => persisted.entries
            .where((entry) => entry.key.$1 == board)
            .map((entry) => entry.value)
            .toList(),
        save: save,
      );
      addTearDown(firstSession.dispose);
      firstSession
        ..set(_first, malformedDraft, toolId: _tool)
        ..set(_second, secondDraft, toolId: _tool)
        // Saves wait for a short deadline; the quit path flushes them before
        // the process ends, so a restart sees the same drafts.
        ..flushAll();

      final restarted = InlineDraftStore(
        load: (board) => persisted.entries
            .where((entry) => entry.key.$1 == board)
            .map((entry) => entry.value)
            .toList(),
        save: save,
      );
      addTearDown(restarted.dispose);

      expect(restarted.read(_first, toolId: _tool), malformedDraft);
      expect(restarted.read(_second, toolId: _tool), secondDraft);
      expect(restarted.read(_otherBoard, toolId: _tool), isNull);
    });

    test(
      'load failure can be retried without losing the failure signal early',
      () {
        var shouldFail = true;
        var loads = 0;
        final draft = ToolArgs.fromJsonObject(const <String, Object?>{
          'left': 'loaded after retry',
        });
        final store = InlineDraftStore(
          load: (_) {
            loads += 1;
            if (shouldFail) throw StateError('store unavailable');
            return [_row(_first, _tool, draft)];
          },
        );
        addTearDown(store.dispose);

        expect(store.read(_first, toolId: _tool), isNull);
        expect(store.hasLoadFailure(_first), isTrue);
        expect(loads, 1);

        shouldFail = false;
        store.retry(_first);

        expect(store.hasLoadFailure(_first), isFalse);
        expect(store.read(_first, toolId: _tool), draft);
        expect(loads, 2);
      },
    );

    test('save failure clears after retry succeeds', () {
      fakeAsync((async) {
        var shouldFail = true;
        var saves = 0;
        ToolArgs? persisted;
        final draft = _text('retry me');
        final store = InlineDraftStore(
          load: (_) => const [],
          save: (_, _, args) {
            saves += 1;
            if (shouldFail) throw StateError('store unavailable');
            persisted = args;
          },
        );

        store.set(_first, draft, toolId: _tool);
        async.elapse(kDraftSaveWindow);
        expect(store.hasSaveFailure(_first), isTrue);
        expect(persisted, isNull);

        shouldFail = false;
        store.retry(_first);

        expect(store.hasSaveFailure(_first), isFalse);
        expect(persisted, draft);
        expect(saves, 2);
        store.dispose();
      });
    });

    test('clear and clearBoard remove only their in-memory drafts', () {
      final store = InlineDraftStore(load: (_) => const []);
      addTearDown(store.dispose);
      final firstDraft = ToolArgs.fromJsonObject(const <String, Object?>{
        'left': 'first',
      });
      final secondDraft = ToolArgs.fromJsonObject(const <String, Object?>{
        'left': 'second',
      });
      final otherBoardDraft = ToolArgs.fromJsonObject(const <String, Object?>{
        'left': 'other board',
      });
      store
        ..set(_first, firstDraft, toolId: _tool)
        ..set(_second, secondDraft, toolId: _tool)
        ..set(_otherBoard, otherBoardDraft, toolId: _tool);

      store.clear(_first);
      expect(store.read(_first, toolId: _tool), isNull);
      expect(store.read(_second, toolId: _tool), secondDraft);

      store.clearBoard(_board);
      expect(store.read(_first, toolId: _tool), isNull);
      expect(store.read(_second, toolId: _tool), isNull);
      expect(store.read(_otherBoard, toolId: _tool), otherBoardDraft);
    });

    test('isolates drafts for the same tool across different boards', () {
      final store = InlineDraftStore();
      addTearDown(store.dispose);
      final firstKey = (BoardKey.parse('first'), PinId.parse(_tool.value));
      final secondKey = (BoardKey.parse('second'), PinId.parse(_tool.value));
      final draft = ToolArgs.fromJsonObject(const <String, Object?>{
        'left': 'first board',
      });

      store.set(firstKey, draft, toolId: _tool);

      expect(store.read(firstKey, toolId: _tool), draft);
      expect(store.read(secondKey, toolId: _tool), isNull);
    });
  });

  group('InlineDraftStore write-behind saves', () {
    const quickEdit = Duration(milliseconds: 40);
    late List<(PinKey, ToolArgs)> saved;
    late _TimerLog timerLog;

    setUp(() {
      saved = [];
      timerLog = _TimerLog();
    });

    InlineDraftStore newStore({PinDraftSave? save}) => InlineDraftStore(
      save: save ?? (pin, _, args) => saved.add((pin, args)),
      timerFactory: timerLog.call,
    );

    test('saves at the first deadline while edits keep arriving', () {
      fakeAsync((async) {
        final store = newStore();

        store.set(_first, _text('a'), toolId: _tool);
        async.elapse(const Duration(milliseconds: 250));
        store.set(_first, _text('b'), toolId: _tool);
        async.elapse(const Duration(milliseconds: 249));
        expect(saved, isEmpty);

        async.elapse(const Duration(milliseconds: 1));
        expect(saved, [(_first, _text('b'))]);

        async.elapse(kDraftSaveWindow * 2);
        expect(saved, hasLength(1));
        store.dispose();
      });
    });

    test('merges thirty edits over 1.2 seconds into at most three saves', () {
      fakeAsync((async) {
        final store = newStore();

        for (var i = 0; i < 30; i++) {
          store.set(_first, _text('v$i'), toolId: _tool);
          async.elapse(quickEdit);
        }
        expect(saved, isNotEmpty);

        async.elapse(kDraftSaveWindow);
        expect(saved.length, lessThanOrEqualTo(3));
        expect(saved.last, (_first, _text('v29')));
        store.dispose();
      });
    });

    test('keeps separate deadlines for separate pins', () {
      fakeAsync((async) {
        final store = newStore();

        store.set(_first, _text('first'), toolId: _tool);
        async.elapse(const Duration(milliseconds: 300));
        store.set(_second, _text('second'), toolId: _tool);
        async.elapse(const Duration(milliseconds: 200));
        expect(saved, [(_first, _text('first'))]);

        async.elapse(const Duration(milliseconds: 300));
        expect(saved, [(_first, _text('first')), (_second, _text('second'))]);
        store.dispose();
      });
    });

    test('clear before the deadline cancels the pending save', () {
      fakeAsync((async) {
        final store = newStore();

        store.set(_first, _text('gone'), toolId: _tool);
        async.elapse(const Duration(milliseconds: 499));
        store.clear(_first);
        expect(timerLog.active, 0);
        async.elapse(kDraftSaveWindow * 2);

        expect(saved, isEmpty);
        store.dispose();
      });
    });

    test('clearBoard cancels pending saves of that board only', () {
      fakeAsync((async) {
        final store = newStore();

        store
          ..set(_first, _text('a'), toolId: _tool)
          ..set(_second, _text('b'), toolId: _tool)
          ..set(_otherBoard, _text('other'), toolId: _tool)
          ..clearBoard(_board);
        expect(timerLog.active, 1);
        async.elapse(kDraftSaveWindow);

        expect(saved, [(_otherBoard, _text('other'))]);
        store.dispose();
      });
    });

    test('resetMemory cancels pending saves', () {
      fakeAsync((async) {
        final store = newStore();

        store
          ..set(_first, _text('a'), toolId: _tool)
          ..set(_otherBoard, _text('other'), toolId: _tool)
          ..resetMemory();
        expect(timerLog.active, 0);
        async.elapse(kDraftSaveWindow * 2);

        expect(saved, isEmpty);
        store.dispose();
      });
    });

    test('flushAll saves at once and the deadline does not save again', () {
      fakeAsync((async) {
        final store = newStore();
        var notifications = 0;
        store.addListener(() => notifications += 1);

        store
          ..set(_first, _text('a'), toolId: _tool)
          ..set(_second, _text('b'), toolId: _tool);
        expect(saved, isEmpty);
        expect(notifications, 2);

        store.flushAll();
        expect(saved, [(_first, _text('a')), (_second, _text('b'))]);
        expect(notifications, 3);

        async.elapse(kDraftSaveWindow * 2);
        expect(saved, hasLength(2));
        store.dispose();
      });
    });

    test('flushAll with nothing pending neither saves nor notifies', () {
      fakeAsync((async) {
        final store = newStore();
        store.set(_first, _text('a'), toolId: _tool);
        async.elapse(kDraftSaveWindow);
        var notifications = 0;
        store.addListener(() => notifications += 1);

        store.flushAll();

        expect(saved, hasLength(1));
        expect(notifications, 0);
        store.dispose();
      });
    });

    test('an edit after a flush starts a new deadline', () {
      fakeAsync((async) {
        final store = newStore();

        store.set(_first, _text('before'), toolId: _tool);
        store.flushAll();
        store.set(_first, _text('after'), toolId: _tool);
        async.elapse(kDraftSaveWindow);

        expect(saved, [(_first, _text('before')), (_first, _text('after'))]);
        store.dispose();
      });
    });

    test('flush saves one pin at once and leaves the others pending', () {
      fakeAsync((async) {
        final store = newStore();
        store
          ..set(_first, _text('a'), toolId: _tool)
          ..set(_second, _text('b'), toolId: _tool);

        store.flush(_first);
        expect(saved, [(_first, _text('a'))]);

        async.elapse(kDraftSaveWindow);
        expect(saved, [(_first, _text('a')), (_second, _text('b'))]);
        store.dispose();
      });
    });

    test('retry saves a pending draft at once and cancels its deadline', () {
      fakeAsync((async) {
        final store = newStore();
        store.set(_first, _text('a'), toolId: _tool);

        store.retry(_first);
        expect(saved, [(_first, _text('a'))]);

        async.elapse(kDraftSaveWindow * 2);
        expect(saved, hasLength(1));
        store.dispose();
      });
    });

    test('a failed deadline save shows the failure only when it runs', () {
      fakeAsync((async) {
        final store = newStore(save: (_, _, _) => throw StateError('disk'));
        var notifications = 0;
        store.addListener(() => notifications += 1);

        store.set(_first, _text('a'), toolId: _tool);
        expect(store.hasSaveFailure(_first), isFalse);
        expect(notifications, 1);

        async.elapse(kDraftSaveWindow);
        expect(store.hasSaveFailure(_first), isTrue);
        expect(notifications, 2);
        store.dispose();
      });
    });

    test('dispose cancels pending saves without writing them', () {
      fakeAsync((async) {
        final store = newStore();
        store.set(_first, _text('a'), toolId: _tool);

        store.dispose();
        expect(timerLog.active, 0);
        async.elapse(kDraftSaveWindow * 2);

        expect(saved, isEmpty);
      });
    });

    test('starts no timer without a save callback or a known tool', () {
      fakeAsync((async) {
        final withoutSave = InlineDraftStore(timerFactory: timerLog.call);
        withoutSave.set(_first, _text('a'), toolId: _tool);
        final withoutTool = newStore();
        withoutTool.set(_first, _text('a'));
        expect(timerLog.timers, isEmpty);

        withoutTool.set(_first, _text('b'), toolId: _tool);
        withoutTool.set(_first, _text('c'), toolId: _tool);
        expect(timerLog.timers, hasLength(1));

        withoutSave.dispose();
        withoutTool.dispose();
      });
    });
  });
}
