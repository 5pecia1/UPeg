/// Shared, write-behind editing drafts, independent from executable presets.
///
/// An edit updates memory at once and is saved to durable storage by one
/// timer per Pin with a fixed [kDraftSaveWindow] deadline. The first unsaved
/// change arms the timer, later edits only replace the value (the deadline is
/// never pushed back), and expiry saves the latest value once. A pending save
/// is written early only where waiting would be wrong: before the process
/// context changes and before the app quits ([InlineDraftStore.flushAll]).
/// Deleting a Pin or Board, resetting the project view, and disposing the
/// store cancel pending saves instead, so a deleted draft never comes back.
library;

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pin_drafts.dart' as frb;
import 'package:upeg/src/state/pin_provider.dart' show PinKey;
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

typedef PinDraftLoad = List<frb.PinDraftDto> Function(BoardKey boardKey);
typedef PinDraftSave =
    void Function(PinKey pinKey, ToolId toolId, ToolArgs args);

/// Longest time an edit waits in memory before it is saved.
const Duration kDraftSaveWindow = Duration(milliseconds: 500);

/// Starts the save timer. Tests inject a recording or fake-clock factory.
typedef DraftTimerFactory = Timer Function(Duration, void Function());

final pinDraftLoadProvider = Provider<PinDraftLoad>(
  (ref) =>
      (board) => frb.loadPinDrafts(boardKey: board.value),
);

final pinDraftSaveProvider = Provider<PinDraftSave>(
  (ref) =>
      (pin, tool, args) => frb.savePinDraft(
        boardKey: pin.$1.value,
        pinId: pin.$2.value,
        toolId: tool.value,
        argsJson: args.encodeJson(),
      ),
);

class InlineDraftStore extends ChangeNotifier {
  InlineDraftStore({
    this.load,
    this.save,
    this.saveWindow = kDraftSaveWindow,
    DraftTimerFactory? timerFactory,
  }) : _timerFactory = timerFactory ?? Timer.new;

  final PinDraftLoad? load;
  final PinDraftSave? save;
  final Duration saveWindow;
  final DraftTimerFactory _timerFactory;
  final Map<PinKey, ToolArgs> _drafts = <PinKey, ToolArgs>{};
  final Map<PinKey, ToolId> _tools = <PinKey, ToolId>{};
  final Set<BoardKey> _loadedBoards = <BoardKey>{};
  final Set<PinKey> _saveFailures = <PinKey>{};
  final Set<BoardKey> _loadFailures = <BoardKey>{};
  final Map<PinKey, Timer> _pendingSaves = <PinKey, Timer>{};
  int _generation = 0;

  int get generation => _generation;

  /// Whether the last save attempt for [pinKey] failed. Saves run on the
  /// deadline, so this can trail the latest edit by up to [saveWindow].
  bool hasSaveFailure(PinKey pinKey) => _saveFailures.contains(pinKey);
  bool hasLoadFailure(PinKey pinKey) =>
      _loadFailures.contains(pinKey.$1) && !_drafts.containsKey(pinKey);

  /// Records the edit in memory now; the save follows on the Pin's deadline.
  void set(PinKey pinKey, ToolArgs args, {ToolId? toolId}) {
    _drafts[pinKey] = args;
    if (toolId != null) _tools[pinKey] = toolId;
    _schedulePersist(pinKey);
    notifyListeners();
  }

  ToolArgs? read(PinKey pinKey, {ToolId? toolId}) {
    _hydrate(pinKey.$1);
    final storedTool = _tools[pinKey];
    if (toolId != null && storedTool != null && storedTool != toolId) {
      return null;
    }
    return _drafts[pinKey];
  }

  void retry(PinKey pinKey) {
    if (_loadFailures.contains(pinKey.$1)) _hydrate(pinKey.$1);
    flush(pinKey);
  }

  /// Saves [pinKey] now, superseding its pending save.
  void flush(PinKey pinKey) {
    _cancelSave(pinKey);
    _persist(pinKey);
    notifyListeners();
  }

