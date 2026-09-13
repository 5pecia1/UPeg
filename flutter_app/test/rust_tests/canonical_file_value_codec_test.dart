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
    test('bytes 파일은 Rust JSON shape를 손실 없이 왕복한다', () {
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

    test('directory 파일은 재귀 entries를 손실 없이 왕복한다', () {
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

    test('JSON decoder가 만든 dynamic map도 typed 파일로 복원한다', () {
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

    test('빈 bytes는 빈 base64 문자열로 왕복한다', () {
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
      '16 MiB canonical base64는 native와 Chrome에서 검증하고 decode한다',
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
      '64 MiB raw cap 초과는 decoded bytes 할당 전에 typed 오류로 거부한다',
      () {
        const oversizedRawBytes = canonicalFileMaximumRawBytes + 1;
        final encoded = _zeroBytesBase64(oversizedRawBytes);

        expect(
          () => canonicalFileValueFromJson(_fileJsonWithBytes(encoded)),
          throwsA(
            isA<CanonicalFileValueCodecException>()
                .having(
                  (error) => error.code,
                  '오류 코드',
                  CanonicalFileValueCodecErrorCode.maximumRawBytesExceeded,
                )
                .having(
                  (error) => error.actualBytes,
                  '실제 raw byte 크기',
                  oversizedRawBytes,
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

    test('legacy 숫자 배열 bytes는 거부한다', () {
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

    test('URL-safe 문자와 공백이 있는 base64는 typed 오류로 거부한다', () {
      for (final bytes in ['-_8=', 'A Q==', 'AQ==\n']) {
        expect(
          () => canonicalFileValueFromJson(_fileJsonWithBytes(bytes)),
          throwsA(
            isA<CanonicalFileValueCodecException>().having(
              (error) => error.code,
              '오류 코드',
              CanonicalFileValueCodecErrorCode.invalidBase64,
            ),
          ),
        );
      }
    });

    test('누락된 padding과 비정규 tail bit는 typed 오류로 거부한다', () {
      for (final bytes in ['AQI', 'AQ===', 'AB==', 'AAB=']) {
        expect(
          () => canonicalFileValueFromJson(_fileJsonWithBytes(bytes)),
          throwsA(
            isA<CanonicalFileValueCodecException>().having(
              (error) => error.code,
              '오류 코드',
              CanonicalFileValueCodecErrorCode.invalidBase64,
            ),
          ),
        );
      }
    });

    test('is_dir와 content kind가 일치하지 않으면 거부한다', () {
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

    test('mime과 content에 알 수 없는 shape가 있으면 거부한다', () {
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

    test('최대 nesting depth 이내 파일은 손실 없이 왕복한다', () {
      final json = _nestedFileJson(canonicalFileMaximumNestingDepth);

      final decoded = canonicalFileValueFromJson(json);

      expect(decoded, isNotNull);
      expect(jsonEncode(canonicalFileValueToJson(decoded!)), jsonEncode(json));
    });

    test('최대 nesting depth를 넘는 dynamic map은 StackOverflow 없이 거부한다', () {
      final json = _nestedFileJson(4000);

      expect(
        () => canonicalFileValueFromJson(json),
        throwsA(
          isA<CanonicalFileValueCodecException>().having(
            (error) => error.code,
            '오류 코드',
            CanonicalFileValueCodecErrorCode.maximumNestingDepthExceeded,
          ),
        ),
      );
    });

    test('최대 nesting depth를 넘는 typed 파일은 StackOverflow 없이 거부한다', () {
      final file = _nestedCanonicalFile(canonicalFileMaximumNestingDepth + 1);

      expect(
        () => canonicalFileValueToJson(file),
        throwsA(
          isA<CanonicalFileValueCodecException>().having(
            (error) => error.code,
            '오류 코드',
            CanonicalFileValueCodecErrorCode.maximumNestingDepthExceeded,
          ),
        ),
      );
    });
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
