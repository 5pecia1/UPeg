import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/canonical_file_value_codec.dart';

const int _largeCanonicalPayloadBytes = 16 * 1024 * 1024;
const int _base64DecodedBytesPerQuartet = 3;
const int _base64QuartetsPerChunk = 4096;
const String _zeroByteQuartet = 'AAAA';

void main() {
  group('CanonicalFileValue codec', () {
    test('bytes file round-trips the Rust JSON shape losslessly', () {
      const json = <String, Object?>{
        'name': 'report.bin',
        'is_dir': false,
        'mime': 'application/octet-stream',
        'content': <String, Object?>{'kind': 'bytes', 'bytes': 'AH//'},
      };

      final decoded = canonicalFileValueFromJson(json);

      expect(
        decoded,
        CanonicalFileValue(
          name: 'report.bin',
          isDir: false,
          mime: 'application/octet-stream',
          content: CanonicalFileContent.bytes(
            bytes: Uint8List.fromList(const <int>[0, 127, 255]),
          ),
        ),
      );
      expect(jsonEncode(canonicalFileValueToJson(decoded!)), jsonEncode(json));
    });

    test('directory file round-trips recursive entries losslessly', () {
      const json = <String, Object?>{
        'name': 'assets',
        'is_dir': true,
        'content': <String, Object?>{
          'kind': 'directory',
          'entries': <Object?>[
            <String, Object?>{
              'name': 'logo.png',
              'is_dir': false,
              'content': <String, Object?>{'kind': 'bytes', 'bytes': 'iVA='},
            },
          ],
        },
      };

      final decoded = canonicalFileValueFromJson(json);

      expect(decoded, isNotNull);
      expect(jsonEncode(canonicalFileValueToJson(decoded!)), jsonEncode(json));
    });

    test('dynamic map from the JSON decoder restores into a typed file', () {
      final json = jsonDecode(
        '{"name":"input.txt","is_dir":false,'
        '"content":{"kind":"bytes","bytes":"QQ=="}}',
      );

      final decoded = canonicalFileValueFromJson(json);

      expect(decoded?.name, 'input.txt');
      expect(canonicalFileValueToJson(decoded!), <String, Object?>{
        'name': 'input.txt',
        'is_dir': false,
        'content': <String, Object?>{'kind': 'bytes', 'bytes': 'QQ=='},
      });
    });

    test('empty bytes round-trip as an empty base64 string', () {
      const json = <String, Object?>{
        'name': 'empty.bin',
        'is_dir': false,
        'content': <String, Object?>{'kind': 'bytes', 'bytes': ''},
      };

      final decoded = canonicalFileValueFromJson(json);

      expect((decoded!.content as CanonicalFileContent_Bytes).bytes, isEmpty);
      expect(canonicalFileValueToJson(decoded), json);
    });

    test(
      '16 MiB canonical base64 validates and decodes on native and Chrome',
      () {
        final encoded = _zeroBytesBase64(_largeCanonicalPayloadBytes);

        final decoded = canonicalFileValueFromJson(_fileJsonWithBytes(encoded));
        final bytes = (decoded!.content as CanonicalFileContent_Bytes).bytes;

        expect(bytes, hasLength(_largeCanonicalPayloadBytes));
        expect(bytes.first, 0);
        expect(bytes.last, 0);
      },
      timeout: const Timeout(Duration(minutes: 2)),
    );

    test(
      'over the 64 MiB raw cap rejects with a typed error before allocating decoded bytes',
      () {
        const oversizedRawBytes = canonicalFileMaximumRawBytes + 1;
        final encoded = _zeroBytesBase64(oversizedRawBytes);

        expect(
          () => canonicalFileValueFromJson(_fileJsonWithBytes(encoded)),
          throwsA(
            isA<CanonicalFileValueCodecException>()
                .having(
                  (error) => error.code,
                  'error code',
                  CanonicalFileValueCodecErrorCode.maximumRawBytesExceeded,
                )
                .having(
                  (error) => error.actualBytes,
                  'actual raw byte size',
                  oversizedRawBytes,
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

    test('legacy numeric-array bytes are rejected', () {
      const json = <String, Object?>{
        'name': 'legacy.bin',
        'is_dir': false,
        'content': <String, Object?>{
          'kind': 'bytes',
          'bytes': <Object?>[0, 127, 255],
        },
      };

      expect(canonicalFileValueFromJson(json), isNull);
    });

    test(
      'base64 with URL-safe characters or whitespace is rejected with a typed error',
      () {
        for (final bytes in ['-_8=', 'A Q==', 'AQ==\n']) {
          expect(
            () => canonicalFileValueFromJson(_fileJsonWithBytes(bytes)),
            throwsA(
              isA<CanonicalFileValueCodecException>().having(
                (error) => error.code,
                'error code',
                CanonicalFileValueCodecErrorCode.invalidBase64,
              ),
            ),
          );
        }
      },
    );

    test(
      'missing padding and non-canonical tail bits are rejected with a typed error',
      () {
        for (final bytes in ['AQI', 'AQ===', 'AB==', 'AAB=']) {
          expect(
            () => canonicalFileValueFromJson(_fileJsonWithBytes(bytes)),
            throwsA(
              isA<CanonicalFileValueCodecException>().having(
                (error) => error.code,
                'error code',
                CanonicalFileValueCodecErrorCode.invalidBase64,
              ),
            ),
          );
        }
      },
    );

    test('rejects when is_dir and content kind disagree', () {
      const directoryMarkedAsFile = <String, Object?>{
        'name': 'folder',
        'is_dir': false,
        'content': <String, Object?>{
          'kind': 'directory',
          'entries': <Object?>[],
        },
      };
      const bytesMarkedAsDirectory = <String, Object?>{
        'name': 'file.bin',
        'is_dir': true,
        'content': <String, Object?>{'kind': 'bytes', 'bytes': ''},
      };

      expect(canonicalFileValueFromJson(directoryMarkedAsFile), isNull);
      expect(canonicalFileValueFromJson(bytesMarkedAsDirectory), isNull);
    });

    test('rejects unknown shapes in mime and content', () {
      const invalidMime = <String, Object?>{
        'name': 'file.bin',
        'is_dir': false,
        'mime': 42,
        'content': <String, Object?>{'kind': 'bytes', 'bytes': ''},
      };
      const unexpectedContent = <String, Object?>{
        'name': 'file.bin',
        'is_dir': false,
        'content': <String, Object?>{
          'kind': 'bytes',
          'bytes': '',
          'path': '/tmp/file.bin',
        },
      };

      expect(canonicalFileValueFromJson(invalidMime), isNull);
      expect(canonicalFileValueFromJson(unexpectedContent), isNull);
    });

    test('file within maximum nesting depth round-trips losslessly', () {
      final json = _nestedFileJson(canonicalFileMaximumNestingDepth);

      final decoded = canonicalFileValueFromJson(json);

      expect(decoded, isNotNull);
      expect(jsonEncode(canonicalFileValueToJson(decoded!)), jsonEncode(json));
    });

    test(
      'dynamic map beyond maximum nesting depth is rejected without StackOverflow',
      () {
        final json = _nestedFileJson(4000);

        expect(
          () => canonicalFileValueFromJson(json),
          throwsA(
            isA<CanonicalFileValueCodecException>().having(
              (error) => error.code,
              'error code',
              CanonicalFileValueCodecErrorCode.maximumNestingDepthExceeded,
            ),
          ),
        );
      },
    );

    test(
      'typed file beyond maximum nesting depth is rejected without StackOverflow',
      () {
        final file = _nestedCanonicalFile(canonicalFileMaximumNestingDepth + 1);

        expect(
          () => canonicalFileValueToJson(file),
          throwsA(
            isA<CanonicalFileValueCodecException>().having(
              (error) => error.code,
              'error code',
              CanonicalFileValueCodecErrorCode.maximumNestingDepthExceeded,
            ),
          ),
        );
      },
    );
  });
}

Map<String, Object?> _nestedFileJson(int depth) {
  Map<String, Object?> current = <String, Object?>{
    'name': 'leaf.bin',
    'is_dir': false,
    'content': <String, Object?>{'kind': 'bytes', 'bytes': ''},
  };
  for (var currentDepth = 1; currentDepth < depth; currentDepth += 1) {
    current = <String, Object?>{
      'name': 'directory-$currentDepth',
      'is_dir': true,
      'content': <String, Object?>{
        'kind': 'directory',
        'entries': <Object?>[current],
      },
    };
  }
  return current;
}

Map<String, Object?> _fileJsonWithBytes(Object? bytes) {
  return <String, Object?>{
    'name': 'input.bin',
    'is_dir': false,
    'content': <String, Object?>{'kind': 'bytes', 'bytes': bytes},
  };
}

CanonicalFileValue _nestedCanonicalFile(int depth) {
  var current = CanonicalFileValue(
    name: 'leaf.bin',
    isDir: false,
    content: CanonicalFileContent.bytes(bytes: Uint8List(0)),
  );
  for (var currentDepth = 1; currentDepth < depth; currentDepth += 1) {
    current = CanonicalFileValue(
      name: 'directory-$currentDepth',
      isDir: true,
      content: CanonicalFileContent.directory(
        entries: <CanonicalFileValue>[current],
      ),
    );
  }
  return current;
}

String _zeroBytesBase64(int byteLength) {
  final fullQuartets = byteLength ~/ _base64DecodedBytesPerQuartet;
  final remainingBytes = byteLength % _base64DecodedBytesPerQuartet;
  final fullChunk = List<String>.filled(
    _base64QuartetsPerChunk,
    _zeroByteQuartet,
  ).join();
  final chunks = List<String>.filled(
    fullQuartets ~/ _base64QuartetsPerChunk,
    fullChunk,
  );
  final remainingQuartets = fullQuartets % _base64QuartetsPerChunk;
  return <String>[
    ...chunks,
    List<String>.filled(remainingQuartets, _zeroByteQuartet).join(),
    if (remainingBytes == 1) 'AA==' else if (remainingBytes == 2) 'AAA=',
  ].join();
}
