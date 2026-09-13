import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/canonical_file_value_codec.dart';

const int _rawChunkBytes = 1024 * 1024;
const int _aggregateChunkCount = canonicalFileMaximumRawBytes ~/ _rawChunkBytes;
const int _base64DecodedBytesPerQuartet = 3;
const String _zeroByteQuartet = 'AAAA';

void main() {
  group('CanonicalFileValue root tree budget', () {
    test(
      '각 leaf가 허용 범위여도 decode raw 합계가 64 MiB보다 한 byte 크면 거부한다',
      () {
        final regularChunk = _zeroBytesBase64(_rawChunkBytes);
        final oversizedAggregateChunk = _zeroBytesBase64(_rawChunkBytes + 1);
        final entries = <Object?>[
          for (var index = 0; index < _aggregateChunkCount - 1; index += 1)
            _jsonBytesFile('$index.bin', regularChunk),
          _jsonBytesFile('last.bin', oversizedAggregateChunk),
        ];

        expect(
          () => canonicalFileValueFromJson(_jsonDirectory(entries)),
          throwsA(
            isA<CanonicalFileValueCodecException>()
                .having(
                  (error) => error.code,
                  '오류 코드',
                  CanonicalFileValueCodecErrorCode.maximumRawBytesExceeded,
                )
                .having(
                  (error) => error.actualBytes,
                  'root tree raw byte 합계',
                  canonicalFileMaximumRawBytes + 1,
                )
                .having(
                  (error) => error.limitBytes,
                  'raw byte 제한',
                  canonicalFileMaximumRawBytes,
                ),
          ),
        );
      },
      timeout: const Timeout(Duration(minutes: 2)),
    );

    test(
      '각 leaf가 허용 범위여도 encode raw 합계가 64 MiB보다 한 byte 크면 거부한다',
      () {
        final sharedBytes = Uint8List(_rawChunkBytes + 1);
        final regularChunk = Uint8List.view(
          sharedBytes.buffer,
          sharedBytes.offsetInBytes,
          _rawChunkBytes,
        );
        final oversizedAggregateChunk = Uint8List.view(
          sharedBytes.buffer,
          sharedBytes.offsetInBytes,
          _rawChunkBytes + 1,
        );
        final entries = <CanonicalFileValue>[
          for (var index = 0; index < _aggregateChunkCount - 1; index += 1)
            _canonicalBytesFile('$index.bin', regularChunk),
          _canonicalBytesFile('last.bin', oversizedAggregateChunk),
        ];

        expect(
          () => canonicalFileValueToJson(_canonicalDirectory(entries)),
          throwsA(
            isA<CanonicalFileValueCodecException>()
                .having(
                  (error) => error.code,
                  '오류 코드',
                  CanonicalFileValueCodecErrorCode.maximumRawBytesExceeded,
                )
                .having(
                  (error) => error.actualBytes,
                  'root tree raw byte 합계',
                  canonicalFileMaximumRawBytes + 1,
                ),
          ),
        );
      },
      timeout: const Timeout(Duration(minutes: 2)),
    );

    test('decode는 root를 포함한 128개 node까지 허용하고 129번째를 거부한다', () {
      final exactEntries = <Object?>[
        for (var index = 0; index < canonicalFileMaximumNodes - 1; index += 1)
          _jsonBytesFile('$index.bin', ''),
      ];
      final oversizedEntries = <Object?>[
        ...exactEntries,
        _jsonBytesFile('overflow.bin', ''),
      ];

      expect(
        canonicalFileValueFromJson(_jsonDirectory(exactEntries)),
        isNotNull,
      );
      expect(
        () => canonicalFileValueFromJson(_jsonDirectory(oversizedEntries)),
        throwsA(
          isA<CanonicalFileValueCodecException>()
              .having(
                (error) => error.code,
                '오류 코드',
                CanonicalFileValueCodecErrorCode.maximumNodesExceeded,
              )
              .having(
                (error) => error.actualNodes,
                '실제 node 수',
                canonicalFileMaximumNodes + 1,
              )
              .having(
                (error) => error.limitNodes,
                'node 제한',
                canonicalFileMaximumNodes,
              ),
        ),
      );
    });

    test('encode는 root를 포함한 128개 node까지 허용하고 129번째를 거부한다', () {
      final exactEntries = <CanonicalFileValue>[
        for (var index = 0; index < canonicalFileMaximumNodes - 1; index += 1)
          _canonicalBytesFile('$index.bin', Uint8List(0)),
      ];
      final oversizedEntries = <CanonicalFileValue>[
        ...exactEntries,
        _canonicalBytesFile('overflow.bin', Uint8List(0)),
      ];

      expect(
        () => canonicalFileValueToJson(_canonicalDirectory(exactEntries)),
        returnsNormally,
      );
      expect(
        () => canonicalFileValueToJson(_canonicalDirectory(oversizedEntries)),
        throwsA(
          isA<CanonicalFileValueCodecException>()
              .having(
                (error) => error.code,
                '오류 코드',
                CanonicalFileValueCodecErrorCode.maximumNodesExceeded,
              )
              .having(
                (error) => error.actualNodes,
                '실제 node 수',
                canonicalFileMaximumNodes + 1,
              ),
        ),
      );
    });

    test('decode는 UTF-8 name과 MIME 합계 16 KiB까지 허용하고 한 byte 초과를 거부한다', () {
      final exactName = _utf8Text(canonicalFileMaximumMetadataBytes);
      final exact = _jsonBytesFile(exactName, '');
      final oversized = <String, Object?>{...exact, 'mime': 'a'};

      expect(canonicalFileValueFromJson(exact), isNotNull);
      expect(
        () => canonicalFileValueFromJson(oversized),
        throwsA(
          isA<CanonicalFileValueCodecException>()
              .having(
                (error) => error.code,
                '오류 코드',
                CanonicalFileValueCodecErrorCode.maximumMetadataBytesExceeded,
              )
              .having(
                (error) => error.actualBytes,
                '실제 metadata byte 수',
                canonicalFileMaximumMetadataBytes + 1,
              )
              .having(
                (error) => error.limitBytes,
                'metadata byte 제한',
                canonicalFileMaximumMetadataBytes,
              ),
        ),
      );
    });

    test('encode는 UTF-8 name과 MIME 합계 16 KiB까지 허용하고 한 byte 초과를 거부한다', () {
      final exactName = _utf8Text(canonicalFileMaximumMetadataBytes);
      final exact = _canonicalBytesFile(exactName, Uint8List(0));
      final oversized = CanonicalFileValue(
        name: exactName,
        isDir: false,
        mime: 'a',
        content: CanonicalFileContent.bytes(bytes: Uint8List(0)),
      );

      expect(() => canonicalFileValueToJson(exact), returnsNormally);
      expect(
        () => canonicalFileValueToJson(oversized),
        throwsA(
          isA<CanonicalFileValueCodecException>()
              .having(
                (error) => error.code,
                '오류 코드',
                CanonicalFileValueCodecErrorCode.maximumMetadataBytesExceeded,
              )
              .having(
                (error) => error.actualBytes,
                '실제 metadata byte 수',
                canonicalFileMaximumMetadataBytes + 1,
              ),
        ),
      );
    });
  });
}

