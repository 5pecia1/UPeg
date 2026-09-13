import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/rust/api/board_details.dart' as frb;
import 'package:upeg/src/rust/api/board_details.dart';

typedef BoardDetailsLoader = Future<BoardDetailsDto> Function(String boardKey);
typedef BoardGuidanceSaver =
    Future<void> Function(
      String boardKey,
      String description,
      String instructions,
    );
typedef BoardConnectionLoader =
    Future<BoardConnectionPreviewDto> Function(String boardKey);

final boardDetailsLoaderProvider = Provider<BoardDetailsLoader>(
  (ref) =>
      (key) => frb.loadBoardDetails(boardKey: key),
);
final boardGuidanceSaverProvider = Provider<BoardGuidanceSaver>(
  (ref) =>
      (key, description, instructions) => frb.saveBoardGuidance(
        boardKey: key,
        description: description,
        instructions: instructions,
      ),
);
final boardConnectionLoaderProvider = Provider<BoardConnectionLoader>(
  (ref) =>
      (key) => frb.previewBoardConnection(boardKey: key),
);

final boardDetailsProvider = FutureProvider.autoDispose
    .family<BoardDetailsDto, String>((ref, key) {
      return ref.watch(boardDetailsLoaderProvider)(key);
    });
final boardConnectionProvider = FutureProvider.autoDispose
    .family<BoardConnectionPreviewDto, String>((ref, key) {
      return ref.watch(boardConnectionLoaderProvider)(key);
    });
