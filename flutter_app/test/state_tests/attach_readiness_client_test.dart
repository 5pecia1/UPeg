import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/testing.dart';
import 'package:http/http.dart' as http;

import 'package:upeg/src/features/host_attach/attach_client.dart';
import 'package:upeg/src/features/host_attach/host_attach_config.dart';
import 'package:upeg/src/identity.dart';

void main() {
  HttpAttachClient clientFor(
    Object? body, {
    int status = 200,
    void Function(http.Request request)? onRequest,
  }) => HttpAttachClient(
    config: const HostAttachConfig(baseUrl: 'http://host/', token: 'secret'),
    httpClient: MockClient((request) async {
      onRequest?.call(request);
      return http.Response(jsonEncode(body), status);
    }),
  );

  test('inspect_readiness_sends_auth_and_board_query', () async {
    http.Request? request;
    final client = clientFor(<String, Object?>{
      'status': 'ready',
      'platform': 'linux',
    }, onRequest: (value) => request = value);

    final result = await client.inspectReadiness(
      toolId: ToolId.parse('shell.rg'),
      boardKey: 'board-1',
    );

    expect(result, isA<AttachReadinessOk>());
    expect(
      request!.url.toString(),
      'http://host/v1/tools/shell.rg/readiness?board=board-1',
    );
    expect(request!.headers['authorization'], 'Bearer secret');
  });

  test('inspect_readiness_decodes_setup_and_install_commands', () async {
    final result = await clientFor(<String, Object?>{
      'status': 'missing_executable',
      'platform': 'linux',
      'command': 'rg',
      'working_directory': '/tmp',
      'executable': '/usr/bin/rg',
      'setup': <String, Object?>{
        'guide_url': 'https://example.test/install',
        'instructions': 'Install ripgrep',
        'install': <String, Object?>{
          'platform': 'linux',
          'commands': <String>['apt install ripgrep'],
        },
      },
    }).inspectReadiness(toolId: ToolId.parse('shell.rg'));

    final value = (result as AttachReadinessOk).readiness;
    expect(value.status.name, 'missingExecutable');
    expect(value.guideUrl, 'https://example.test/install');
    expect(value.installCommands, ['apt install ripgrep']);
  });

  test('inspect_readiness_maps_null_unauthorized_and_malformed', () async {
    final nullResult = await clientFor(
      null,
    ).inspectReadiness(toolId: ToolId.parse('x.y'));
    expect(nullResult, isA<AttachReadinessNotApplicable>());

    final unauthorized = await clientFor(
      <String, Object?>{},
      status: 401,
    ).inspectReadiness(toolId: ToolId.parse('x.y'));
    expect(unauthorized, isA<AttachReadinessUnauthorized>());

    final malformed = await clientFor(<String, Object?>{
      'status': 'not-a-status',
      'platform': 'linux',
    }).inspectReadiness(toolId: ToolId.parse('x.y'));
    expect(malformed, isA<AttachReadinessMalformed>());
  });

  test('inspect_readiness_rejects_malformed_nested_types', () async {
    final result = await clientFor(<String, Object?>{
      'status': 'ready',
      'platform': 'linux',
      'setup': <String, Object?>{
        'install': <String, Object?>{
          'commands': <Object?>['ok', 42],
        },
      },
    }).inspectReadiness(toolId: ToolId.parse('x.y'));
    expect(result, isA<AttachReadinessMalformed>());
  });
}
