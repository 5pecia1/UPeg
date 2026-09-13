'use strict';

// upeg in-page selector adapter — the extension-side counterpart of the
// Desktop Controlled Embed runner.
//
// Desktop drives a webview it owns: `upeg_runtime::selector_pipeline`
// compiles a tool's `SelectorBinding` rows into JS strings and `evaluate`s
// them inside that webview. The extension has no webview — it has the page
// the user is already on. So the same binding rows are compiled here into a
// plan and applied directly to that page's DOM by the content script.
//
// The two sides must agree on semantics, so this file mirrors
// `upeg-runtime/src/selector_pipeline.rs` deliberately:
//   * `input`   — write `.value`, then fire `input` and `change` (bubbling)
//   * `trigger` — `click()`, or focus + a bubbling/cancelable Enter key
//                 triple; the FIRST trigger row wins, later ones are ignored
//   * `output`  — read `.value` when the element has a non-empty one, else
//                 `.textContent`; a selector that matches nothing reads `''`
//   * an `input` row whose `field` is absent from the args is silently
//     dropped — the page just keeps whatever was in that field
//
// `planSelectorApplication` is pure (bindings + args in, a plan out) and
// `applySelectorPlan` touches the DOM only through `querySelector` and the
// injected event factory, so both are testable without a browser.

const UpegSelectorAdapter = (() => {
  // Mirrors `upeg_core::BindingRole::label`.
  const BINDING_ROLE = Object.freeze({
    INPUT: 'input',
    TRIGGER: 'trigger',
    OUTPUT: 'output',
  });
  // Mirrors `upeg_core::ControlledEmbedTriggerAction::label`.
  const TRIGGER_ACTION = Object.freeze({
    CLICK: 'click',
    ENTER: 'enter',
  });

  const ENTER_KEY = 'Enter';
  const INPUT_WRITE_EVENTS = Object.freeze(['input', 'change']);
  const ENTER_KEY_EVENTS = Object.freeze(['keydown', 'keypress', 'keyup']);
  const EMPTY_OUTPUT_TEXT = '';

  const BINDING_FIELD = Object.freeze({
    ROLE: 'role',
    FIELD: 'field',
    SELECTOR: 'selector',
    ACTION: 'action',
  });

  const DOM_EVENT_FACTORY = Object.freeze({
    bubbling: (type) => new Event(type, { bubbles: true }),
    enterKey: (type) =>
      new KeyboardEvent(type, {
        key: ENTER_KEY,
        code: ENTER_KEY,
        bubbles: true,
        cancelable: true,
      }),
  });

  function bindingSelector(binding) {
    const selector = binding && binding[BINDING_FIELD.SELECTOR];
    return typeof selector === 'string' && selector.length > 0 ? selector : null;
  }

  function bindingTriggerAction(binding) {
    return binding[BINDING_FIELD.ACTION] === TRIGGER_ACTION.ENTER
      ? TRIGGER_ACTION.ENTER
      : TRIGGER_ACTION.CLICK;
  }

  /// Compile `bindings` (the canonical `selectorBindings` rows the HTTP
  /// surface serves, `{role, field, selector, action}`) plus an args object
  /// into a plan. Pure: no DOM, no chrome.*, no I/O.
  function planSelectorApplication(bindings, args) {
    const rows = Array.isArray(bindings) ? bindings : [];
    const values = args && typeof args === 'object' ? args : {};
    const writes = [];
    const reads = [];
    let trigger = null;

    for (const binding of rows) {
      const selector = bindingSelector(binding);
      if (selector === null) continue;
      const field = binding[BINDING_FIELD.FIELD];
      switch (binding[BINDING_FIELD.ROLE]) {
        case BINDING_ROLE.INPUT: {
          if (!Object.prototype.hasOwnProperty.call(values, field)) break;
          const value = values[field];
          if (value === null || value === undefined) break;
          writes.push(Object.freeze({ selector, field, value: String(value) }));
          break;
        }
        case BINDING_ROLE.TRIGGER: {
          // First trigger wins — one button, one action.
          if (trigger === null) {
            trigger = Object.freeze({ selector, action: bindingTriggerAction(binding) });
          }
          break;
        }
        case BINDING_ROLE.OUTPUT: {
          reads.push(Object.freeze({ selector, field }));
          break;
        }
        default:
          break;
      }
    }

    return Object.freeze({
      writes: Object.freeze(writes),
      trigger,
      reads: Object.freeze(reads),
    });
  }

  /// `true` when the plan would touch the page at all.
  function isActionable(plan) {
    return plan.writes.length > 0 || plan.trigger !== null || plan.reads.length > 0;
  }

  function readElementText(element) {
    if (element === null || element === undefined) return EMPTY_OUTPUT_TEXT;
    const value = element.value;
    if (value !== undefined && value !== null && value !== EMPTY_OUTPUT_TEXT) {
      return String(value);
    }
    const text = element.textContent;
    return text === undefined || text === null ? EMPTY_OUTPUT_TEXT : String(text);
  }

  /// Apply `plan` to `root` (anything exposing `querySelector`) and report
  /// what happened. `eventFactory` is injected so tests can drive a fake
  /// element tree without a DOM `Event` constructor.
  function applySelectorPlan(plan, root, eventFactory = DOM_EVENT_FACTORY) {
    const written = [];
    const missing = [];
    const outputs = {};
    let triggered = false;

    for (const write of plan.writes) {
      const element = root.querySelector(write.selector);
      if (!element) {
        missing.push(write.selector);
        continue;
      }
      element.value = write.value;
      for (const type of INPUT_WRITE_EVENTS) {
        element.dispatchEvent(eventFactory.bubbling(type));
      }
      written.push(write.field);
    }

    if (plan.trigger !== null) {
      const element = root.querySelector(plan.trigger.selector);
      if (!element) {
        missing.push(plan.trigger.selector);
      } else if (plan.trigger.action === TRIGGER_ACTION.ENTER) {
        if (typeof element.focus === 'function') element.focus();
        for (const type of ENTER_KEY_EVENTS) {
          element.dispatchEvent(eventFactory.enterKey(type));
        }
        triggered = true;
      } else {
        element.click();
        triggered = true;
      }
    }

    for (const read of plan.reads) {
      const element = root.querySelector(read.selector);
      if (!element) missing.push(read.selector);
      outputs[read.field] = readElementText(element);
    }

    return { written, triggered, outputs, missing };
  }

  return Object.freeze({
    BINDING_FIELD,
    BINDING_ROLE,
    DOM_EVENT_FACTORY,
    ENTER_KEY,
    ENTER_KEY_EVENTS,
    INPUT_WRITE_EVENTS,
    TRIGGER_ACTION,
    applySelectorPlan,
    isActionable,
    planSelectorApplication,
  });
})();

if (typeof module === 'object' && module.exports) {
  module.exports = UpegSelectorAdapter;
}
