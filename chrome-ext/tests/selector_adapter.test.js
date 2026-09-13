'use strict';

// In-page selector adapter (chrome-ext/selector_adapter.js) — the extension
// counterpart of `upeg_runtime::selector_pipeline`.
//
// jsdom is not a dependency of this repo (the extension ships zero npm
// deps), so DOM application is exercised against a tiny fake element tree
// that implements exactly the surface the adapter is allowed to touch:
// `querySelector`, `.value`, `.textContent`, `.click()`, `.focus()`,
// `.dispatchEvent()`. Anything the adapter reached for beyond that would
// throw here instead of silently working only in a browser.

const assert = require('node:assert/strict');
const test = require('node:test');

const {
  BINDING_ROLE,
  ENTER_KEY_EVENTS,
  INPUT_WRITE_EVENTS,
  TRIGGER_ACTION,
  applySelectorPlan,
  isActionable,
  planSelectorApplication,
} = require('../selector_adapter.js');

// --- fake element tree -------------------------------------------------

const FAKE_EVENT_FACTORY = {
  bubbling: (type) => ({ type, bubbles: true }),
  enterKey: (type) => ({ type, key: 'Enter', bubbles: true, cancelable: true }),
};

function fakeElement({ value, textContent } = {}) {
  return {
    value,
    textContent,
    events: [],
    clicks: 0,
    focuses: 0,
    click() {
      this.clicks += 1;
    },
    focus() {
      this.focuses += 1;
    },
    dispatchEvent(event) {
      this.events.push(event.type);
    },
  };
}

function fakeRoot(elementsBySelector) {
  return {
    elements: elementsBySelector,
    querySelector(selector) {
      return Object.prototype.hasOwnProperty.call(elementsBySelector, selector)
        ? elementsBySelector[selector]
        : null;
    },
  };
}

const binding = (role, field, selector, action) => ({ role, field, selector, action });

// --- planning (pure) ---------------------------------------------------

test('계획은_input_trigger_output_바인딩을_역할별로_나눈다', () => {
  const plan = planSelectorApplication(
    [
      binding(BINDING_ROLE.INPUT, 'query', '#q', TRIGGER_ACTION.CLICK),
      binding(BINDING_ROLE.TRIGGER, '', '#go', TRIGGER_ACTION.CLICK),
      binding(BINDING_ROLE.OUTPUT, 'answer', '#out', TRIGGER_ACTION.CLICK),
    ],
    { query: 'upeg' },
  );

  assert.deepEqual(plan.writes, [{ selector: '#q', field: 'query', value: 'upeg' }]);
  assert.deepEqual(plan.trigger, { selector: '#go', action: TRIGGER_ACTION.CLICK });
  assert.deepEqual(plan.reads, [{ selector: '#out', field: 'answer' }]);
  assert.equal(isActionable(plan), true);
});

test('인자가_없는_input_바인딩은_조용히_빠진다', () => {
  // Mirrors `ExecutionPlan::build`: the page just keeps whatever was there.
  const plan = planSelectorApplication(
    [
      binding(BINDING_ROLE.INPUT, 'query', '#q', TRIGGER_ACTION.CLICK),
      binding(BINDING_ROLE.INPUT, 'missing', '#m', TRIGGER_ACTION.CLICK),
    ],
    { query: 'upeg', unbound: 'ignored' },
  );

  assert.deepEqual(
    plan.writes.map((write) => write.field),
    ['query'],
  );
});

test('첫_trigger가_이기고_나머지는_무시된다', () => {
  const plan = planSelectorApplication(
    [
      binding(BINDING_ROLE.TRIGGER, '', '#first', TRIGGER_ACTION.ENTER),
      binding(BINDING_ROLE.TRIGGER, '', '#second', TRIGGER_ACTION.CLICK),
    ],
    {},
  );

  assert.deepEqual(plan.trigger, { selector: '#first', action: TRIGGER_ACTION.ENTER });
});

test('빈_selector와_알_수_없는_역할과_비배열_입력은_계획을_만들지_않는다', () => {
  for (const bindings of [
    null,
    undefined,
    'not-an-array',
    [binding(BINDING_ROLE.INPUT, 'q', '', TRIGGER_ACTION.CLICK)],
    [binding('teleport', 'q', '#q', TRIGGER_ACTION.CLICK)],
  ]) {
    const plan = planSelectorApplication(bindings, { q: 'x' });
    assert.equal(isActionable(plan), false);
  }
});

