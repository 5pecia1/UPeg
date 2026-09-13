library;

import 'package:upeg/src/platform/file_picker_bridge.dart';
import 'package:upeg/src/widgets/expanded_modal/file_selection_assembler.dart';

/// Transfers each picked byte buffer into the assembly pipeline.
///
/// Callers must not mutate or reuse a [PickedFileData] buffer after adapting it.
List<FileSelectionCandidate> fileSelectionCandidatesFromPicked(
  List<PickedFileData> files,
) {
  return files
      .map(
        (file) => FileSelectionCandidate.file(
          name: file.name,
          mime: file.mime,
          readBytes: (_) async => file.bytes,
        ),
      )
      .toList(growable: false);
}

FileSelectionFailure fileSelectionFailureFromPicker(
  FilePickerSelectionFailure failure,
) {
  return switch (failure.code) {
    FilePickerSelectionErrorCode.extensionNotAllowed => FileSelectionFailure(
      FileSelectionErrorCode.extensionNotAllowed,
      fileName: failure.fileName,
    ),
    FilePickerSelectionErrorCode.tooManyFiles => FileSelectionFailure(
      FileSelectionErrorCode.tooManyFiles,
      maximum: failure.maximum,
    ),
    FilePickerSelectionErrorCode.tooManyNodes => FileSelectionFailure(
      FileSelectionErrorCode.tooManyNodes,
      maximum: failure.maximum,
    ),
    FilePickerSelectionErrorCode.metadataTooLarge => FileSelectionFailure(
      FileSelectionErrorCode.metadataTooLarge,
      maximum: failure.maximum,
    ),
    FilePickerSelectionErrorCode.fileTooLarge => FileSelectionFailure(
      FileSelectionErrorCode.fileTooLarge,
      fileName: failure.fileName,
      maximum: failure.maximum,
    ),
    FilePickerSelectionErrorCode.totalTooLarge => FileSelectionFailure(
      FileSelectionErrorCode.totalTooLarge,
      maximum: failure.maximum,
    ),
  };
}
