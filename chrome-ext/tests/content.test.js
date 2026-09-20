'use strict';

// In-page content script (chrome-ext/content.js) — the two invariants it
// keeps about a page it did not author:
//
//   1. It lives once per frame. The script arrives by two paths (the
//      dynamic `registerContentScripts` registration and the popup's
//      `executeScript` on the current tab), so a second evaluation in the
//      same frame is normal, and it must not double the runtime-message
//      listener or the MutationObserver.
//   2. It never rewrites text in a skipped context (`<textarea>`, `<code>`,
//      `<pre>`, …) — whichever path found that text: the initial tree walk
//      or the MutationObserver.
//
// content.js is a browser IIFE that reads globals and exports nothing, so
// it is evaluated inside a `node:vm` context built from the fakes below.
// That is also the only way to evaluate it TWICE and watch what the second
// evaluation does. The decision modules it depends on are the REAL ones
// (detectors.js / host_api.js / selector_adapter.js) — only the browser is
// fake.

const assert = require('node:assert/strict');
const test = require('node:test');
const { readFileSync } = require('node:fs');
const { join } = require('node:path');
const vm = require('node:vm');

const UpegHostApi = require('../host_api.js');
const UpegDetectors = require('../detectors.js');
const UpegSelectorAdapter = require('../selector_adapter.js');

const CONTENT_JS_PATH = join(__dirname, '..', 'content.js');
const CONTENT_JS = readFileSync(CONTENT_JS_PATH, 'utf8');

const ELEMENT_NODE = 1;
const TEXT_NODE = 3;
const MARK_CLASS = 'upeg-mark';
const SENTINEL_KEY = 'upegContentScriptActive';
// A hex literal detectors.js has a row for, so a wrap either happens or
// is provably suppressed — never "nothing matched anyway".
const DETECTED_TEXT = 'balance 0xff wei';

// --- fake DOM ----------------------------------------------------------
//
// Only what content.js touches. Nodes carry `parentNode`, so the ancestor
// walk `isSkippedContext` does is the real one.

class FakeText {
  constructor(nodeValue) {
    this.nodeType = TEXT_NODE;
    this.nodeValue = nodeValue;
    this.parentNode = null;
  }
}

class FakeElement {
  constructor(tagName) {
    this.nodeType = ELEMENT_NODE;
    this.tagName = tagName.toUpperCase();
    this.parentNode = null;
    this.childNodes = [];
    this.attributes = new Map();
    this.listeners = [];
    this.id = '';
    this.textContent = '';
    this.className = '';
  }

  get classList() {
    const names = this.className.split(' ').filter((name) => name !== '');
    return { contains: (name) => names.includes(name) };
  }

  appendChild(node) {
    if (node instanceof FakeFragment) {
      for (const child of node.childNodes.slice()) this.appendChild(child);
      return node;
    }
    node.parentNode = this;
    this.childNodes.push(node);
    return node;
  }

  replaceChild(next, previous) {
    const at = this.childNodes.indexOf(previous);
    if (at < 0) throw new Error('replaceChild: node is not a child');
    const incoming = next instanceof FakeFragment ? next.childNodes : [next];
    for (const node of incoming) node.parentNode = this;
    this.childNodes.splice(at, 1, ...incoming);
    previous.parentNode = null;
    return previous;
  }

  setAttribute(name, value) {
    this.attributes.set(name, value);
  }

  getAttribute(name) {
    return this.attributes.has(name) ? this.attributes.get(name) : null;
  }

  addEventListener(type, handler) {
    this.listeners.push({ type, handler });
  }
}

class FakeFragment {
  constructor() {
    this.childNodes = [];
  }

  appendChild(node) {
    this.childNodes.push(node);
    return node;
  }
}

const NODE_FILTER = Object.freeze({
  SHOW_TEXT: 4,
  FILTER_ACCEPT: 1,
  FILTER_REJECT: 2,
});

/** Depth-first text-node walk that really consults `filter.acceptNode`. */
function fakeTreeWalker(root, filter) {
  const found = [];
  const visit = (node) => {
    if (node.nodeType === TEXT_NODE) {
      if (filter.acceptNode(node) === NODE_FILTER.FILTER_ACCEPT) found.push(node);
      return;
    }
    for (const child of node.childNodes ?? []) visit(child);
  };
  visit(root);
  let at = 0;
  return { nextNode: () => (at < found.length ? found[at++] : null) };
}

