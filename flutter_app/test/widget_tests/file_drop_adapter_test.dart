library;

import 'dart:convert';
import 'dart:typed_data';

import 'package:desktop_drop/desktop_drop.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/platform/bounded_file_reader.dart';
import 'package:upeg/src/platform/file_input_resource_limits.dart';
import 'package:upeg/src/widgets/expanded_modal/file_drop_adapter.dart';
import 'package:upeg/src/widgets/expanded_modal/file_selection_assembler.dart';

const _utf8MetadataSample = '가';

String _utf8Metadata(int byteBudget) {
  final sampleBytes = utf8.encode(_utf8MetadataSample).length;
  return List<String>.filled(
    byteBudget ~/ sampleBytes,
    _utf8MetadataSample,
  ).join();
}

final class _RecordingDropItem extends DropItemFile {
  _RecordingDropItem({
    required this.fileName,
    required this.metadataLength,
    required this.streamFactory,
  }) : super('/unused/$fileName');

  final String fileName;
  final int metadataLength;
  final Stream<Uint8List> Function(int? start, int? end) streamFactory;
  int lengthCalls = 0;
  int openReadCalls = 0;
  int? lastReadStart;
  int? lastReadEnd;

  @override
  String get name => fileName;

  @override
  Future<int> length() async {
    lengthCalls += 1;
    return metadataLength;
  }

  @override
  Stream<Uint8List> openRead([int? start, int? end]) {
    openReadCalls += 1;
    lastReadStart = start;
    lastReadEnd = end;
    return streamFactory(start, end);
  }
}

Future<FileSelectionFailure> _failure(
  FileSelectionAssembler assembler,
  FileSelectionCandidate candidate,
) async {
  try {
    await assembler.assemble([candidate]);
  } on FileSelectionFailure catch (failure) {
    return failure;
  }
  throw StateError('the drop selection must fail');
}

void main() {
  group('DesktopFileDropAdapter', () {
    test(
      'drop_utf8_metadata_over_the_cap_rejects_before_length_or_stream_are_read',
      () async {
        final item = _RecordingDropItem(
          fileName: '${_utf8Metadata(maxFileInputMetadataBytes)}xx',
          metadataLength: 1,
          streamFactory: (_, _) => Stream.value(Uint8List.fromList([1])),
        );
        final candidate = const DesktopFileDropAdapter().candidateFromItem(
          item,
        );
        final assembler = FileSelectionAssembler(
          const FileSelectionPolicy(extensions: [], maxCount: 1),
        );

        final failure = await _failure(assembler, candidate);

        expect(failure.code, FileSelectionErrorCode.metadataTooLarge);
        expect(item.lengthCalls, isZero);
        expect(item.openReadCalls, isZero);
      },
    );

    test(
      'drop_metadata_over_the_cap_rejects_before_any_stream_opens',
      () async {
        final item = _RecordingDropItem(
          fileName: 'large.bin',
          metadataLength: 3,
          streamFactory: (_, _) => Stream.value(Uint8List.fromList([1])),
        );
        final candidate = const DesktopFileDropAdapter().candidateFromItem(
          item,
        );
        final assembler = FileSelectionAssembler(
          const FileSelectionPolicy(
            extensions: [],
            maxCount: 1,
            maxFileBytes: 2,
          ),
        );

        final failure = await _failure(assembler, candidate);

        expect(failure.code, FileSelectionErrorCode.fileTooLarge);
        expect(item.lengthCalls, 1);
        expect(item.openReadCalls, isZero);
      },
    );

    test(
      'drop_cancels_the_stream_subscription_the_moment_the_cap_is_crossed',
      () async {
        var yieldedChunks = 0;
        Stream<Uint8List> growingStream(int? start, int? end) async* {
          yieldedChunks += 1;
          yield Uint8List.fromList([1, 2]);
          yieldedChunks += 1;
          yield Uint8List.fromList([3]);
          yieldedChunks += 1;
          yield Uint8List.fromList([4]);
        }

        final item = _RecordingDropItem(
          fileName: 'growing.bin',
          metadataLength: 1,
          streamFactory: growingStream,
        );
        final candidate = const DesktopFileDropAdapter().candidateFromItem(
          item,
        );
        final assembler = FileSelectionAssembler(
          const FileSelectionPolicy(
            extensions: [],
            maxCount: 1,
            maxFileBytes: 2,
          ),
        );

        final failure = await _failure(assembler, candidate);

        expect(failure.code, FileSelectionErrorCode.fileTooLarge);
        expect(item.lengthCalls, 1);
        expect(item.openReadCalls, 1);
        expect(item.lastReadStart, 0);
        expect(item.lastReadEnd, 3);
        expect(yieldedChunks, 2);
      },
    );

    test(
      'a_second_drop_metadata_over_the_remaining_aggregate_budget_opens_no_stream',
      () async {
        final firstItem = _RecordingDropItem(
          fileName: 'first.bin',
          metadataLength: 2,
          streamFactory: (_, _) => Stream.value(Uint8List.fromList([1, 2])),
        );
        final secondItem = _RecordingDropItem(
          fileName: 'second.bin',
          metadataLength: maxFileInputAggregateTransportBytes - 1,
          streamFactory: (_, _) => Stream<Uint8List>.error(
            const FileReadLimitExceeded(
              maxFileInputAggregateTransportBytes - 2,
            ),
          ),
        );
        final assembler = FileSelectionAssembler(
          const FileSelectionPolicy(extensions: [], maxCount: 2),
        );
        const adapter = DesktopFileDropAdapter();
        final candidates = [
          adapter.candidateFromItem(firstItem),
          adapter.candidateFromItem(secondItem),
        ];

        FileSelectionFailure? failure;
        try {
          await assembler.assemble(candidates);
        } on FileSelectionFailure catch (caught) {
          failure = caught;
        }

        expect(failure?.code, FileSelectionErrorCode.totalTooLarge);
        expect(failure?.maximum, maxFileInputAggregateTransportBytes);
        expect(firstItem.openReadCalls, 1);
        expect(secondItem.lengthCalls, 1);
        expect(secondItem.openReadCalls, isZero);
      },
    );
  });
}