  /// Saves every pending draft now. Call it before the process context
  /// changes (the save namespace is resolved when the write happens) and
  /// before the app quits.
  void flushAll() {
    if (_pendingSaves.isEmpty) return;
    final pending = _pendingSaves.keys.toList(growable: false);
    _cancelAllSaves();
    pending.forEach(_persist);
    notifyListeners();
  }

  void _hydrate(BoardKey board) {
    if (_loadedBoards.contains(board)) return;
    try {
      for (final row in load?.call(board) ?? <frb.PinDraftDto>[]) {
        final pinId = PinId.tryParse(row.pinId);
        final toolId = ToolId.tryParse(row.toolId);
        final args = ToolArgs.tryDecodeObject(row.argsJson);
        if (pinId == null || toolId == null || args == null) continue;
        final pin = (board, pinId);
        if (_drafts.containsKey(pin)) continue;
        _drafts[pin] = args;
        _tools[pin] = toolId;
      }
      _loadFailures.remove(board);
      _loadedBoards.add(board);
    } on Object catch (error) {
      _loadFailures.add(board);
      debugPrint('upeg: loadPinDrafts failed: $error');
    }
  }

  void _persist(PinKey pinKey) {
    final tool = _tools[pinKey];
    final args = _drafts[pinKey];
    if (tool == null || args == null) return;
    try {
      save?.call(pinKey, tool, args);
      _saveFailures.remove(pinKey);
    } on Object catch (error) {
      _saveFailures.add(pinKey);
      debugPrint('upeg: savePinDraft failed: $error');
    }
  }

  /// Arms the Pin's save deadline unless one is already running. Without
  /// [save] or a known Tool there is nothing to write, so no timer is made.
  void _schedulePersist(PinKey pinKey) {
    if (save == null || _tools[pinKey] == null) return;
    if (_pendingSaves.containsKey(pinKey)) return;
    _pendingSaves[pinKey] = _timerFactory(saveWindow, () {
      _pendingSaves.remove(pinKey);
      _persist(pinKey);
      notifyListeners();
    });
  }

  void _cancelSave(PinKey pinKey) => _pendingSaves.remove(pinKey)?.cancel();

  void _cancelAllSaves() {
    for (final timer in _pendingSaves.values) {
      timer.cancel();
    }
    _pendingSaves.clear();
  }

  /// The backend removes durable state in the placement's delete transaction,
  /// so a pending save is cancelled rather than written after that delete.
  void clear(PinKey pinKey) {
    _cancelSave(pinKey);
    _drafts.remove(pinKey);
    _tools.remove(pinKey);
    _saveFailures.remove(pinKey);
    notifyListeners();
  }

  void clearBoard(BoardKey board) {
    _pendingSaves.keys
        .where((key) => key.$1 == board)
        .toList(growable: false)
        .forEach(_cancelSave);
    _drafts.removeWhere((key, _) => key.$1 == board);
    _tools.removeWhere((key, _) => key.$1 == board);
    _saveFailures.removeWhere((key) => key.$1 == board);
    _loadedBoards.remove(board);
    _loadFailures.remove(board);
    notifyListeners();
  }

  /// Visible board ids may be reused by another project. Durable rows stay
  /// in their own project namespace; only this view's cache is reset.
  void resetMemory() {
    _generation += 1;
    _cancelAllSaves();
    _drafts.clear();
    _tools.clear();
    _saveFailures.clear();
    _loadedBoards.clear();
    _loadFailures.clear();
    notifyListeners();
  }

  /// Pending saves are dropped, never flushed: a write after the owner is
  /// gone could resurrect a deleted draft or land in another project's
  /// namespace. The quit path flushes before the container is torn down.
  @override
  void dispose() {
    _cancelAllSaves();
    super.dispose();
  }
}

final inlineDraftProvider = Provider<InlineDraftStore>((ref) {
  final store = InlineDraftStore(
    load: ref.watch(pinDraftLoadProvider),
    save: ref.watch(pinDraftSaveProvider),
  );
  ref.onDispose(store.dispose);
  return store;
});
