'use strict';

// Host wire (chrome-ext/host_api.js) — deep-link construction and the
// dispatch classification popup.js and background.js share.

const assert = require('node:assert/strict');
const test = require('node:test');

const {
  CONTENT_SCRIPT_BOARD,
  DISPATCH_RESULT_KIND,
  HTTP_STATUS,
  RUNTIME_MESSAGE,
  authHeaders,
  buildDeepLink,
  callTool,
  classifyDispatchResponse,
  isAuthStatus,
  outputText,
  primaryOutput,
  toolCallPath,
} = require('../host_api.js');

test('deep_link는_rust_desktop_deep_link와_같은_순서와_인코딩을_쓴다', () => {
  // `upeg_pegboard_ui::deep_link::desktop_deep_link` emits surface, board,
  // tool, input in this order with the RFC 3986 unreserved set kept.
  assert.equal(
    buildDeepLink({ board: CONTENT_SCRIPT_BOARD, toolId: 'num.hex_to_decimal', input: '0xff' }),
    'upeg://open?surface=ext&board=dev&tool=num.hex_to_decimal&input=0xff',
  );
  assert.equal(buildDeepLink(), 'upeg://open?surface=ext');
  assert.equal(buildDeepLink({ board: 'dev' }), 'upeg://open?surface=ext&board=dev');
});

test('deep_link_값은_예약_문자를_퍼센트_인코딩한다', () => {
  assert.equal(
    buildDeepLink({ board: 'dev', toolId: 'convert.base64_decode', input: 'a+b/c==' }),
    'upeg://open?surface=ext&board=dev&tool=convert.base64_decode&input=a%2Bb%2Fc%3D%3D',
  );
});

test('빈_값과_공백만_있는_값은_deep_link에_실리지_않는다', () => {
  assert.equal(buildDeepLink({ board: '  ', toolId: '', input: null }), 'upeg://open?surface=ext');
});

test('토큰이_있을_때만_authorization_헤더가_붙는다', () => {
  assert.deepEqual(authHeaders('tok'), { Accept: 'application/json', Authorization: 'Bearer tok' });
  assert.deepEqual(authHeaders(null), { Accept: 'application/json' });
  assert.deepEqual(authHeaders(''), { Accept: 'application/json' });
});

test('도구_경로는_id를_인코딩한다', () => {
  assert.equal(toolCallPath('num.hex_to_decimal'), '/v1/tools/num.hex_to_decimal');
  assert.equal(toolCallPath('a/b'), '/v1/tools/a%2Fb');
});

test('401과_403은_인증_실패로_분류된다', () => {
  for (const status of [HTTP_STATUS.UNAUTHORIZED, HTTP_STATUS.FORBIDDEN]) {
    assert.equal(isAuthStatus(status), true);
    assert.equal(classifyDispatchResponse(status, null).kind, DISPATCH_RESULT_KIND.AUTH_ERROR);
  }
  assert.equal(isAuthStatus(200), false);
});

test('503은_서버_hint를_그대로_실어_host_unavailable이_된다', () => {
  assert.deepEqual(
    classifyDispatchResponse(HTTP_STATUS.SERVICE_UNAVAILABLE, { hint: 'shutting down' }),
    { kind: DISPATCH_RESULT_KIND.HOST_UNAVAILABLE, hint: 'shutting down' },
  );
  assert.deepEqual(classifyDispatchResponse(HTTP_STATUS.SERVICE_UNAVAILABLE, null), {
    kind: DISPATCH_RESULT_KIND.HOST_UNAVAILABLE,
    hint: null,
  });
});

test('canonical_성공과_실패_envelope는_각각의_종류로_분류된다', () => {
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

test('본문이_없는_오류는_상태_코드만_실어_실패로_분류된다', () => {
  // host_api.js does not know this surface's message catalog, so it hands
  // the status back and popup.js renders it.
  const outcome = classifyDispatchResponse(500, null);
  assert.equal(outcome.kind, DISPATCH_RESULT_KIND.FAILURE);
  assert.equal(outcome.status, 500);
});

test('네트워크가_끊기면_network_error로_분류되고_예외는_새지_않는다', async () => {
  const outcome = await callTool({
    fetchImpl: async () => {
      throw new TypeError('Failed to fetch');
    },
    toolId: 'num.hex_to_decimal',
    args: { input: '0xff' },
  });
  assert.deepEqual(outcome, { kind: DISPATCH_RESULT_KIND.NETWORK_ERROR });
});

test('본문이_요청_상한을_넘으면_전송_전에_request_too_large가_된다', async () => {
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

test('디스패치는_bearer_토큰과_json_본문으로_v1_tools에_post한다', async () => {
  const seen = [];
  const outcome = await callTool({
    fetchImpl: async (url, init) => {
      seen.push([url, init]);
      return { status: 200, json: async () => ({ ok: true, outputs: [] }) };
    },
    baseUrl: 'http://127.0.0.1:7173',
    token: 'test-tok',
    toolId: 'num.hex_to_decimal',
    args: { input: '0xff' },
  });

  assert.equal(outcome.kind, DISPATCH_RESULT_KIND.SUCCESS);
  const [url, init] = seen[0];
  assert.equal(url, 'http://127.0.0.1:7173/v1/tools/num.hex_to_decimal');
  assert.equal(init.method, 'POST');
  assert.equal(init.headers.Authorization, 'Bearer test-tok');
  assert.equal(init.headers['Content-Type'], 'application/json');
  assert.deepEqual(JSON.parse(init.body), { input: '0xff' });
});

test('primary_output은_선언된_id를_고르고_없으면_첫_행으로_되돌아간다', () => {
  const outputs = [
    { id: 'extra', value: 'no' },
    { id: 'result', value: '255' },
  ];
  assert.equal(primaryOutput({ primary_output_id: 'result', outputs }).value, '255');
  assert.equal(primaryOutput({ primary_output_id: 'gone', outputs }).value, 'no');
  assert.equal(primaryOutput({ outputs: [] }), null);
  assert.equal(primaryOutput({}), null);
});

test('출력_텍스트는_문자열을_그대로_두고_나머지를_compact_json으로_만든다', () => {
  assert.equal(outputText('255'), '255');
  assert.equal(outputText(255), '255');
  assert.equal(outputText({ a: 1 }), '{"a":1}');
  assert.equal(outputText(null), '');
  assert.equal(outputText(undefined), '');
});

test('런타임_메시지_종류는_확장_안에서만_쓰이는_고정_이름이다', () => {
  const values = Object.values(RUNTIME_MESSAGE);
  assert.equal(new Set(values).size, values.length);
  for (const value of values) {
    assert.match(value, /^upeg:/);
  }
});
