import 'dart:async';
import 'dart:convert';
import 'dart:math' as math;
import 'dart:typed_data';

import 'package:fake_async/fake_async.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:upeg/src/features/host_attach/attach_client.dart';
import 'package:upeg/src/features/host_attach/host_attach_config.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/canonical_file_value_codec.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

const int _dispatchWireLimitBytes = 96 * 1024 * 1024;
const int _streamChunkBytes = 1024 * 1024;
const String _responseTooLargeCode = 'response_too_large';
const String _invalidFileOutputCode = 'invalid_file_output';
const Duration _hangingCancellationTestDeadline = Duration(milliseconds: 250);

final class _StreamingClient extends http.BaseClient {
  _StreamingClient(this._handler);

  final Future<http.StreamedResponse> Function(http.BaseRequest request)
  _handler;

  @override
  Future<http.StreamedResponse> send(http.BaseRequest request) =>
      _handler(request);
}

Stream<List<int>> _paddedJsonStream({
  required String json,
  required int totalBytes,
  void Function()? onCancel,
}) {
  late StreamController<List<int>> controller;
  controller = StreamController<List<int>>(
    sync: true,
    onListen: () {
      final prefix = utf8.encode(json);
      controller.add(prefix);
      var remaining = totalBytes - prefix.length;
      while (remaining > 0) {
        final length = math.min(remaining, _streamChunkBytes);
        controller.add(Uint8List(length)..fillRange(0, length, 0x20));
        remaining -= length;
      }
      controller.close();
    },
    onCancel: onCancel,
  );
  return controller.stream;
}

HttpAttachClient _dispatchClient(http.Client httpClient) => HttpAttachClient(
  config: const HostAttachConfig(baseUrl: 'http://host', token: 'token'),
  httpClient: httpClient,
);

Future<AttachDispatchResult> _dispatch(HttpAttachClient client) =>
    client.dispatch(toolId: ToolId.parse('fixture.file'), args: ToolArgs.empty);

String _successBodyWithValue(Object? value) => jsonEncode(<String, Object?>{
  'ok': true,
  'primary_output_id': 'file',
  'outputs': <Object?>[
    <String, Object?>{
      'id': 'file',
      'label': 'File',
      'kind': 'file',
      'value': value,
    },
  ],
});

Map<String, Object?> _fileJson({
  String name = 'report.txt',
  String bytes = 'AQID',
}) => <String, Object?>{
  'name': name,
  'is_dir': false,
  'mime': 'text/plain',
  'content': <String, Object?>{'kind': 'bytes', 'bytes': bytes},
};

