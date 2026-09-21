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
      'rejects when decode raw total exceeds 64 MiB by one byte even if every leaf is within limits',
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
                  'error code',
                  CanonicalFileValueCodecErrorCode.maximumRawBytesExceeded,
                )
                .having(
                  (error) => error.actualBytes,
                  'root tree raw byte total',
                  canonicalFileMaximumRawBytes + 1,
                )
                .having(
                  (error) => error.limitBytes,
                  'raw byte limit',
                  canonicalFileMaximumRawBytes,
                ),
          ),
        );
      },
      timeout: const Timeout(Duration(minutes: 2)),
    );

    test(
      'rejects when encode raw total exceeds 64 MiB by one byte even if every leaf is within limits',
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
                  'error code',
                  CanonicalFileValueCodecErrorCode.maximumRawBytesExceeded,
                )
                .having(
                  (error) => error.actualBytes,
                  'root tree raw byte total',
                  canonicalFileMaximumRawBytes + 1,
                ),
          ),
        );
      },
      timeout: const Timeout(Duration(minutes: 2)),
    );

    test(
      'decode allows up to 128 nodes including root and rejects the 129th',
      () {
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
                  'error code',
                  CanonicalFileValueCodecErrorCode.maximumNodesExceeded,
                )
                .having(
                  (error) => error.actualNodes,
                  'actual node count',
                  canonicalFileMaximumNodes + 1,
                )
                .having(
                  (error) => error.limitNodes,
                  'node limit',
                  canonicalFileMaximumNodes,
                ),
          ),
        );
      },
    );

    test(
      'encode allows up to 128 nodes including root and rejects the 129th',
      () {
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
                  'error code',
                  CanonicalFileValueCodecErrorCode.maximumNodesExceeded,
                )
                .having(
                  (error) => error.actualNodes,
                  'actual node count',
                  canonicalFileMaximumNodes + 1,
                ),
          ),
        );
      },
    );

    test(
      'decode allows up to 16 KiB of UTF-8 name plus MIME and rejects one byte over',
      () {
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
                  'error code',
                  CanonicalFileValueCodecErrorCode.maximumMetadataBytesExceeded,
                )
                .having(
                  (error) => error.actualBytes,
                  'actual metadata byte count',
                  canonicalFileMaximumMetadataBytes + 1,
                )
                .having(
                  (error) => error.limitBytes,
                  'metadata byte limit',
                  canonicalFileMaximumMetadataBytes,
                ),
          ),
        );
      },
    );

    test(
      'encode allows up to 16 KiB of UTF-8 name plus MIME and rejects one byte over',
      () {
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
                  'error code',
                  CanonicalFileValueCodecErrorCode.maximumMetadataBytesExceeded,
                )
                .having(
                  (error) => error.actualBytes,
                  'actual metadata byte count',
                  canonicalFileMaximumMetadataBytes + 1,
                ),
          ),
        );
      },
    );
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
