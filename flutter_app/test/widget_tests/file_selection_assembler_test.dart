library;

import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/platform/bounded_file_reader.dart';
import 'package:upeg/src/platform/file_input_resource_limits.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/widgets/expanded_modal/file_selection_assembler.dart';

const _utf8MetadataSample = '가';

String _utf8Metadata(int byteBudget) {
  final sampleBytes = utf8.encode(_utf8MetadataSample).length;
  return List<String>.filled(
    byteBudget ~/ sampleBytes,
    _utf8MetadataSample,
  ).join();
}

FileSelectionCandidate _file(String name, List<int> bytes, {String? mime}) {
  return FileSelectionCandidate.file(
    name: name,
    mime: mime,
    readBytes: (_) async => Uint8List.fromList(bytes),
  );
}

Future<FileSelectionFailure> _failure(
  FileSelectionAssembler assembler,
  List<FileSelectionCandidate> candidates,
) async {
  try {
    await assembler.assemble(candidates);
  } on FileSelectionFailure catch (failure) {
    return failure;
  }
  throw StateError('selection assembly must fail');
}

void main() {
  group('FileSelectionAssembler', () {
    test('an_empty_selection_rejects_the_whole_pick', () async {
      final assembler = FileSelectionAssembler(
        const FileSelectionPolicy(extensions: [], maxCount: 1),
      );

      final failure = await _failure(assembler, const []);

      expect(failure.code, FileSelectionErrorCode.emptySelection);
    });

    test('a_single_directory_in_the_mix_rejects_the_whole_pick', () async {
      final assembler = FileSelectionAssembler(
        const FileSelectionPolicy(extensions: [], maxCount: 3),
      );
      final candidates = [
        _file('ok.txt', [1]),
        FileSelectionCandidate.directory(name: 'folder'),
      ];

      final failure = await _failure(assembler, candidates);

      expect(failure.code, FileSelectionErrorCode.directorySelected);
    });

    test('a_single_disallowed_extension_rejects_the_whole_pick', () async {
      final assembler = FileSelectionAssembler(
        const FileSelectionPolicy(extensions: ['txt'], maxCount: 3),
      );

      final failure = await _failure(assembler, [
        _file('ok.TXT', [1]),
        _file('bad.pdf', [2]),
      ]);

      expect(failure.code, FileSelectionErrorCode.extensionNotAllowed);
      expect(failure.fileName, 'bad.pdf');
    });

    test(
      'over_the_file_count_cap_rejects_the_whole_pick_before_reading_bytes',
      () async {
        var readCount = 0;
        final assembler = FileSelectionAssembler(
          const FileSelectionPolicy(extensions: [], maxCount: 1),
        );
        final candidates = List.generate(
          2,
          (index) => FileSelectionCandidate.file(
            name: '$index.bin',
            readBytes: (_) async {
              readCount += 1;
              return Uint8List(1);
            },
          ),
        );

        final failure = await _failure(assembler, candidates);

        expect(failure.code, FileSelectionErrorCode.tooManyFiles);
        expect(readCount, isZero);
      },
    );

    test(
      'over_the_utf8_name_and_mime_metadata_cap_rejects_before_reading',
      () async {
        var readCount = 0;
        final assembler = FileSelectionAssembler(
          const FileSelectionPolicy(extensions: [], maxCount: 1),
        );
        final candidate = FileSelectionCandidate.file(
          name: _utf8Metadata(maxFileInputMetadataBytes),
          mime: 'application/octet-stream',
          readBytes: (_) async {
            readCount += 1;
            return Uint8List(1);
          },
        );

        final failure = await _failure(assembler, [candidate]);

        expect(failure.code, FileSelectionErrorCode.metadataTooLarge);
        expect(failure.maximum, maxFileInputMetadataBytes);
        expect(readCount, isZero);
      },
    );

    test('metadata_at_exactly_the_utf8_cap_is_allowed', () async {
      const mime = 'x';
      final assembler = FileSelectionAssembler(
        const FileSelectionPolicy(extensions: [], maxCount: 1),
      );
      final candidate = _file(
        _utf8Metadata(maxFileInputMetadataBytes - utf8.encode(mime).length),
        [1],
        mime: mime,
      );

      final result = await assembler.assemble([candidate]);

      expect(result.name, candidate.name);
    });

    test(
      'over_the_per_file_size_cap_rejects_the_whole_pick_with_no_partial_result',
      () async {
        final assembler = FileSelectionAssembler(
          const FileSelectionPolicy(
            extensions: [],
            maxCount: 3,
            maxFileBytes: 2,
          ),
        );

        final failure = await _failure(assembler, [
          _file('small.bin', [1]),
          _file('large.bin', [1, 2, 3]),
        ]);

        expect(failure.code, FileSelectionErrorCode.fileTooLarge);
        expect(failure.fileName, 'large.bin');
      },
    );

    test(
      'over_the_total_size_cap_rejects_the_whole_pick_with_no_partial_result',
      () async {
        final assembler = FileSelectionAssembler(
          const FileSelectionPolicy(
            extensions: [],
            maxCount: 3,
            maxTotalBytes: 3,
          ),
        );

        final failure = await _failure(assembler, [
          _file('first.bin', [1, 2]),
          _file('second.bin', [3, 4]),
        ]);

        expect(failure.code, FileSelectionErrorCode.totalTooLarge);
      },
    );

    test(
      'a_single_policy_assembles_the_picked_file_into_a_bytes_root',
      () async {
        final assembler = FileSelectionAssembler(
          const FileSelectionPolicy(extensions: ['txt'], maxCount: 1),
        );

        final result = await assembler.assemble([
          _file('note.txt', [1, 2], mime: 'text/plain'),
        ]);

        expect(result.name, 'note.txt');
        expect(result.isDir, isFalse);
        expect(result.mime, 'text/plain');
        expect(result.content, isA<CanonicalFileContent_Bytes>());
      },
    );

    test(
      'a_multi_policy_assembles_even_one_file_into_a_flat_directory_root',
      () async {
        final assembler = FileSelectionAssembler(
          const FileSelectionPolicy(extensions: [], maxCount: 3),
        );

        final result = await assembler.assemble([
          _file('only.bin', [1]),
        ]);

        expect(result.isDir, isTrue);
        final directory = result.content as CanonicalFileContent_Directory;
        expect(directory.entries.map((entry) => entry.name), ['only.bin']);
        expect(directory.entries.single.isDir, isFalse);
      },
    );

    test(
      'a_failed_file_read_becomes_a_single_failure_carrying_the_cause',
      () async {
        final assembler = FileSelectionAssembler(
          const FileSelectionPolicy(extensions: [], maxCount: 2),
        );
        final candidate = FileSelectionCandidate.file(
          name: 'broken.bin',
          readBytes: (_) => Future<Uint8List>.error(const FormatException()),
        );

        final failure = await _failure(assembler, [candidate]);

        expect(failure.code, FileSelectionErrorCode.readFailed);
        expect(failure.fileName, 'broken.bin');
      },
    );

    test(
      'each_file_reader_gets_the_smaller_of_policy_and_surface_transport_byte_budget',
      () async {
        int? receivedMaximum;
        final assembler = FileSelectionAssembler(
          const FileSelectionPolicy(extensions: [], maxCount: 1),
        );
        final candidate = FileSelectionCandidate.file(
          name: 'bounded.bin',
          readBytes: (maximumBytes) async {
            receivedMaximum = maximumBytes;
            return Uint8List(1);
          },
        );

        await assembler.assemble([candidate]);

        expect(receivedMaximum, maxFileInputTransportBytes);
      },
    );

    test(
      'after_a_file_is_read_only_the_remaining_total_byte_budget_reaches_the_next_reader',
      () async {
        final receivedMaximums = <int>[];
        final assembler = FileSelectionAssembler(
          const FileSelectionPolicy(
            extensions: [],
            maxCount: 2,
            maxFileBytes: 5,
            maxTotalBytes: 3,
          ),
        );
        final candidates = [
          FileSelectionCandidate.file(
            name: 'first.bin',
            readBytes: (maximumBytes) async {
              receivedMaximums.add(maximumBytes);
              return Uint8List.fromList([1, 2]);
            },
          ),
          FileSelectionCandidate.file(
            name: 'second.bin',
            readBytes: (maximumBytes) async {
              receivedMaximums.add(maximumBytes);
              return Uint8List.fromList([3]);
            },
          ),
        ];

        await assembler.assemble(candidates);

        expect(receivedMaximums, [3, 1]);
      },
    );

    test(
      'the_next_reader_gets_the_remaining_aggregate_budget_even_without_a_policy_total_cap',
      () async {
        int? secondMaximum;
        final assembler = FileSelectionAssembler(
          const FileSelectionPolicy(extensions: [], maxCount: 2),
        );
        final candidates = [
          _file('first.bin', [1, 2]),
          FileSelectionCandidate.file(
            name: 'second.bin',
            readBytes: (maximumBytes) async {
              secondMaximum = maximumBytes;
              throw FileReadLimitExceeded(maximumBytes);
            },
          ),
        ];

        final failure = await _failure(assembler, candidates);

        expect(secondMaximum, maxFileInputAggregateTransportBytes - 2);
        expect(failure.code, FileSelectionErrorCode.totalTooLarge);
        expect(failure.maximum, maxFileInputAggregateTransportBytes);
      },
    );

    test(
      'a_programmer_error_from_the_reader_propagates_instead_of_hiding_as_user_error',
      () async {
        final assembler = FileSelectionAssembler(
          const FileSelectionPolicy(extensions: [], maxCount: 1),
        );
        final candidate = FileSelectionCandidate.file(
          name: 'programmer-error.bin',
          readBytes: (_) async => throw StateError('programmer bug'),
        );

        await expectLater(
          assembler.assemble([candidate]),
          throwsA(isA<StateError>()),
        );
      },
    );
  });
}
