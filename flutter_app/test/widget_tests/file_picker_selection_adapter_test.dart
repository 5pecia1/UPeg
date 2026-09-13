import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/widgets/expanded_modal/file_picker_selection_adapter.dart';
import 'package:upeg/src/widgets/expanded_modal/file_selection_assembler.dart';

void main() {
  group('File picker selection adapter', () {
    test('picker buffer를 adapter와 assembler가 복사하지 않고 그대로 전달한다', () async {
      final originalBytes = Uint8List.fromList([1, 2, 3]);
      final candidates = fileSelectionCandidatesFromPicked([
        (
          name: 'input.bin',
          bytes: originalBytes,
          mime: 'application/octet-stream',
        ),
      ]);

      final adaptedBytes = await (candidates.single as FileSelectionFile)
          .readBytes(originalBytes.length);
      final assembled = await const FileSelectionAssembler(
        FileSelectionPolicy(extensions: [], maxCount: 1),
      ).assemble(candidates);
      final assembledBytes =
          (assembled.content as CanonicalFileContent_Bytes).bytes;

      expect(identical(adaptedBytes, originalBytes), isTrue);
      expect(identical(assembledBytes, originalBytes), isTrue);
    });

    test('adapter는 byte 정책을 재검증하지 않고 assembler가 단독으로 거부한다', () async {
      final originalBytes = Uint8List.fromList([1, 2]);
      final candidates = fileSelectionCandidatesFromPicked([
        (name: 'oversized.bin', bytes: originalBytes, mime: null),
      ]);
      final candidate = candidates.single as FileSelectionFile;

      final adaptedBytes = await candidate.readBytes(0);

      expect(identical(adaptedBytes, originalBytes), isTrue);
      await expectLater(
        const FileSelectionAssembler(
          FileSelectionPolicy(extensions: [], maxCount: 1, maxFileBytes: 1),
        ).assemble(candidates),
        throwsA(
          isA<FileSelectionFailure>().having(
            (failure) => failure.code,
            'assembler 오류 코드',
            FileSelectionErrorCode.fileTooLarge,
          ),
        ),
      );
    });
  });
}
