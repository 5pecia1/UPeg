import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/widgets/expanded_modal/file_picker_selection_adapter.dart';
import 'package:upeg/src/widgets/expanded_modal/file_selection_assembler.dart';

void main() {
  group('File picker selection adapter', () {
    test(
      'the_adapter_and_assembler_pass_picker_buffers_through_without_copying',
      () async {
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
      },
    );

    test(
      'the_adapter_does_not_revalidate_byte_policy_the_assembler_rejects_alone',
      () async {
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
              'assembler error code',
              FileSelectionErrorCode.fileTooLarge,
            ),
          ),
        );
      },
    );
  });
}