Map<String, Object?> _jsonBytesFile(String name, String bytes) {
  return <String, Object?>{
    'name': name,
    'is_dir': false,
    'content': <String, Object?>{'kind': 'bytes', 'bytes': bytes},
  };
}

Map<String, Object?> _jsonDirectory(List<Object?> entries) {
  return <String, Object?>{
    'name': '',
    'is_dir': true,
    'content': <String, Object?>{'kind': 'directory', 'entries': entries},
  };
}

CanonicalFileValue _canonicalBytesFile(String name, Uint8List bytes) {
  return CanonicalFileValue(
    name: name,
    isDir: false,
    content: CanonicalFileContent.bytes(bytes: bytes),
  );
}

CanonicalFileValue _canonicalDirectory(List<CanonicalFileValue> entries) {
  return CanonicalFileValue(
    name: '',
    isDir: true,
    content: CanonicalFileContent.directory(entries: entries),
  );
}

String _utf8Text(int byteLength) {
  final threeByteCharacters = byteLength ~/ 3;
  final remainingAsciiBytes = byteLength - threeByteCharacters * 3;
  return '${List<String>.filled(threeByteCharacters, '가').join()}'
      '${List<String>.filled(remainingAsciiBytes, 'a').join()}';
}

String _zeroBytesBase64(int byteLength) {
  final fullQuartets = byteLength ~/ _base64DecodedBytesPerQuartet;
  final remainingBytes = byteLength % _base64DecodedBytesPerQuartet;
  return '${List<String>.filled(fullQuartets, _zeroByteQuartet).join()}'
      '${remainingBytes == 1
          ? 'AA=='
          : remainingBytes == 2
          ? 'AAA='
          : ''}';
}
