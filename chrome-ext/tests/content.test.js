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

test('같은_프레임에서_다시_주입돼도_리스너와_옵저버는_하나씩만_남는다', () => {
  const browser = makeBrowser();
  const { context, evaluate } = loadContentScript(browser);

  assert.equal(context[SENTINEL_KEY], true, '첫 평가가 표식을 남겨야 한다');
  assert.equal(browser.messageListeners.length, 1);
  assert.equal(browser.observers.length, 1);

  evaluate();
  evaluate();

  assert.equal(
    browser.messageListeners.length,
    1,
    '재주입이 chrome.runtime.onMessage 리스너를 늘리면 PING에 여러 번 답한다',
  );
  assert.equal(
    browser.observers.length,
    1,
    '재주입이 MutationObserver를 늘리면 DOM 변경 하나를 여러 번 스캔한다',
  );
});

test('첫_스캔은_skip_태그_안의_텍스트를_건드리지_않는다', () => {
  const browser = makeBrowser();
  const skipped = elementWith('textarea', DETECTED_TEXT);
  const plain = elementWith('div', DETECTED_TEXT);
  browser.body.appendChild(skipped);
  browser.body.appendChild(plain);

  loadContentScript(browser);

  assert.equal(skipped.childNodes.length, 1);
  assert.equal(skipped.childNodes[0].nodeType, TEXT_NODE);
  assert.equal(skipped.childNodes[0].nodeValue, DETECTED_TEXT);
  assert.equal(marks(plain).length, 1, '일반 문맥은 그대로 감싸져야 한다');
});

test('mutation으로_들어온_텍스트도_skip_태그_안이면_건드리지_않는다', () => {
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

test('mutation으로_들어온_텍스트가_일반_문맥이면_감싼다', () => {
  const browser = makeBrowser();
  loadContentScript(browser);
  const [observer] = browser.observers;

  const host = new FakeElement('div');
  browser.body.appendChild(host);
  const added = new FakeText(DETECTED_TEXT);
  host.appendChild(added);

  observer.callback([{ addedNodes: [added] }]);

  assert.equal(marks(host).length, 1, 'mutation 경로가 감지를 아예 못 하면 위 두 테스트가 헛돈다');
});

test('mutation으로_들어온_엘리먼트도_skip_태그면_그_안을_건드리지_않는다', () => {
  const browser = makeBrowser();
  loadContentScript(browser);
  const [observer] = browser.observers;

  const skipped = elementWith('pre', DETECTED_TEXT);
  browser.body.appendChild(skipped);

  observer.callback([{ addedNodes: [skipped] }]);

  assert.equal(skipped.childNodes.length, 1);
  assert.equal(skipped.childNodes[0].nodeType, TEXT_NODE);
});

test('이미_감싼_span_안의_텍스트는_다시_감싸지_않는다', () => {
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
