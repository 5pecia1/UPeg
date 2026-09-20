library;

import 'package:flutter/foundation.dart';
import 'package:upeg/src/platform/bounded_file_reader.dart';
import 'package:upeg/src/platform/file_extension.dart';
import 'package:upeg/src/platform/file_input_resource_limits.dart';
import 'package:upeg/src/rust/api/tools.dart';

export 'package:upeg/src/platform/bounded_file_reader.dart'
    show FileReadLimitExceeded, maxFileInputTransportBytes;

@immutable
final class FileSelectionPolicy {
  const FileSelectionPolicy({
    required this.extensions,
    required this.maxCount,
    this.maxFileBytes,
    this.maxTotalBytes,
  });

  final List<String> extensions;
  final int maxCount;
  final int? maxFileBytes;
  final int? maxTotalBytes;
}

typedef FileBytesReader = Future<Uint8List> Function(int maximumBytes);

@immutable
sealed class FileSelectionCandidate {
  const FileSelectionCandidate();

  const factory FileSelectionCandidate.file({
    required String name,
    required FileBytesReader readBytes,
    String? mime,
  }) = FileSelectionFile;

  const factory FileSelectionCandidate.directory({required String name}) =
      FileSelectionDirectory;

  String get name;
  String? get mime;
  bool get isDirectory;
}

final class FileSelectionFile extends FileSelectionCandidate {
  const FileSelectionFile({
    required this.name,
    required this.readBytes,
    this.mime,
  });

  @override
  final String name;
  final FileBytesReader readBytes;
  @override
  final String? mime;
  @override
  bool get isDirectory => false;
}

final class FileSelectionDirectory extends FileSelectionCandidate {
  const FileSelectionDirectory({required this.name});

  @override
  final String name;
  @override
  String? get mime => null;
  @override
  bool get isDirectory => true;
}

enum FileSelectionErrorCode {
  invalidPolicy,
  emptySelection,
  directorySelected,
  extensionNotAllowed,
  tooManyFiles,
  tooManyNodes,
  metadataTooLarge,
  fileTooLarge,
  totalTooLarge,
  readFailed,
}

final class FileSelectionFailure implements Exception {
  const FileSelectionFailure(this.code, {this.fileName, this.maximum});

  final FileSelectionErrorCode code;
  final String? fileName;
  final int? maximum;

  /// Catalog key (upeg-pegboard-ui/src/i18n.rs) for the localized
  /// presentation of this failure. Pair with [messageArgs] and render
  /// via `tRead(ref, failure.messageKey, failure.messageArgs)`.
  String get messageKey => switch (code) {
    FileSelectionErrorCode.invalidPolicy => 'modal.file.error.invalid_policy',
    FileSelectionErrorCode.emptySelection => 'modal.file.error.empty_selection',
    FileSelectionErrorCode.directorySelected =>
      'modal.file.error.directory_selected',
    FileSelectionErrorCode.extensionNotAllowed =>
      'modal.file.error.extension_not_allowed',
    FileSelectionErrorCode.tooManyFiles => 'modal.file.error.too_many_files',
    FileSelectionErrorCode.tooManyNodes => 'modal.file.error.too_many_nodes',
    FileSelectionErrorCode.metadataTooLarge =>
      'modal.file.error.metadata_too_large',
    FileSelectionErrorCode.fileTooLarge => 'modal.file.error.file_too_large',
    FileSelectionErrorCode.totalTooLarge => 'modal.file.error.total_too_large',
    FileSelectionErrorCode.readFailed => 'modal.file.error.read_failed',
  };

  /// `{name}` args for [messageKey], drawn from the structured fields.
  /// `null` when the code carries no interpolations.
  Map<String, String>? get messageArgs {
    final args = <String, String>{
      'file': ?fileName,
      'max': ?maximum?.toString(),
    };
    return args.isEmpty ? null : args;
  }

  /// Locale-independent diagnostic for logs and `toString()` — user-facing
  /// copy comes from [messageKey], never from this getter.
  String get message => switch (code) {
    FileSelectionErrorCode.invalidPolicy => 'invalid file selection policy',
    FileSelectionErrorCode.emptySelection => 'no files selected',
    FileSelectionErrorCode.directorySelected => 'directory selected: $fileName',
    FileSelectionErrorCode.extensionNotAllowed =>
      'file type not allowed: $fileName',
    FileSelectionErrorCode.tooManyFiles => 'too many files (limit $maximum)',
    FileSelectionErrorCode.tooManyNodes =>
      'file structure exceeds the $maximum-node limit',
    FileSelectionErrorCode.metadataTooLarge => 'file metadata too large',
    FileSelectionErrorCode.fileTooLarge =>
      'file exceeds the size limit: $fileName',
    FileSelectionErrorCode.totalTooLarge => 'total file size exceeds the limit',
    FileSelectionErrorCode.readFailed => 'could not read file: $fileName',
  };

  @override
  String toString() => 'FileSelectionFailure(${code.name}): $message';
}

final class FileSelectionAssembler {
  const FileSelectionAssembler(this.policy);