/** One page + the extension globals content.js reads. */
function makeBrowser() {
  const body = new FakeElement('body');
  const head = new FakeElement('head');
  const byId = new Map();
  const observers = [];
  const messageListeners = [];

  const document = {
    readyState: 'complete',
    body,
    head,
    documentElement: new FakeElement('html'),
    createElement: (tag) => new FakeElement(tag),
    createTextNode: (text) => new FakeText(text),
    createDocumentFragment: () => new FakeFragment(),
    getElementById: (id) => byId.get(id) ?? null,
    addEventListener: () => {},
    querySelectorAll: () => [],
    createTreeWalker: (root, _whatToShow, filter) => fakeTreeWalker(root, filter),
  };
  // ensureStyles() appends the style element and then looks it up by id on
  // the next evaluation; register whatever lands in <head> so the second
  // evaluation cannot silently re-add it.
  const appendToHead = head.appendChild.bind(head);
  head.appendChild = (node) => {
    if (node.id) byId.set(node.id, node);
    return appendToHead(node);
  };

  const chrome = {
    i18n: { getMessage: (key) => key },
    runtime: {
      onMessage: { addListener: (fn) => messageListeners.push(fn) },
      sendMessage: async () => null,
    },
  };

  class FakeMutationObserver {
    constructor(callback) {
      this.callback = callback;
      this.observed = [];
      observers.push(this);
    }

    observe(target, options) {
      this.observed.push({ target, options });
    }
  }

  return {
    body,
    head,
    observers,
    messageListeners,
    globals: {
      document,
      chrome,
      MutationObserver: FakeMutationObserver,
      NodeFilter: NODE_FILTER,
      UpegHostApi,
      UpegDetectors,
      UpegSelectorAdapter,
      console,
    },
  };
}

/** Evaluate content.js in a fresh frame; `evaluate()` re-runs it in place. */
function loadContentScript(browser) {
  const context = vm.createContext({ ...browser.globals });
  vm.runInContext('globalThis.window = globalThis;', context);
  const evaluate = () => vm.runInContext(CONTENT_JS, context, { filename: CONTENT_JS_PATH });
  evaluate();
  return { context, evaluate };
}

const elementWith = (tagName, text) => {
  const element = new FakeElement(tagName);
  element.appendChild(new FakeText(text));
  return element;
};

const marks = (element) =>
  element.childNodes.filter(
    (node) => node.nodeType === ELEMENT_NODE && node.className === MARK_CLASS,
  );

// --- tests -------------------------------------------------------------

test('reinjection_in_the_same_frame_keeps_one_listener_and_one_observer', () => {
  const browser = makeBrowser();
  const { context, evaluate } = loadContentScript(browser);

  assert.equal(context[SENTINEL_KEY], true, 'the first evaluation must leave the mark');
  assert.equal(browser.messageListeners.length, 1);
  assert.equal(browser.observers.length, 1);

  evaluate();
  evaluate();

  assert.equal(
    browser.messageListeners.length,
    1,
    'reinjection adding another onMessage listener would answer PING twice',
  );
  assert.equal(
    browser.observers.length,
    1,
    'reinjection adding another MutationObserver would scan each DOM change twice',
  );
});

test('the_first_scan_leaves_text_inside_skip_tags_alone', () => {
  const browser = makeBrowser();
  const skipped = elementWith('textarea', DETECTED_TEXT);
  const plain = elementWith('div', DETECTED_TEXT);
  browser.body.appendChild(skipped);
  browser.body.appendChild(plain);

  loadContentScript(browser);

  assert.equal(skipped.childNodes.length, 1);
  assert.equal(skipped.childNodes[0].nodeType, TEXT_NODE);
  assert.equal(skipped.childNodes[0].nodeValue, DETECTED_TEXT);
  assert.equal(marks(plain).length, 1, 'normal context must still be wrapped');
});

test('mutation_delivered_text_inside_skip_tags_is_left_alone', () => {
  const browser = makeBrowser();
  loadContentScript(browser);
  const [observer] = browser.observers;

  const skipped = new FakeElement('code');
  browser.body.appendChild(skipped);
  const added = new FakeText(DETECTED_TEXT);
  skipped.appendChild(added);

  observer.callback([{ addedNodes: [added] }]);

  assert.equal(skipped.childNodes.length, 1);
  assert.equal(skipped.childNodes[0], added);
  assert.equal(added.nodeValue, DETECTED_TEXT);
});

test('mutation_delivered_text_in_normal_context_is_wrapped', () => {
  const browser = makeBrowser();
  loadContentScript(browser);
  const [observer] = browser.observers;

  const host = new FakeElement('div');
  browser.body.appendChild(host);
  const added = new FakeText(DETECTED_TEXT);
  host.appendChild(added);

  observer.callback([{ addedNodes: [added] }]);

  assert.equal(marks(host).length, 1, 'if the mutation path detected nothing the two tests above would be moot');
});

test('mutation_delivered_elements_inside_skip_tags_are_not_touched', () => {
  const browser = makeBrowser();
  loadContentScript(browser);
  const [observer] = browser.observers;

  const skipped = elementWith('pre', DETECTED_TEXT);
  browser.body.appendChild(skipped);

  observer.callback([{ addedNodes: [skipped] }]);

  assert.equal(skipped.childNodes.length, 1);
  assert.equal(skipped.childNodes[0].nodeType, TEXT_NODE);
});

test('text_inside_an_already_wrapped_span_is_not_wrapped_again', () => {
  const browser = makeBrowser();
  loadContentScript(browser);
  const [observer] = browser.observers;

  const mark = new FakeElement('span');
  mark.className = MARK_CLASS;
  browser.body.appendChild(mark);
  const added = new FakeText(DETECTED_TEXT);
  mark.appendChild(added);

  observer.callback([{ addedNodes: [added] }]);

  assert.equal(mark.childNodes.length, 1);
  assert.equal(mark.childNodes[0], added);
});
