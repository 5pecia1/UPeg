library;

import 'dart:convert';

import 'package:file_picker/file_picker.dart' as fp;
import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/platform/bounded_file_reader.dart';
import 'package:upeg/src/platform/file_picker_bridge.dart';
import 'package:upeg/src/platform/file_input_resource_limits.dart';

const _missingNativeFilePath =
    '/__upeg_file_picker_bridge_test__/missing-file.bin';
const _defaultCompressionQuality = 30;
const _utf8MetadataSample = '가';

String _utf8Metadata(int byteBudget) {
  final sampleBytes = utf8.encode(_utf8MetadataSample).length;
  return List<String>.filled(
    byteBudget ~/ sampleBytes,
    _utf8MetadataSample,
  ).join();
}

final class _UnexpectedPickFilesInvocation implements Exception {
  const _UnexpectedPickFilesInvocation();
}

final class _ResultFilePicker extends fp.FilePicker {
  _ResultFilePicker(this.result, {this.errorToThrow});

  final fp.FilePickerResult? result;
  final Object? errorToThrow;
  int pickFilesCallCount = 0;
  bool? lastAllowMultiple;
  List<String>? lastAllowedExtensions;
  bool? lastWithData;
  bool? lastWithReadStream;

  @override
  Future<fp.FilePickerResult?> pickFiles({
    String? dialogTitle,
    String? initialDirectory,
    fp.FileType type = fp.FileType.any,
    List<String>? allowedExtensions,
    dynamic Function(fp.FilePickerStatus)? onFileLoading,
    bool allowCompression = true,
    int compressionQuality = _defaultCompressionQuality,
    bool allowMultiple = false,
    bool withData = false,
    bool withReadStream = false,
    bool lockParentWindow = false,
    bool readSequential = false,
  }) async {
    pickFilesCallCount += 1;
    lastAllowMultiple = allowMultiple;
    lastAllowedExtensions = allowedExtensions;
    lastWithData = withData;
    lastWithReadStream = withReadStream;
    final error = errorToThrow;
    if (error != null) {
      throw error;
    }
    return result;
  }
}

fp.FilePicker _registeredFilePickerOrFallback() {
  try {
    return fp.FilePicker.platform;
  } catch (_) {
    final fallback = _ResultFilePicker(null);
    fp.FilePicker.platform = fallback;
    return fallback;
  }
}

