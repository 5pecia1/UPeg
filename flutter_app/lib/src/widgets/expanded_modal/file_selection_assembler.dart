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

  String get message => switch (code) {
    FileSelectionErrorCode.invalidPolicy => '파일 선택 정책이 올바르지 않습니다.',
    FileSelectionErrorCode.emptySelection => '선택된 파일이 없습니다.',
    FileSelectionErrorCode.directorySelected => '폴더는 선택할 수 없습니다: $fileName',
    FileSelectionErrorCode.extensionNotAllowed => '허용되지 않는 파일 형식입니다: $fileName',
    FileSelectionErrorCode.tooManyFiles => '파일은 최대 $maximum개까지 선택할 수 있습니다.',
    FileSelectionErrorCode.tooManyNodes => '파일 구조가 최대 $maximum개 노드를 초과했습니다.',
    FileSelectionErrorCode.metadataTooLarge => '파일 이름과 MIME 정보가 너무 큽니다.',
    FileSelectionErrorCode.fileTooLarge => '파일 크기 제한을 초과했습니다: $fileName',
    FileSelectionErrorCode.totalTooLarge => '전체 파일 크기 제한을 초과했습니다.',
    FileSelectionErrorCode.readFailed => '파일을 읽지 못했습니다: $fileName',
  };

  @override
  String toString() => message;
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
