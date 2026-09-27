'use strict';

// Host wire (chrome-ext/host_api.js) — deep-link construction, the
// dispatch classification popup.js and background.js share, and the
// configurable loopback endpoint both of them read.

const assert = require('node:assert/strict');
const test = require('node:test');

const {
  CONTENT_SCRIPT_BOARD,
  DEFAULT_ENDPOINT,
  DEFAULT_HTTP_ADDRESS,
  DEFAULT_HTTP_BASE_URL,
  DISPATCH_RESULT_KIND,
  ENDPOINT_ERROR,
  ENDPOINT_STORAGE_KEY,
  HTTP_STATUS,
  PAIRING_COMMAND,
  RUNTIME_MESSAGE,
  RUN_HINT_COMMAND,
  authHeaders,
  buildDeepLink,
  boardShowPath,
  callTool,
  classifyDispatchResponse,
  endpointForStoredValue,
  isAuthStatus,
  outputText,
  parseLoopbackEndpoint,
  primaryOutput,
  readHostEndpoint,
  toolCallPath,
} = require('../host_api.js');

test('extension_board_paths_are_separate_from_the_legacy_http_routes', () => {
  assert.equal(boardShowPath('dev'), '/v1/ext/boards/dev');
  assert.equal(toolCallPath('dev', 'num.hex_to_decimal'), '/v1/ext/boards/dev/tools/num.hex_to_decimal');
});

test('deep_link_uses_the_same_order_and_encoding_as_the_rust_desktop_deep_link', () => {
  // `upeg_pegboard_ui::deep_link::desktop_deep_link` emits surface, board,
  // tool, input in this order with the RFC 3986 unreserved set kept.
  assert.equal(
    buildDeepLink({ board: CONTENT_SCRIPT_BOARD, toolId: 'num.hex_to_decimal', input: '0xff' }),
    'upeg://open?surface=ext&board=dev&tool=num.hex_to_decimal&input=0xff',
  );
  assert.equal(buildDeepLink(), 'upeg://open?surface=ext');
  assert.equal(buildDeepLink({ board: 'dev' }), 'upeg://open?surface=ext&board=dev');
});

test('deep_link_values_percent_encode_reserved_characters', () => {
  assert.equal(
    buildDeepLink({ board: 'dev', toolId: 'convert.base64_decode', input: 'a+b/c==' }),
    'upeg://open?surface=ext&board=dev&tool=convert.base64_decode&input=a%2Bb%2Fc%3D%3D',
  );
});

test('empty_and_whitespace_only_values_are_not_carried_into_the_deep_link', () => {
  assert.equal(buildDeepLink({ board: '  ', toolId: '', input: null }), 'upeg://open?surface=ext');
});

test('the_authorization_header_is_attached_only_when_a_token_exists', () => {
  assert.deepEqual(authHeaders('tok'), { Accept: 'application/json', Authorization: 'Bearer tok' });
  assert.deepEqual(authHeaders(null), { Accept: 'application/json' });
  assert.deepEqual(authHeaders(''), { Accept: 'application/json' });
});

test('the_extension_tool_path_encodes_the_board_and_id', () => {
  assert.equal(toolCallPath('dev', 'num.hex_to_decimal'), '/v1/ext/boards/dev/tools/num.hex_to_decimal');
  assert.equal(toolCallPath('board/a', 'a/b'), '/v1/ext/boards/board%2Fa/tools/a%2Fb');
});

test('401_and_403_are_classified_as_auth_failures', () => {
  for (const status of [HTTP_STATUS.UNAUTHORIZED, HTTP_STATUS.FORBIDDEN]) {
    assert.equal(isAuthStatus(status), true);
    assert.equal(classifyDispatchResponse(status, null).kind, DISPATCH_RESULT_KIND.AUTH_ERROR);
  }
  assert.equal(isAuthStatus(200), false);
});