void main() {
  test('복수 선택과 확장자 필터를 file_picker에 그대로 전달한다', () async {
    final originalFilePicker = _registeredFilePickerOrFallback();
    addTearDown(() => fp.FilePicker.platform = originalFilePicker);
    final picker = _ResultFilePicker(
      fp.FilePickerResult([
        fp.PlatformFile(name: 'a.txt', size: 1, readStream: Stream.value([1])),
        fp.PlatformFile(name: 'b.txt', size: 1, readStream: Stream.value([2])),
      ]),
    );
    fp.FilePicker.platform = picker;

    final picked = await const FilePickerPluginBridge().pickOpenFiles(
      allowMultiple: true,
      allowedExtensions: const ['txt'],
    );

    expect(picked?.map((file) => file.name), ['a.txt', 'b.txt']);
    expect(picker.lastAllowMultiple, isTrue);
    expect(picker.lastAllowedExtensions, ['txt']);
    expect(picker.lastWithData, isFalse);
    expect(picker.lastWithReadStream, isTrue);
    expect(picked?.map((file) => file.bytes.single), [1, 2]);
  });

  test('단일 선택은 복수 선택 구현을 allowMultiple false로 공유한다', () async {
    final originalFilePicker = _registeredFilePickerOrFallback();
    addTearDown(() => fp.FilePicker.platform = originalFilePicker);
    final picker = _ResultFilePicker(
      fp.FilePickerResult([
        fp.PlatformFile(
          name: 'one.bin',
          size: 1,
          readStream: Stream.value([1]),
        ),
      ]),
    );
    fp.FilePicker.platform = picker;

    final picked = await const FilePickerPluginBridge().pickOpenFile();

    expect(picked?.name, 'one.bin');
    expect(picker.lastAllowMultiple, isFalse);
  });

  test('사용자가 복수 선택을 취소하면 null을 반환한다', () async {
    final originalFilePicker = _registeredFilePickerOrFallback();
    addTearDown(() => fp.FilePicker.platform = originalFilePicker);
    fp.FilePicker.platform = _ResultFilePicker(null);

    final picked = await const FilePickerPluginBridge().pickOpenFiles(
      allowMultiple: true,
    );

    expect(picked, isNull);
  });

  test('file_picker 오류는 호출자에게 전달한다', () async {
    final originalFilePicker = _registeredFilePickerOrFallback();
    addTearDown(() => fp.FilePicker.platform = originalFilePicker);
    fp.FilePicker.platform = _ResultFilePicker(
      null,
      errorToThrow: const _UnexpectedPickFilesInvocation(),
    );

    await expectLater(
      const FilePickerPluginBridge().pickOpenFiles(),
      throwsA(isA<_UnexpectedPickFilesInvocation>()),
    );
  });

  test('metadata 파일 개수 초과는 어떤 stream도 읽기 전에 거부한다', () async {
    final originalFilePicker = _registeredFilePickerOrFallback();
    addTearDown(() => fp.FilePicker.platform = originalFilePicker);
    var listenCount = 0;
    Stream<List<int>> trackedStream() async* {
      listenCount += 1;
      yield [1];
    }

    fp.FilePicker.platform = _ResultFilePicker(
      fp.FilePickerResult([
        fp.PlatformFile(name: 'a.bin', size: 1, readStream: trackedStream()),
        fp.PlatformFile(name: 'b.bin', size: 1, readStream: trackedStream()),
      ]),
    );

    await expectLater(
      const FilePickerPluginBridge().pickOpenFiles(maxCount: 1),
      throwsA(
        isA<FilePickerSelectionFailure>().having(
          (failure) => failure.code,
          '오류 코드',
          FilePickerSelectionErrorCode.tooManyFiles,
        ),
      ),
    );
    expect(listenCount, isZero);
  });

  test('고정 파일 개수 상한 초과는 어떤 stream도 읽기 전에 거부한다', () async {
    final originalFilePicker = _registeredFilePickerOrFallback();
    addTearDown(() => fp.FilePicker.platform = originalFilePicker);
    var listenCount = 0;
    Stream<List<int>> trackedStream() async* {
      listenCount += 1;
      yield [1];
    }

    fp.FilePicker.platform = _ResultFilePicker(
      fp.FilePickerResult(
        List.generate(
          maxFileInputCount + 1,
          (index) => fp.PlatformFile(
            name: '$index.bin',
            size: 1,
            readStream: trackedStream(),
          ),
        ),
      ),
    );

    await expectLater(
      const FilePickerPluginBridge().pickOpenFiles(
        allowMultiple: true,
        maxCount: maxFileInputCount + 1,
      ),
      throwsA(
        isA<FilePickerSelectionFailure>()
            .having(
              (failure) => failure.code,
              '오류 코드',
              FilePickerSelectionErrorCode.tooManyFiles,
            )
            .having(
              (failure) => failure.maximum,
              '고정 파일 개수 상한',
              maxFileInputCount,
            ),
      ),
    );
    expect(listenCount, isZero);
  });

  test('UTF-8 metadata 상한 초과는 어떤 stream도 읽기 전에 거부한다', () async {
    final originalFilePicker = _registeredFilePickerOrFallback();
    addTearDown(() => fp.FilePicker.platform = originalFilePicker);
    var listenCount = 0;
    Stream<List<int>> trackedStream() async* {
      listenCount += 1;
      yield [1];
    }

    final oversizedName = _utf8Metadata(maxFileInputMetadataBytes);

    fp.FilePicker.platform = _ResultFilePicker(
      fp.FilePickerResult([
        fp.PlatformFile(
          name: '$oversizedName.bin',
          size: 1,
          readStream: trackedStream(),
        ),
      ]),
    );

    await expectLater(
      const FilePickerPluginBridge().pickOpenFiles(maxCount: 1),
      throwsA(
        isA<FilePickerSelectionFailure>()
            .having(
              (failure) => failure.code,
              '오류 코드',
              FilePickerSelectionErrorCode.metadataTooLarge,
            )
            .having(
              (failure) => failure.maximum,
              'metadata byte 상한',
              maxFileInputMetadataBytes,
            ),
      ),
    );
    expect(listenCount, isZero);
  });

  test('metadata 개별 크기 초과는 stream을 읽기 전에 거부한다', () async {
    final originalFilePicker = _registeredFilePickerOrFallback();
    addTearDown(() => fp.FilePicker.platform = originalFilePicker);
    var listenCount = 0;
    Stream<List<int>> trackedStream() async* {
      listenCount += 1;
      yield [1];
    }

    fp.FilePicker.platform = _ResultFilePicker(
      fp.FilePickerResult([
        fp.PlatformFile(
          name: 'large.bin',
          size: 3,
          readStream: trackedStream(),
        ),
      ]),
    );

    await expectLater(
      const FilePickerPluginBridge().pickOpenFiles(maxFileBytes: 2),
      throwsA(
        isA<FilePickerSelectionFailure>().having(
          (failure) => failure.code,
          '오류 코드',
          FilePickerSelectionErrorCode.fileTooLarge,
        ),
      ),
    );
    expect(listenCount, isZero);
  });

  test('metadata 전체 크기 초과는 모든 stream을 읽기 전에 거부한다', () async {
    final originalFilePicker = _registeredFilePickerOrFallback();
    addTearDown(() => fp.FilePicker.platform = originalFilePicker);
    var listenCount = 0;
    Stream<List<int>> trackedStream() async* {
      listenCount += 1;
      yield [1, 2];
    }

    fp.FilePicker.platform = _ResultFilePicker(
      fp.FilePickerResult([
        fp.PlatformFile(name: 'a.bin', size: 2, readStream: trackedStream()),
        fp.PlatformFile(name: 'b.bin', size: 2, readStream: trackedStream()),
      ]),
    );

    await expectLater(
      const FilePickerPluginBridge().pickOpenFiles(
        allowMultiple: true,
        maxCount: 2,
        maxTotalBytes: 3,
      ),
      throwsA(
        isA<FilePickerSelectionFailure>().having(
          (failure) => failure.code,
          '오류 코드',
          FilePickerSelectionErrorCode.totalTooLarge,
        ),
      ),
    );
    expect(listenCount, isZero);
  });

  test('metadata가 작아도 stream은 cap 초과를 확인한 즉시 읽기를 중단한다', () async {
    final originalFilePicker = _registeredFilePickerOrFallback();
    addTearDown(() => fp.FilePicker.platform = originalFilePicker);
    var yieldedChunks = 0;
    Stream<List<int>> growingStream() async* {
      yieldedChunks += 1;
      yield [1, 2];
      yieldedChunks += 1;
      yield [3];
      yieldedChunks += 1;
      yield [4];
    }

    fp.FilePicker.platform = _ResultFilePicker(
      fp.FilePickerResult([
        fp.PlatformFile(
          name: 'growing.bin',
          size: 1,
          readStream: growingStream(),
        ),
      ]),
    );

    await expectLater(
      const FilePickerPluginBridge().pickOpenFiles(maxFileBytes: 2),
      throwsA(
        isA<FilePickerSelectionFailure>().having(
          (failure) => failure.code,
          '오류 코드',
          FilePickerSelectionErrorCode.fileTooLarge,
        ),
      ),
    );
    expect(yieldedChunks, 2);
  });

  test('picker가 반환한 허용되지 않은 복합 확장자는 어떤 stream도 읽기 전에 거부한다', () async {
    final originalFilePicker = _registeredFilePickerOrFallback();
    addTearDown(() => fp.FilePicker.platform = originalFilePicker);
    var listenCount = 0;
    Stream<List<int>> trackedStream() async* {
      listenCount += 1;
      yield [1];
    }

    fp.FilePicker.platform = _ResultFilePicker(
      fp.FilePickerResult([
        fp.PlatformFile(
          name: '허용됨.TAR.GZ',
          size: 1,
          readStream: trackedStream(),
        ),
        fp.PlatformFile(
          name: '거부됨.tar.zip',
          size: 1,
          readStream: trackedStream(),
        ),
      ]),
    );

    await expectLater(
      const FilePickerPluginBridge().pickOpenFiles(
        allowMultiple: true,
        allowedExtensions: ['tar.gz'],
        maxCount: 2,
      ),
      throwsA(
        isA<FilePickerSelectionFailure>()
            .having(
              (failure) => failure.code,
              '오류 코드',
              FilePickerSelectionErrorCode.extensionNotAllowed,
            )
            .having((failure) => failure.fileName, '파일 이름', '거부됨.tar.zip'),
      ),
    );
    expect(listenCount, isZero);
  });

  test('빈 허용 확장자 목록은 모든 파일 이름을 허용한다', () async {
    final originalFilePicker = _registeredFilePickerOrFallback();
    addTearDown(() => fp.FilePicker.platform = originalFilePicker);
    final picker = _ResultFilePicker(
      fp.FilePickerResult([
        fp.PlatformFile(
          name: '자유로운.형식',
          size: 1,
          readStream: Stream.value([1]),
        ),
      ]),
    );
    fp.FilePicker.platform = picker;

    final picked = await const FilePickerPluginBridge().pickOpenFiles(
      allowedExtensions: [],
    );

    expect(picked?.single.name, '자유로운.형식');
    expect(picker.lastAllowedExtensions, isNull);
  });

  test('음수 metadata 크기는 aggregate 합계에서 다른 파일 크기를 상쇄하지 않는다', () async {
    final originalFilePicker = _registeredFilePickerOrFallback();
    addTearDown(() => fp.FilePicker.platform = originalFilePicker);
    var listenCount = 0;
    Stream<List<int>> trackedStream() async* {
      listenCount += 1;
      yield [1];
    }

    fp.FilePicker.platform = _ResultFilePicker(
      fp.FilePickerResult([
        fp.PlatformFile(
          name: 'unknown.bin',
          size: -(maxFileInputAggregateTransportBytes ~/ 5),
          readStream: trackedStream(),
        ),
        fp.PlatformFile(
          name: 'first.bin',
          size: maxFileInputAggregateTransportBytes * 3 ~/ 5,
          readStream: trackedStream(),
        ),
        fp.PlatformFile(
          name: 'second.bin',
          size: maxFileInputAggregateTransportBytes * 3 ~/ 5,
          readStream: trackedStream(),
        ),
      ]),
    );

    await expectLater(
      const FilePickerPluginBridge().pickOpenFiles(
        allowMultiple: true,
        maxCount: 3,
      ),
      throwsA(
        isA<FilePickerSelectionFailure>().having(
          (failure) => failure.code,
          '오류 코드',
          FilePickerSelectionErrorCode.totalTooLarge,
        ),
      ),
    );
    expect(listenCount, isZero);
  });

  test('정책 전체 제한이 없거나 더 느슨해도 고정 aggregate cap은 읽기 전에 적용한다', () async {
    final originalFilePicker = _registeredFilePickerOrFallback();
    addTearDown(() => fp.FilePicker.platform = originalFilePicker);

    for (final policyMaximum in <int?>[
      null,
      maxFileInputAggregateTransportBytes + 1,
    ]) {
      var listenCount = 0;
      Stream<List<int>> trackedStream() async* {
        listenCount += 1;
        yield [1];
      }

      fp.FilePicker.platform = _ResultFilePicker(
        fp.FilePickerResult([
          fp.PlatformFile(
            name: 'first.bin',
            size: maxFileInputAggregateTransportBytes,
            readStream: trackedStream(),
          ),
          fp.PlatformFile(
            name: 'second.bin',
            size: 1,
            readStream: trackedStream(),
          ),
        ]),
      );

      await expectLater(
        const FilePickerPluginBridge().pickOpenFiles(
          allowMultiple: true,
          maxCount: 2,
          maxTotalBytes: policyMaximum,
        ),
        throwsA(
          isA<FilePickerSelectionFailure>()
              .having(
                (failure) => failure.code,
                '오류 코드',
                FilePickerSelectionErrorCode.totalTooLarge,
              )
              .having(
                (failure) => failure.maximum,
                'aggregate 최대 크기',
                maxFileInputAggregateTransportBytes,
              ),
        ),
      );
      expect(listenCount, isZero);
    }
  });

  test('native byte fallback 읽기 실패는 typed 오류로 전달한다', () async {
    final originalFilePicker = _registeredFilePickerOrFallback();
    addTearDown(() => fp.FilePicker.platform = originalFilePicker);
    fp.FilePicker.platform = _ResultFilePicker(
      fp.FilePickerResult([
        fp.PlatformFile(
          name: 'missing-file.bin',
          size: 0,
          path: _missingNativeFilePath,
        ),
      ]),
    );

    await expectLater(
      const FilePickerPluginBridge().pickOpenFile(),
      throwsA(
        isA<FilePickerReadFailure>().having(
          (failure) => failure.fileName,
          '파일 이름',
          'missing-file.bin',
        ),
      ),
    );
  }, skip: kIsWeb);

  test('Web FilePath 선택은 path getter를 평가하지 않고 null을 반환한다', () async {
    final originalFilePicker = _registeredFilePickerOrFallback();
    addTearDown(() => fp.FilePicker.platform = originalFilePicker);
    final picker = _ResultFilePicker(
      null,
      errorToThrow: const _UnexpectedPickFilesInvocation(),
    );
    fp.FilePicker.platform = picker;

    final picked = await const FilePickerPluginBridge().pickOpenPath();

    expect(picked, isNull);
    expect(picker.pickFilesCallCount, isZero);
  }, skip: !kIsWeb);
}
