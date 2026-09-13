import 'package:flutter_riverpod/misc.dart' show Override;

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/state/pegboard_selection_provider.dart';
import 'package:upeg/src/state/tag_selection.dart';

List<Override> pegboardSelectionOverrides({
  String? boardKey,
  String? tag,
  void Function(PegboardSelectionDto selection)? onSave,
  List<String> Function(BoardKey? boardKey)? tagOptions,
}) {
  final all = const TagAll().frbValue;
  return [
    pegboardSelectionLoaderProvider.overrideWithValue(
      () => PegboardSelectionDto(boardKey: boardKey, tag: tag ?? all),
    ),
    pegboardSelectionSaverProvider.overrideWithValue(onSave ?? (_) {}),
    pegboardSelectionTagOptionsLoaderProvider.overrideWithValue(
      tagOptions ?? (_) => [all],
    ),
  ];
}