test('503_carries_the_server_hint_and_becomes_host_unavailable', () => {
  assert.deepEqual(
    classifyDispatchResponse(HTTP_STATUS.SERVICE_UNAVAILABLE, { hint: 'shutting down' }),
    { kind: DISPATCH_RESULT_KIND.HOST_UNAVAILABLE, hint: 'shutting down' },
  );
  assert.deepEqual(classifyDispatchResponse(HTTP_STATUS.SERVICE_UNAVAILABLE, null), {
    kind: DISPATCH_RESULT_KIND.HOST_UNAVAILABLE,
    hint: null,
  });
});

test('canonical_success_and_failure_envelopes_are_classified_into_their_kinds', () => {
  const success = { ok: true, primary_output_id: 'result', outputs: [] };
  assert.deepEqual(classifyDispatchResponse(200, success), {
    kind: DISPATCH_RESULT_KIND.SUCCESS,
    result: success,
  });

  const failure = { ok: false, error: { code: 'invalid', message: 'invalid hex' } };
  assert.deepEqual(classifyDispatchResponse(422, failure), {
    kind: DISPATCH_RESULT_KIND.FAILURE,
    error: failure.error,
  });
});

test('a_bodiless_error_is_classified_as_failure_carrying_only_the_status', () => {
  // host_api.js does not know this surface's message catalog, so it hands
  // the status back and popup.js renders it.
  const outcome = classifyDispatchResponse(500, null);
  assert.equal(outcome.kind, DISPATCH_RESULT_KIND.FAILURE);
  assert.equal(outcome.status, 500);
});

test('a_dropped_network_is_classified_as_network_error_and_the_exception_does_not_leak', async () => {
  const outcome = await callTool({
    fetchImpl: async () => {
      throw new TypeError('Failed to fetch');
    },
    toolId: 'num.hex_to_decimal',
    args: { input: '0xff' },
  });
  assert.deepEqual(outcome, { kind: DISPATCH_RESULT_KIND.NETWORK_ERROR });
});

test('a_body_over_the_request_cap_becomes_request_too_large_before_sending', async () => {
  let called = false;
  const outcome = await callTool({
    fetchImpl: async () => {
      called = true;
      throw new Error('unreachable');
    },
    toolId: 'text.repeat',
    args: { input: 'x'.repeat(1_000_001) },
  });
  assert.equal(called, false, 'an oversized body must never leave the extension');
  assert.equal(outcome.kind, DISPATCH_RESULT_KIND.REQUEST_TOO_LARGE);
  assert.ok(outcome.actualBytes > outcome.limitBytes - 1);
});

test('dispatch_posts_to_the_extension_board_route_with_a_bearer_token_and_a_json_body', async () => {
  const seen = [];
  const outcome = await callTool({
    fetchImpl: async (url, init) => {
      seen.push([url, init]);
      return { status: 200, json: async () => ({ ok: true, outputs: [] }) };
    },
    baseUrl: 'http://127.0.0.1:7173',
    token: 'test-tok',
    board: 'dev',
    toolId: 'num.hex_to_decimal',
    args: { input: '0xff' },
  });

  assert.equal(outcome.kind, DISPATCH_RESULT_KIND.SUCCESS);
  const [url, init] = seen[0];
  assert.equal(url, 'http://127.0.0.1:7173/v1/ext/boards/dev/tools/num.hex_to_decimal');
  assert.equal(init.method, 'POST');
  assert.equal(init.headers.Authorization, 'Bearer test-tok');
  assert.equal(init.headers['Content-Type'], 'application/json');
  assert.deepEqual(JSON.parse(init.body), { input: '0xff' });
});