test('숫자와_불리언_인자는_문자열로_적힌다', () => {
  const plan = planSelectorApplication(
    [
      binding(BINDING_ROLE.INPUT, 'count', '#count', TRIGGER_ACTION.CLICK),
      binding(BINDING_ROLE.INPUT, 'flag', '#flag', TRIGGER_ACTION.CLICK),
      binding(BINDING_ROLE.INPUT, 'nothing', '#nothing', TRIGGER_ACTION.CLICK),
    ],
    { count: 7, flag: false, nothing: null },
  );

  assert.deepEqual(
    plan.writes.map((write) => [write.field, write.value]),
    [
      ['count', '7'],
      ['flag', 'false'],
    ],
  );
});

// --- application (fake DOM) --------------------------------------------

test('계획_적용은_필드를_채우고_input_change_이벤트를_발생시킨다', () => {
  const field = fakeElement({ value: '' });
  const root = fakeRoot({ '#q': field });
  const plan = planSelectorApplication(
    [binding(BINDING_ROLE.INPUT, 'query', '#q', TRIGGER_ACTION.CLICK)],
    { query: 'upeg' },
  );

  const applied = applySelectorPlan(plan, root, FAKE_EVENT_FACTORY);

  assert.equal(field.value, 'upeg');
  assert.deepEqual(field.events, [...INPUT_WRITE_EVENTS]);
  assert.deepEqual(applied.written, ['query']);
  assert.deepEqual(applied.missing, []);
});

test('click_trigger는_클릭하고_enter_trigger는_포커스_후_키_이벤트를_보낸다', () => {
  const clickTarget = fakeElement();
  const clicked = applySelectorPlan(
    planSelectorApplication(
      [binding(BINDING_ROLE.TRIGGER, '', '#go', TRIGGER_ACTION.CLICK)],
      {},
    ),
    fakeRoot({ '#go': clickTarget }),
    FAKE_EVENT_FACTORY,
  );
  assert.equal(clickTarget.clicks, 1);
  assert.deepEqual(clickTarget.events, []);
  assert.equal(clicked.triggered, true);

  const enterTarget = fakeElement();
  applySelectorPlan(
    planSelectorApplication(
      [binding(BINDING_ROLE.TRIGGER, '', '#go', TRIGGER_ACTION.ENTER)],
      {},
    ),
    fakeRoot({ '#go': enterTarget }),
    FAKE_EVENT_FACTORY,
  );
  assert.equal(enterTarget.focuses, 1);
  assert.equal(enterTarget.clicks, 0);
  assert.deepEqual(enterTarget.events, [...ENTER_KEY_EVENTS]);
});

test('output_읽기는_value를_우선하고_없으면_textContent를_쓴다', () => {
  const root = fakeRoot({
    '#withValue': fakeElement({ value: 'from-value', textContent: 'ignored' }),
    '#withText': fakeElement({ value: '', textContent: 'from-text' }),
    '#plain': fakeElement({ textContent: 'plain-text' }),
  });
  const plan = planSelectorApplication(
    [
      binding(BINDING_ROLE.OUTPUT, 'a', '#withValue', TRIGGER_ACTION.CLICK),
      binding(BINDING_ROLE.OUTPUT, 'b', '#withText', TRIGGER_ACTION.CLICK),
      binding(BINDING_ROLE.OUTPUT, 'c', '#plain', TRIGGER_ACTION.CLICK),
      binding(BINDING_ROLE.OUTPUT, 'd', '#gone', TRIGGER_ACTION.CLICK),
    ],
    {},
  );

  const applied = applySelectorPlan(plan, root, FAKE_EVENT_FACTORY);

  assert.deepEqual(applied.outputs, {
    a: 'from-value',
    b: 'from-text',
    c: 'plain-text',
    d: '',
  });
  assert.deepEqual(applied.missing, ['#gone']);
});

test('일치하는_요소가_없으면_missing에_기록하고_계속_진행한다', () => {
  const present = fakeElement({ value: '' });
  const root = fakeRoot({ '#present': present });
  const plan = planSelectorApplication(
    [
      binding(BINDING_ROLE.INPUT, 'gone', '#gone', TRIGGER_ACTION.CLICK),
      binding(BINDING_ROLE.INPUT, 'here', '#present', TRIGGER_ACTION.CLICK),
      binding(BINDING_ROLE.TRIGGER, '', '#no-button', TRIGGER_ACTION.CLICK),
    ],
    { gone: 'x', here: 'y' },
  );

  const applied = applySelectorPlan(plan, root, FAKE_EVENT_FACTORY);

  assert.equal(present.value, 'y');
  assert.deepEqual(applied.written, ['here']);
  assert.equal(applied.triggered, false);
  assert.deepEqual(applied.missing, ['#gone', '#no-button']);
});