  final FileSelectionPolicy policy;

  Future<CanonicalFileValue> assemble(
    List<FileSelectionCandidate> candidates,
  ) async {
    _validatePolicy();
    final files = _validatedFiles(candidates);

    final entries = <CanonicalFileValue>[];
    var totalBytes = 0;
    final aggregateBudget = effectiveFileInputAggregateBytes(
      policy.maxTotalBytes,
    );
    for (final candidate in files) {
      final fileBudget = _fileBudget();
      final remainingAggregateBudget = aggregateBudget - totalBytes;
      final readBudget = _minimum(fileBudget, remainingAggregateBudget);
      final Uint8List bytes;
      try {
        bytes = await candidate.readBytes(readBudget);
      } on FileReadLimitExceeded {
        if (remainingAggregateBudget < fileBudget) {
          throw FileSelectionFailure(
            FileSelectionErrorCode.totalTooLarge,
            maximum: aggregateBudget,
          );
        }
        throw FileSelectionFailure(
          FileSelectionErrorCode.fileTooLarge,
          fileName: candidate.name,
          maximum: fileBudget,
        );
      } on Exception {
        throw FileSelectionFailure(
          FileSelectionErrorCode.readFailed,
          fileName: candidate.name,
        );
      }
      if (bytes.length > readBudget) {
        if (remainingAggregateBudget < fileBudget) {
          throw FileSelectionFailure(
            FileSelectionErrorCode.totalTooLarge,
            maximum: aggregateBudget,
          );
        }
        throw FileSelectionFailure(
          FileSelectionErrorCode.fileTooLarge,
          fileName: candidate.name,
          maximum: fileBudget,
        );
      }
      totalBytes += bytes.length;
      entries.add(
        CanonicalFileValue(
          name: candidate.name,
          isDir: false,
          content: CanonicalFileContent.bytes(bytes: bytes),
          mime: candidate.mime,
        ),
      );
    }

    if (policy.maxCount == 1) return entries.single;
    return CanonicalFileValue(
      name: fileInputSelectionRootName,
      isDir: true,
      content: CanonicalFileContent.directory(
        entries: List<CanonicalFileValue>.unmodifiable(entries),
      ),
      mime: null,
    );
  }

  int _fileBudget() {
    final policyLimit = policy.maxFileBytes;
    if (policyLimit == null) return maxFileInputTransportBytes;
    return _minimum(maxFileInputTransportBytes, policyLimit);
  }

  void _validatePolicy() {
    final limits = [policy.maxFileBytes, policy.maxTotalBytes];
    if (policy.maxCount < 1 ||
        policy.maxCount > maxFileInputCount ||
        limits.whereType<int>().any((limit) => limit < 0)) {
      throw const FileSelectionFailure(FileSelectionErrorCode.invalidPolicy);
    }
  }

  List<FileSelectionFile> _validatedFiles(
    List<FileSelectionCandidate> candidates,
  ) {
    if (candidates.isEmpty) {
      throw const FileSelectionFailure(FileSelectionErrorCode.emptySelection);
    }
    if (candidates.length > policy.maxCount) {
      throw FileSelectionFailure(
        FileSelectionErrorCode.tooManyFiles,
        maximum: policy.maxCount,
      );
    }
    if (candidates.length > maxFileInputCount) {
      throw const FileSelectionFailure(
        FileSelectionErrorCode.tooManyFiles,
        maximum: maxFileInputCount,
      );
    }
    final createsDirectory = policy.maxCount > 1;
    final nodeCount = flatFileInputNodeCount(
      candidates.length,
      createsDirectory: createsDirectory,
    );
    if (nodeCount > maxFileInputNodes) {
      throw const FileSelectionFailure(
        FileSelectionErrorCode.tooManyNodes,
        maximum: maxFileInputNodes,
      );
    }
    final extensions = normalizeFileExtensions(policy.extensions);
    if (extensions.contains('')) {
      throw const FileSelectionFailure(FileSelectionErrorCode.invalidPolicy);
    }
    final files = <FileSelectionFile>[];
    for (final candidate in candidates) {
      final file = switch (candidate) {
        final FileSelectionFile file => file,
        FileSelectionDirectory() => throw FileSelectionFailure(
          FileSelectionErrorCode.directorySelected,
          fileName: candidate.name,
        ),
      };
      if (extensions.isNotEmpty &&
          !hasAllowedFileExtension(file.name, extensions)) {
        throw FileSelectionFailure(
          FileSelectionErrorCode.extensionNotAllowed,
          fileName: file.name,
        );
      }
      files.add(file);
    }
    final metadataBytes = fileInputMetadataByteCount(
      files.map((file) => (name: file.name, mime: file.mime)),
      includesSelectionRoot: createsDirectory,
    );
    if (metadataBytes > maxFileInputMetadataBytes) {
      throw const FileSelectionFailure(
        FileSelectionErrorCode.metadataTooLarge,
        maximum: maxFileInputMetadataBytes,
      );
    }
    return files;
  }
}

int _minimum(int left, int right) => left < right ? left : right;