test('dispatch_against_a_configured_endpoint_uses_that_base_url', async () => {
  // `upeg http` binds an ephemeral port, so the endpoint the popup saved
  // must be the origin every request targets.
  const seen = [];
  const endpoint = parseLoopbackEndpoint('127.0.0.1:49317').endpoint;
  const outcome = await callTool({
    fetchImpl: async (url) => {
      seen.push(url);
      return { status: 200, json: async () => ({ ok: true, outputs: [] }) };
    },
    baseUrl: endpoint.baseUrl,
    board: 'dev',
    toolId: 'num.hex_to_decimal',
    args: {},
  });
  assert.equal(outcome.kind, DISPATCH_RESULT_KIND.SUCCESS);
  assert.equal(seen[0], 'http://127.0.0.1:49317/v1/ext/boards/dev/tools/num.hex_to_decimal');
});

test('primary_output_picks_the_declared_id_and_falls_back_to_the_first_row', () => {
  const outputs = [
    { id: 'extra', value: 'no' },
    { id: 'result', value: '255' },
  ];
  assert.equal(primaryOutput({ primary_output_id: 'result', outputs }).value, '255');
  assert.equal(primaryOutput({ primary_output_id: 'gone', outputs }).value, 'no');
  assert.equal(primaryOutput({ outputs: [] }), null);
  assert.equal(primaryOutput({}), null);
});

test('output_text_keeps_strings_and_compacts_the_rest_to_json', () => {
  assert.equal(outputText('255'), '255');
  assert.equal(outputText(255), '255');
  assert.equal(outputText({ a: 1 }), '{"a":1}');
  assert.equal(outputText(null), '');
  assert.equal(outputText(undefined), '');
});

test('runtime_message_kinds_are_fixed_names_used_only_inside_the_extension', () => {
  const values = Object.values(RUNTIME_MESSAGE);
  assert.equal(new Set(values).size, values.length);
  for (const value of values) {
    assert.match(value, /^upeg:/);
  }
});

// === Configurable endpoint ===

test('the_default_endpoint_stays_the_legacy_127_0_0_1_7173', () => {
  assert.equal(DEFAULT_HTTP_BASE_URL, 'http://127.0.0.1:7173');
  assert.equal(DEFAULT_HTTP_ADDRESS, '127.0.0.1:7173');
  assert.equal(DEFAULT_ENDPOINT.baseUrl, 'http://127.0.0.1:7173');
  assert.equal(DEFAULT_ENDPOINT.address, '127.0.0.1:7173');
  assert.equal(DEFAULT_ENDPOINT.permissionPattern, 'http://127.0.0.1:7173/*');
});

test('the_pairing_command_matches_the_cli_status_flag', () => {
  // upeg-cli prints the running host's URL + token for
  // `upeg http status --pairing`; the run hint points there.
  assert.equal(PAIRING_COMMAND, 'upeg http status --pairing');
  assert.equal(RUN_HINT_COMMAND, 'upeg http');
});

test('the_endpoint_parser_accepts_url_host_port_and_localhost_spellings', () => {
  for (const input of [
    'http://127.0.0.1:49317',
    '127.0.0.1:49317',
    'localhost:8080',
    'http://localhost:8080',
    '[::1]:9000',
    'http://[::1]:9000',
  ]) {
    const parsed = parseLoopbackEndpoint(input);
    assert.equal(parsed.ok, true, input);
  }
  const parsed = parseLoopbackEndpoint('localhost:8080');
  assert.equal(parsed.endpoint.baseUrl, 'http://localhost:8080');
  assert.equal(parsed.endpoint.address, 'localhost:8080');
  assert.equal(parsed.endpoint.host, 'localhost');
  assert.equal(parsed.endpoint.port, 8080);
  assert.equal(parsed.endpoint.permissionPattern, 'http://localhost:8080/*');
});

test('an_omitted_port_means_the_default_7173_so_a_bare_host_keeps_working', () => {
  for (const input of ['127.0.0.1', 'localhost', 'http://127.0.0.1']) {
    const parsed = parseLoopbackEndpoint(input);
    assert.equal(parsed.ok, true, input);
    assert.equal(parsed.endpoint.port, 7173, input);
  }
});

