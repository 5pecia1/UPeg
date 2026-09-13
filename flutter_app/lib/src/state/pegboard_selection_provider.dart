library;

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/state/tag_selection.dart';

@immutable
class PegboardSelectionState {
  const PegboardSelectionState({required this.boardKey, required this.tag});

  final BoardKey? boardKey;
  final TagSelection tag;

  @override
  bool operator ==(Object other) =>
      other is PegboardSelectionState &&
      other.boardKey == boardKey &&
      other.tag == tag;

  @override
  int get hashCode => Object.hash(boardKey, tag);
}

typedef PegboardSelectionLoader = PegboardSelectionDto Function();
typedef PegboardSelectionSaver = void Function(PegboardSelectionDto selection);
typedef PegboardSelectionTagOptionsLoader =
    List<String> Function(BoardKey? boardKey);

final pegboardSelectionLoaderProvider = Provider<PegboardSelectionLoader>(
  (ref) => loadPegboardSelection,
);

final pegboardSelectionSaverProvider = Provider<PegboardSelectionSaver>(
  (ref) =>
      (selection) => savePegboardSelection(selection: selection),
);

final pegboardSelectionTagOptionsLoaderProvider =
    Provider<PegboardSelectionTagOptionsLoader>(
      (ref) =>
          (boardKey) => tagOptionsForBoard(boardKey: boardKey?.value),
    );

class PegboardSelectionNotifier extends Notifier<PegboardSelectionState> {
  @override
  PegboardSelectionState build() {
    return const PegboardSelectionState(boardKey: null, tag: TagAll());
  }

  Future<void> restore(List<BoardDto> boards) async {
    final dto = ref.read(pegboardSelectionLoaderProvider)();
    final boardKey = _resolveBoardKey(dto.boardKey, boards);
    final tag = _normalizeTagForBoard(_tagFromDto(dto), boardKey);
    state = PegboardSelectionState(boardKey: boardKey, tag: tag);
  }

  void selectBoard(BoardKey boardKey) {
    final tag = _normalizeTagForBoard(state.tag, boardKey);
    _setAndSave(PegboardSelectionState(boardKey: boardKey, tag: tag));
  }

  void clearBoard() {
    final tag = _normalizeTagForBoard(state.tag, null);
    _setAndSave(PegboardSelectionState(boardKey: null, tag: tag));
  }

  void setTag(TagSelection tag) {
    _setAndSave(PegboardSelectionState(boardKey: state.boardKey, tag: tag));
  }

  BoardKey? _resolveBoardKey(String? raw, List<BoardDto> boards) {
    final keys = boards.map((board) => board.key).toSet();
    if (raw != null && keys.contains(raw)) {
      return BoardKey.parse(raw);
    }
    if (boards.isEmpty) {
      return null;
    }
    return BoardKey.parse(boards.first.key);
  }

  TagSelection _tagFromDto(PegboardSelectionDto dto) {
    final all = const TagAll().frbValue;
    return TagSelection.fromPersisted(dto.tag == all ? null : dto.tag);
  }

  TagSelection _normalizeTagForBoard(TagSelection tag, BoardKey? boardKey) {
    if (tag is TagAll) {
      return tag;
    }
    final options = ref.read(pegboardSelectionTagOptionsLoaderProvider)(
      boardKey,
    );
    if (options.any((option) => option == tag.frbValue)) {
      return tag;
    }
    return const TagAll();
  }

  void _setAndSave(PegboardSelectionState next) {
    state = next;
    final saver = ref.read(pegboardSelectionSaverProvider);
    try {
      saver(
        PegboardSelectionDto(
          boardKey: next.boardKey?.value,
          tag: next.tag.frbValue,
        ),
      );
    } on Object catch (err) {
      debugPrint('upeg: savePegboardSelection failed: $err');
    }
  }
}

final pegboardSelectionProvider =
    NotifierProvider<PegboardSelectionNotifier, PegboardSelectionState>(
      PegboardSelectionNotifier.new,
    );
