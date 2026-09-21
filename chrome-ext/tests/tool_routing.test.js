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

test('a_plain_invoker_dispatches_straight_to_the_host', () => {
  for (const invoker of ['function', 'external', 'http', 'chain', 'llm', 'wasm']) {
    assert.equal(
      activationRouteFor(tool({ [TOOL_FIELD.INVOKER]: invoker }), { inPageAvailable: true }),
      ACTIVATION_ROUTE.DISPATCH,
      invoker,
    );
  }
});

test('a_static_invoker_keeps_the_deep_link_even_on_an_enabled_site', () => {
  // Passive Embed: the webview IS the tool, there is nothing to POST and
  // nothing to fill in.
  assert.equal(
    activationRouteFor(tool({ [TOOL_FIELD.INVOKER]: 'static' }), { inPageAvailable: true }),
    ACTIVATION_ROUTE.DEEP_LINK,
  );
});

test('a_controlled_embed_pin_runs_in_page_when_the_site_is_enabled', () => {
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

test('a_controlled_embed_pin_falls_back_to_deep_link_when_the_site_is_off', () => {
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

test('a_controlled_embed_without_bindings_has_no_page_work_so_it_deep_links', () => {
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

test('an_embed_invoker_without_a_controlled_pin_deep_links', () => {
  // `POST /v1/tools/{id}` cannot answer for an Embed invoker: it needs a
  // browser to drive, and this pin declares no page work.
  const embed = tool({ [TOOL_FIELD.INVOKER]: 'embed', [TOOL_FIELD.PIN]: 'Embed' });
  assert.equal(activationRouteFor(embed, { inPageAvailable: true }), ACTIVATION_ROUTE.DEEP_LINK);
});

test('tool_metadata_accessors_give_safe_defaults_for_missing_fields', () => {
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
