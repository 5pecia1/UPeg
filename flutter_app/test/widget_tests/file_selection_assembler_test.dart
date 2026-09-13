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
  throw StateError('선택 조립이 실패해야 합니다.');
}

void main() {
  group('FileSelectionAssembler', () {
    test('선택한 파일이 없으면 전체 선택을 거부한다', () async {
      final assembler = FileSelectionAssembler(
        const FileSelectionPolicy(extensions: [], maxCount: 1),
      );

      final failure = await _failure(assembler, const []);

      expect(failure.code, FileSelectionErrorCode.emptySelection);
    });

    test('폴더가 하나라도 섞이면 전체 선택을 거부한다', () async {
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

    test('허용하지 않은 확장자가 하나라도 있으면 전체 선택을 거부한다', () async {
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

    test('파일 개수 제한을 넘으면 바이트를 읽기 전에 전체 선택을 거부한다', () async {
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
    });

    test('UTF-8 name과 MIME metadata 상한을 넘으면 읽기 전에 거부한다', () async {
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
    });

    test('정확한 UTF-8 metadata 상한은 허용한다', () async {
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

    test('파일별 크기 제한을 넘으면 부분 결과 없이 전체 선택을 거부한다', () async {
      final assembler = FileSelectionAssembler(
        const FileSelectionPolicy(extensions: [], maxCount: 3, maxFileBytes: 2),
      );

      final failure = await _failure(assembler, [
        _file('small.bin', [1]),
        _file('large.bin', [1, 2, 3]),
      ]);

      expect(failure.code, FileSelectionErrorCode.fileTooLarge);
      expect(failure.fileName, 'large.bin');
    });

    test('전체 크기 제한을 넘으면 부분 결과 없이 전체 선택을 거부한다', () async {
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
    });

    test('단일 정책은 선택한 파일을 Bytes 루트로 조립한다', () async {
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
    });

    test('복수 정책은 파일이 하나여도 평평한 Directory 루트로 조립한다', () async {
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
    });

    test('파일 읽기가 실패하면 원인을 가진 단일 실패로 변환한다', () async {
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
    });

    test(
      '파일 reader에는 정책과 surface transport 중 더 작은 byte budget을 전달한다',
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

    test('앞선 파일을 읽은 뒤에는 남은 전체 byte budget만 다음 reader에 전달한다', () async {
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
    });

    test('정책 전체 제한이 없어도 다음 reader에는 남은 aggregate budget을 전달한다', () async {
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
    });

    test('파일 읽기의 programmer Error는 사용자 오류로 숨기지 않고 전파한다', () async {
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
    });
  });
}
