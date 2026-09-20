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
  test(
    'passes_multi_select_and_extension_filter_verbatim_to_file_picker',
    () async {
      final originalFilePicker = _registeredFilePickerOrFallback();
      addTearDown(() => fp.FilePicker.platform = originalFilePicker);
      final picker = _ResultFilePicker(
        fp.FilePickerResult([
          fp.PlatformFile(
            name: 'a.txt',
            size: 1,
            readStream: Stream.value([1]),
          ),
          fp.PlatformFile(
            name: 'b.txt',
            size: 1,
            readStream: Stream.value([2]),
          ),
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
    },
  );

  test(
    'single_select_shares_the_multi_select_implementation_with_allowMultiple_false',
    () async {
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
    },
  );

  test('returns_null_when_the_user_cancels_multi_select', () async {
    final originalFilePicker = _registeredFilePickerOrFallback();
    addTearDown(() => fp.FilePicker.platform = originalFilePicker);
    fp.FilePicker.platform = _ResultFilePicker(null);

    final picked = await const FilePickerPluginBridge().pickOpenFiles(
      allowMultiple: true,
    );

    expect(picked, isNull);
  });

  test('propagates_file_picker_errors_to_the_caller', () async {
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

  test(
    'metadata_file_count_overflow_is_rejected_before_any_stream_is_read',
    () async {
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
            'error code',
            FilePickerSelectionErrorCode.tooManyFiles,
          ),
        ),
      );
      expect(listenCount, isZero);
    },
  );

  test(
    'fixed_file_count_cap_overflow_is_rejected_before_any_stream_is_read',
    () async {
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
                'error code',
                FilePickerSelectionErrorCode.tooManyFiles,
              )
              .having(
                (failure) => failure.maximum,
                'fixed file count cap',
                maxFileInputCount,
              ),
        ),
      );
      expect(listenCount, isZero);
    },
  );

  test(
    'UTF-8_metadata_cap_overflow_is_rejected_before_any_stream_is_read',
    () async {
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
                'error code',
                FilePickerSelectionErrorCode.metadataTooLarge,
              )
              .having(
                (failure) => failure.maximum,
                'metadata byte cap',
                maxFileInputMetadataBytes,
              ),
        ),
      );
      expect(listenCount, isZero);
    },
  );

  test(
    'metadata_individual_size_overflow_is_rejected_before_the_stream_is_read',
    () async {
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
            'error code',
            FilePickerSelectionErrorCode.fileTooLarge,
          ),
        ),
      );
      expect(listenCount, isZero);
    },
  );

  test(
    'metadata_total_size_overflow_is_rejected_before_all_streams_are_read',
    () async {
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
            'error code',
            FilePickerSelectionErrorCode.totalTooLarge,
          ),
        ),
      );
      expect(listenCount, isZero);
    },
  );

  test(
    'stream_stops_reading_as_soon_as_the_cap_overflow_is_detected_even_with_small_metadata',
    () async {
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
            'error code',
            FilePickerSelectionErrorCode.fileTooLarge,
          ),
        ),
      );
      expect(yieldedChunks, 2);
    },
  );

  test(
    'a_disallowed_compound_extension_returned_by_the_picker_is_rejected_before_any_stream_is_read',
    () async {
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
                'error code',
                FilePickerSelectionErrorCode.extensionNotAllowed,
              )
              .having(
                (failure) => failure.fileName,
                'file name',
                '거부됨.tar.zip',
              ),
        ),
      );
      expect(listenCount, isZero);
    },
  );

  test('an_empty_allowed_extension_list_allows_all_file_names', () async {
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

  test(
    'a_negative_metadata_size_does_not_offset_other_file_sizes_in_the_aggregate_total',
    () async {
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
            'error code',
            FilePickerSelectionErrorCode.totalTooLarge,
          ),
        ),
      );
      expect(listenCount, isZero);
    },
  );

  test(
    'the_fixed_aggregate_cap_applies_before_reading_when_the_policy_total_limit_is_absent_or_looser',
    () async {
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
                  'error code',
                  FilePickerSelectionErrorCode.totalTooLarge,
                )
                .having(
                  (failure) => failure.maximum,
                  'aggregate maximum size',
                  maxFileInputAggregateTransportBytes,
                ),
          ),
        );
        expect(listenCount, isZero);
      }
    },
  );

  test(
    'native_byte_fallback_read_failure_is_propagated_as_a_typed_error',
    () async {
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
            'file name',
            'missing-file.bin',
          ),
        ),
      );
    },
    skip: kIsWeb,
  );

  test(
    'web_FilePath_selection_returns_null_without_evaluating_the_path_getter',
    () async {
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
    },
    skip: !kIsWeb,
  );
}