void main() {
  test(
    'an_empty_host_address_is_unreachable_without_issuing_a_request',
    () async {
      var called = false;
      final client = HttpAttachClient(
        config: HostAttachConfig.empty,
        httpClient: MockClient((request) async {
          called = true;
          return http.Response('{"name":"upeg","restApi":true}', 200);
        }),
      );

      final result = await client.checkHealth();

      expect(result, isA<HealthzUnreachable>());
      expect(called, isFalse);
    },
  );

  group('dispatch response stream boundaries', () {
    test('a_response_of_exactly_96mib_reads_to_the_end_and_succeeds', () async {
      const body = '{"ok":true,"outputs":[]}';
      final client = _dispatchClient(
        _StreamingClient(
          (_) async => http.StreamedResponse(
            _paddedJsonStream(json: body, totalBytes: _dispatchWireLimitBytes),
            200,
            contentLength: _dispatchWireLimitBytes,
          ),
        ),
      );

      final result = await _dispatch(client);

      expect(result, isA<AttachDispatchOk>());
    });

    test(
      'a_content_length_over_96mib_becomes_a_controlled_error_before_the_body_is_read',
      () async {
        var emitted = false;
        var cancelled = false;
        late StreamController<List<int>> controller;
        controller = StreamController<List<int>>(
          onListen: () {
            scheduleMicrotask(() {
              if (controller.isClosed || cancelled) return;
              emitted = true;
              controller.add(utf8.encode('{"ok":true,"outputs":[]}'));
              controller.close();
            });
          },
          onCancel: () => cancelled = true,
        );
        addTearDown(() async {
          if (!controller.isClosed) await controller.close();
        });
        final client = _dispatchClient(
          _StreamingClient(
            (_) async => http.StreamedResponse(
              controller.stream,
              200,
              contentLength: _dispatchWireLimitBytes + 1,
            ),
          ),
        );

        final result = await _dispatch(client);

        expect(
          result,
          isA<AttachDispatchToolError>().having(
            (value) => value.error.code,
            'error code',
            _responseTooLargeCode,
          ),
        );
        expect(emitted, isFalse);
        expect(cancelled, isTrue);
      },
    );

    test(
      'one_byte_over_96mib_without_a_length_header_cancels_collection_as_a_controlled_error',
      () async {
        var cancelled = false;
        const body = '{"ok":true,"outputs":[]}';
        final client = _dispatchClient(
          _StreamingClient(
            (_) async => http.StreamedResponse(
              _paddedJsonStream(
                json: body,
                totalBytes: _dispatchWireLimitBytes + 1,
                onCancel: () => cancelled = true,
              ),
              200,
            ),
          ),
        );

        final result = await _dispatch(client);

        expect(
          result,
          isA<AttachDispatchToolError>().having(
            (value) => value.error.code,
            'error code',
            _responseTooLargeCode,
          ),
        );
        expect(cancelled, isTrue);
      },
    );

    test(
      'an_over_length_result_returns_as_a_controlled_error_even_when_cancellation_hangs',
      () async {
        final releaseCancellation = Completer<void>();
        late StreamController<List<int>> controller;
        controller = StreamController<List<int>>(
          onCancel: () => releaseCancellation.future,
        );
        addTearDown(() async {
          if (!releaseCancellation.isCompleted) {
            releaseCancellation.complete();
          }
          if (!controller.isClosed) await controller.close();
        });
        final client = _dispatchClient(
          _StreamingClient(
            (_) async => http.StreamedResponse(
              controller.stream,
              200,
              contentLength: _dispatchWireLimitBytes + 1,
            ),
          ),
        );
        final deadline = Completer<AttachDispatchResult?>();
        final timer = Timer(
          _hangingCancellationTestDeadline,
          () => deadline.complete(null),
        );
        addTearDown(timer.cancel);

        final result = await Future.any<AttachDispatchResult?>([
          _dispatch(client),
          deadline.future,
        ]);
        timer.cancel();

        expect(
          result,
          isA<AttachDispatchToolError>().having(
            (value) => value.error.code,
            'error code',
            _responseTooLargeCode,
          ),
          reason:
              'waiting_forever_on_cancellation_would_never_return_a_dispatch_result',
        );
      },
    );

    test(
      'the_response_header_timeout_completes_the_real_abortable_request_trigger',
      () {
        fakeAsync((async) {
          http.BaseRequest? capturedRequest;
          var abortObserved = false;
          final pendingSend = Completer<http.StreamedResponse>();
          final client = _dispatchClient(
            _StreamingClient((request) {
              capturedRequest = request;
              if (request is! http.AbortableRequest) {
                return pendingSend.future;
              }
              return request.abortTrigger!.then<http.StreamedResponse>((_) {
                abortObserved = true;
                throw http.RequestAbortedException(request.url);
              });
            }),
          );
          AttachDispatchResult? result;
          _dispatch(client).then((value) => result = value);
          async.flushMicrotasks();

          async.elapse(kAttachRequestTimeout);
          async.flushMicrotasks();

          expect(capturedRequest, isA<http.AbortableRequest>());
          expect(abortObserved, isTrue);
          expect(result, isA<AttachDispatchUnreachable>());
        });
      },
    );
  });

  group('dispatch File output', () {
    test('a_valid_canonical_file_becomes_typed_output_not_a_json_string', () {
      final result = decodeDispatchBody(_successBodyWithValue(_fileJson()));

      expect(result, isA<AttachDispatchOk>());
      final output = (result as AttachDispatchOk).result.outputs.single.value;
      expect(
        output,
        isA<CanonicalOutputValue_File>().having(
          (value) => value.value.content,
          'decoded content',
          CanonicalFileContent.bytes(bytes: Uint8List.fromList([1, 2, 3])),
        ),
      );
    });

    test(
      'a_malformed_canonical_file_becomes_a_controlled_error_without_throwing',
      () {
        final result = decodeDispatchBody(
          _successBodyWithValue(<String, Object?>{
            'name': 'broken.bin',
            'is_dir': false,
            'content': <String, Object?>{'kind': 'bytes'},
          }),
        );

        expect(
          result,
          isA<AttachDispatchToolError>().having(
            (value) => value.error.code,
            'error code',
            _invalidFileOutputCode,
          ),
        );
      },
    );

    test(
      'a_canonical_file_over_the_node_budget_becomes_a_controlled_error_without_throwing',
      () {
        final entries = List<Object?>.generate(
          canonicalFileMaximumNodes,
          (index) => _fileJson(name: 'entry-$index.bin', bytes: ''),
          growable: false,
        );
        final result = decodeDispatchBody(
          _successBodyWithValue(<String, Object?>{
            'name': 'bundle',
            'is_dir': true,
            'content': <String, Object?>{
              'kind': 'directory',
              'entries': entries,
            },
          }),
        );

        expect(
          result,
          isA<AttachDispatchToolError>().having(
            (value) => value.error.code,
            'error code',
            _invalidFileOutputCode,
          ),
        );
      },
    );
  });
}