test('the_endpoint_parser_normalizes_ipv4_spellings_to_the_dotted_quad', () => {
  // `new URL` canonicalizes "127.1" and "0x7f.0.0.1" to 127.0.0.1 — the
  // loopback check must see the normalized host, not the raw spelling.
  const parsed = parseLoopbackEndpoint('http://127.1:8080');
  assert.equal(parsed.ok, true);
  assert.equal(parsed.endpoint.host, '127.0.0.1');
  assert.equal(parsed.endpoint.baseUrl, 'http://127.0.0.1:8080');
});

test('the_endpoint_parser_rejects_everything_that_is_not_plain_loopback_http', () => {
  const cases = [
    ['https://127.0.0.1:7173', ENDPOINT_ERROR.NOT_HTTP],
    ['http://example.com:7173', ENDPOINT_ERROR.NOT_LOOPBACK],
    ['http://10.0.0.5:7173', ENDPOINT_ERROR.NOT_LOOPBACK],
    ['http://[::2]:7173', ENDPOINT_ERROR.NOT_LOOPBACK],
    ['http://user:pw@127.0.0.1:7173', ENDPOINT_ERROR.CREDENTIALS],
    ['http://127.0.0.1:7173/v1/boards', ENDPOINT_ERROR.PATH_OR_QUERY],
    ['http://127.0.0.1:7173?x=1', ENDPOINT_ERROR.PATH_OR_QUERY],
    ['http://127.0.0.1:notaport', ENDPOINT_ERROR.BAD_PORT],
    ['http://127.0.0.1:0', ENDPOINT_ERROR.BAD_PORT],
    ['http://127.0.0.1:99999', ENDPOINT_ERROR.BAD_PORT],
    ['', ENDPOINT_ERROR.NOT_A_URL],
    ['   ', ENDPOINT_ERROR.NOT_A_URL],
    ['::not a url::', ENDPOINT_ERROR.NOT_A_URL],
  ];
  for (const [input, error] of cases) {
    const parsed = parseLoopbackEndpoint(input);
    assert.equal(parsed.ok, false, input);
    assert.equal(parsed.error, error, input);
  }
  assert.equal(parseLoopbackEndpoint(undefined).ok, false);
  assert.equal(parseLoopbackEndpoint(7173).ok, false);
});

test('endpointForStoredValue_returns_the_default_for_missing_or_corrupt_values', () => {
  for (const stored of [undefined, null, '', 'https://evil.example', 42, {}]) {
    assert.equal(endpointForStoredValue(stored), DEFAULT_ENDPOINT, String(stored));
  }
  // A valid persisted value survives the round trip.
  const stored = parseLoopbackEndpoint('127.0.0.1:49317').endpoint.baseUrl;
  assert.equal(endpointForStoredValue(stored).baseUrl, 'http://127.0.0.1:49317');
});

test('readHostEndpoint_falls_back_to_the_default_when_nothing_is_stored', async () => {
  // Legacy installs have only the token key — a missing endpoint must
  // migrate to the default, not break the existing pairing.
  const storage = { get: async () => ({}) };
  assert.equal(await readHostEndpoint(storage), DEFAULT_ENDPOINT);
});

test('readHostEndpoint_returns_the_stored_endpoint_and_survives_storage_errors', async () => {
  const storage = {
    get: async (key) => {
      assert.equal(key, ENDPOINT_STORAGE_KEY);
      return { [ENDPOINT_STORAGE_KEY]: 'http://127.0.0.1:49317' };
    },
  };
  const endpoint = await readHostEndpoint(storage);
  assert.equal(endpoint.baseUrl, 'http://127.0.0.1:49317');
  assert.equal(endpoint.address, '127.0.0.1:49317');

  const broken = {
    get: async () => {
      throw new Error('storage unavailable');
    },
  };
  assert.equal(await readHostEndpoint(broken), DEFAULT_ENDPOINT);
});
