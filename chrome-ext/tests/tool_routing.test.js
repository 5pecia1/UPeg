'use strict';

// Activation routing (chrome-ext/tool_routing.js) — which of IN_PAGE /
// DISPATCH / DEEP_LINK a pinned tool takes when the user activates it.

const assert = require('node:assert/strict');
const test = require('node:test');

const {
  ACTIVATION_ROUTE,
  PIN_KIND,
  TOOL_FIELD,
  activationRouteFor,
  isControlledEmbed,
  toolInputProperties,
  toolLabel,
  toolRequiredFields,
  toolSelectorBindings,
} = require('../tool_routing.js');

const tool = (overrides) => ({
  [TOOL_FIELD.ID]: 'num.hex_to_decimal',
  [TOOL_FIELD.LABEL]: 'Hex → Decimal',
  [TOOL_FIELD.INVOKER]: 'function',
  [TOOL_FIELD.PIN]: 'Inline',
  [TOOL_FIELD.INPUT_SCHEMA]: { type: 'object', properties: {}, required: [] },
  [TOOL_FIELD.SELECTOR_BINDINGS]: [],
  ...overrides,
});

const BINDINGS = [
  { role: 'input', field: 'query', selector: '#q', action: 'click' },
  { role: 'trigger', field: '', selector: '#go', action: 'click' },
];

test('평범한_invoker는_호스트로_직접_디스패치한다', () => {
  for (const invoker of ['function', 'external', 'http', 'chain', 'llm', 'wasm']) {
    assert.equal(
      activationRouteFor(tool({ [TOOL_FIELD.INVOKER]: invoker }), { inPageAvailable: true }),
      ACTIVATION_ROUTE.DISPATCH,
      invoker,
    );
  }
});

test('static_invoker는_열린_사이트에서도_deep_link를_유지한다', () => {
  // Passive Embed: the webview IS the tool, there is nothing to POST and
  // nothing to fill in.
  assert.equal(
    activationRouteFor(tool({ [TOOL_FIELD.INVOKER]: 'static' }), { inPageAvailable: true }),
    ACTIVATION_ROUTE.DEEP_LINK,
  );
});

test('controlled_embed_핀은_사이트가_켜져_있으면_페이지_안에서_실행된다', () => {
  const controlled = tool({
    [TOOL_FIELD.INVOKER]: 'embed',
    [TOOL_FIELD.PIN]: PIN_KIND.CONTROLLED_EMBED,
    [TOOL_FIELD.SELECTOR_BINDINGS]: BINDINGS,
  });

  assert.equal(isControlledEmbed(controlled), true);
  assert.equal(
    activationRouteFor(controlled, { inPageAvailable: true }),
    ACTIVATION_ROUTE.IN_PAGE,
  );
});

test('controlled_embed_핀은_사이트가_꺼져_있으면_deep_link로_되돌아간다', () => {
  const controlled = tool({
    [TOOL_FIELD.INVOKER]: 'embed',
    [TOOL_FIELD.PIN]: PIN_KIND.CONTROLLED_EMBED,
    [TOOL_FIELD.SELECTOR_BINDINGS]: BINDINGS,
  });

  assert.equal(
    activationRouteFor(controlled, { inPageAvailable: false }),
    ACTIVATION_ROUTE.DEEP_LINK,
  );
  // Missing context is the same as "no page to drive" — never IN_PAGE.
  assert.equal(activationRouteFor(controlled), ACTIVATION_ROUTE.DEEP_LINK);
});

test('바인딩이_없는_controlled_embed는_페이지에서_할_일이_없어_deep_link로_간다', () => {
  const controlled = tool({
    [TOOL_FIELD.INVOKER]: 'embed',
    [TOOL_FIELD.PIN]: PIN_KIND.CONTROLLED_EMBED,
    [TOOL_FIELD.SELECTOR_BINDINGS]: [],
  });

  assert.equal(
    activationRouteFor(controlled, { inPageAvailable: true }),
    ACTIVATION_ROUTE.DEEP_LINK,
  );
});

test('embed_invoker인데_controlled_핀이_아니면_deep_link로_간다', () => {
  // `POST /v1/tools/{id}` cannot answer for an Embed invoker: it needs a
  // browser to drive, and this pin declares no page work.
  const embed = tool({ [TOOL_FIELD.INVOKER]: 'embed', [TOOL_FIELD.PIN]: 'Embed' });
  assert.equal(activationRouteFor(embed, { inPageAvailable: true }), ACTIVATION_ROUTE.DEEP_LINK);
});

test('도구_메타데이터_접근자는_빠진_필드에_안전한_기본값을_준다', () => {
  const bare = { [TOOL_FIELD.ID]: 'x.y' };
  assert.equal(toolLabel(bare), 'x.y');
  assert.deepEqual(toolInputProperties(bare), {});
  assert.deepEqual(toolRequiredFields(bare), []);
  assert.deepEqual(toolSelectorBindings(bare), []);

  const full = tool({
    [TOOL_FIELD.INPUT_SCHEMA]: { properties: { input: { type: 'string' } }, required: ['input'] },
    [TOOL_FIELD.SELECTOR_BINDINGS]: BINDINGS,
  });
  assert.deepEqual(Object.keys(toolInputProperties(full)), ['input']);
  assert.deepEqual(toolRequiredFields(full), ['input']);
  assert.equal(toolSelectorBindings(full).length, 2);
});
