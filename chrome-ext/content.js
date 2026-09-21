// upeg content script — the extension-only surface
// (docs/ui-ux-surface-contract.md "Chrome extension contract").
//
// Two capabilities that exist nowhere else in upeg, because both need to be
// inside a page the user did not author:
//
//   1. **In-page detections.** Every row of the `UpegDetectors` table is
//      scanned for; matches are wrapped in an inline span whose tooltip
//      carries the answer. Rows that name a host tool resolve through it —
//      lazily, on first hover/focus — and fall back to a Desktop deep link
//      when no host answers. Rows without a host tool (see detectors.js)
//      show their offline value and stay non-actionable.
//   2. **The selector adapter.** `APPLY_SELECTOR_BINDINGS` fills this page's
//      fields from a Controlled Embed tool's bindings and optionally fires
//      its trigger — the in-page counterpart of the Desktop runner.
//
// Registered dynamically per enabled site (site_access.js), never from a
// hard-coded `content_scripts` match list. Vanilla JS, no build step: the
// decisions live in detectors.js / selector_adapter.js, this file owns the
// DOM and chrome.* glue only.

(() => {
  'use strict';

  // Lives at most once per frame.
  //
  // This file arrives by two paths: `chrome.scripting.registerContentScripts`
  // dynamic registration (from the next load) and the popup's
  // `executeScript` (the tab being looked at right now). If both paths hit
  // the same frame, or the user toggles twice, the file is re-evaluated in
  // that frame — each time adding one more `chrome.runtime.onMessage`
  // listener and `MutationObserver`, answering PING multiple times and
  // scanning every DOM change multiple times. Content scripts share an
  // isolated world per frame, so a mark on that world's global cuts
  // re-evaluation off right here.
  const CONTENT_SCRIPT_SENTINEL = 'upegContentScriptActive';
  if (window[CONTENT_SCRIPT_SENTINEL]) return;
  window[CONTENT_SCRIPT_SENTINEL] = true;

  const {
    CONTENT_SCRIPT_BOARD,
    DISPATCH_RESULT_KIND,
    RUNTIME_MESSAGE,
    buildDeepLink,
    outputText,
    primaryOutput,
  } = UpegHostApi;
  const { detect, dispatchArgsFor, hasDetection, labelKeyFor, previewFor, toolIdFor } =
    UpegDetectors;
  const { applySelectorPlan, planSelectorApplication } = UpegSelectorAdapter;

  const MARK_CLASS = 'upeg-mark';
  const STYLE_ELEMENT_ID = 'upeg-content-style';
  const TIP_ATTR = 'data-upeg-tip';
  const LINK_ATTR = 'data-upeg-link';
  const DETECTOR_ATTR = 'data-upeg-detector';
  const PROCESSED = Symbol.for('upeg-processed');
  const RESOLVED = Symbol.for('upeg-resolved');
  const ACTIVATION_KEYS = new Set(['Enter', ' ']);
  const CACHE_KEY_SEPARATOR = ' ';
  const ELEMENT_NODE = 1;
  const TEXT_NODE = 3;

  // Never rewrite text inside code editors or existing form controls.
  const SKIP_TAGS = new Set([
    'SCRIPT', 'STYLE', 'NOSCRIPT', 'TEXTAREA', 'INPUT',
    'CODE', 'PRE', 'TEMPLATE',
  ]);

  const tagOf = (node) =>
    node && node.nodeType === ELEMENT_NODE ? node.tagName : '';

  // The one answer to "may this text node be rewritten?", shared by the
  // initial tree walk and the MutationObserver. Two copies is how the
  // observer's text-node path came to rewrite inside <textarea>/<code>:
  // the walk filtered on SKIP_TAGS and the observer filtered on nothing.
  function isSkippedContext(node) {
    let parent = node ? node.parentNode : null;
    while (parent) {
      if (SKIP_TAGS.has(tagOf(parent))) return true;
      if (parent.classList && parent.classList.contains(MARK_CLASS)) return true;
      parent = parent.parentNode;
    }
    return false;
  }

  const i18nMessage = (key, substitutions) => chrome.i18n.getMessage(key, substitutions);

  function ensureStyles() {
    if (document.getElementById(STYLE_ELEMENT_ID)) return;
    const style = document.createElement('style');
    style.id = STYLE_ELEMENT_ID;
    style.textContent = `
      .${MARK_CLASS} {
        border-bottom: 1px dotted #888;
        position: relative;
      }
      .${MARK_CLASS}[role="button"] {
        cursor: pointer;
      }
      .${MARK_CLASS}:hover::after,
      .${MARK_CLASS}:focus::after {
        content: attr(${TIP_ATTR});
        position: absolute;
        left: 0;
        top: 100%;
        margin-top: 4px;
        background: #14171c;
        color: #e8ebef;
        border: 1px solid #23282f;
        border-radius: 4px;
        padding: 4px 8px;
        font: 500 11px/1.3 ui-monospace, monospace;
        white-space: nowrap;
        z-index: 2147483647;
        pointer-events: none;
      }
    `;
    (document.head || document.documentElement).appendChild(style);
  }

  // === Tooltip text (one whole message per state — never concatenation) ===

  function tipFor(detection, { value, linked }) {
    const label = i18nMessage(labelKeyFor(detection));
    if (value === null) {
      return linked
        ? i18nMessage('detectorTipLink', [label])
        : i18nMessage('detectorTipPending', [label]);
    }
    return linked
      ? i18nMessage('detectorTipValueLink', [label, value])
      : i18nMessage('detectorTipValue', [label, value]);
  }

  function openDesktop(span) {
    window.location.href = span.getAttribute(LINK_ATTR);
  }

  // === Host resolution (through the service worker — see background.js) ===

  const resolutionCache = new Map();

  function resolveThroughHost(toolId, args) {
    const key = `${toolId}${CACHE_KEY_SEPARATOR}${JSON.stringify(args)}`;
    if (!resolutionCache.has(key)) {
      resolutionCache.set(
        key,
        chrome.runtime
          .sendMessage({ type: RUNTIME_MESSAGE.DISPATCH_TOOL, toolId, args })
          .then((outcome) => {
            if (!outcome || outcome.kind !== DISPATCH_RESULT_KIND.SUCCESS) return null;
            const primary = primaryOutput(outcome.result);
            return primary === null ? null : outputText(primary.value);
          })
          .catch(() => null),
      );
    }
    return resolutionCache.get(key);
  }

  async function resolveSpan(span, detection) {
    if (span[RESOLVED]) return;
    span[RESOLVED] = true;
    const toolId = toolIdFor(detection);
    const args = dispatchArgsFor(detection);
    if (toolId === null || args === null) return;
    const hosted = await resolveThroughHost(toolId, args);
    // No host (or a host that refused): the offline preview is the best
    // this page can say, and the deep link stays the way out.
    const value = hosted === null ? previewFor(detection) : hosted;
    span.setAttribute(TIP_ATTR, tipFor(detection, { value, linked: true }));
  }

  // === Wrapping ===

  function buildSpan(detection) {
    const span = document.createElement('span');
    span.className = MARK_CLASS;
    span.textContent = detection.raw;
    span.setAttribute(DETECTOR_ATTR, detection.detectorId);
    span[PROCESSED] = true;

    const toolId = toolIdFor(detection);
    const preview = previewFor(detection);
    if (toolId === null) {
      // No host tool can resolve this row, so there is nothing to click —
      // an annotation, not a dead affordance.
      span.setAttribute(TIP_ATTR, tipFor(detection, { value: preview, linked: false }));
      return span;
    }

    span.setAttribute(TIP_ATTR, tipFor(detection, { value: preview, linked: true }));
    span.setAttribute(
      LINK_ATTR,
      buildDeepLink({ board: CONTENT_SCRIPT_BOARD, toolId, input: detection.value }),
    );
    span.setAttribute('role', 'button');
    span.setAttribute('tabindex', '0');
    span.setAttribute('title', i18nMessage('openInDesktopTitle'));
    span.addEventListener('click', () => openDesktop(span));
    span.addEventListener('keydown', (evt) => {
      if (ACTIVATION_KEYS.has(evt.key)) {
        evt.preventDefault();
        openDesktop(span);
      }
    });
    // Resolve lazily: a page can carry hundreds of matches and none of them
    // is worth a host round-trip until the user looks at it.
    span.addEventListener('mouseenter', () => resolveSpan(span, detection));
    span.addEventListener('focus', () => resolveSpan(span, detection));
    return span;
  }

  function wrapTextNode(node) {
    if (node[PROCESSED]) return;
    if (isSkippedContext(node)) return;
    const text = node.nodeValue;
    const detections = detect(text);
    if (detections.length === 0) return;

    const frag = document.createDocumentFragment();
    let last = 0;
    for (const detection of detections) {
      const before = text.slice(last, detection.index);
      if (before) frag.appendChild(document.createTextNode(before));
      frag.appendChild(buildSpan(detection));
      last = detection.index + detection.length;
    }
    const tail = text.slice(last);
    if (tail) frag.appendChild(document.createTextNode(tail));
    if (node.parentNode) node.parentNode.replaceChild(frag, node);
  }

  function walk(root) {
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
      acceptNode(n) {
        if (isSkippedContext(n)) return NodeFilter.FILTER_REJECT;
        return hasDetection(n.nodeValue) ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT;
      },
    });
    const targets = [];
    let cur = walker.nextNode();
    while (cur) {
      targets.push(cur);
      cur = walker.nextNode();
    }
    targets.forEach(wrapTextNode);
  }

  // === Selector adapter (Controlled Embed, in-page) ===

  function applyBindings(message) {
    const plan = planSelectorApplication(message.bindings, message.args);
    return Object.assign({ ok: true }, applySelectorPlan(plan, document));
  }

  chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
    switch (message && message.type) {
      case RUNTIME_MESSAGE.PING:
        sendResponse({ ok: true });
        return false;
      case RUNTIME_MESSAGE.APPLY_SELECTOR_BINDINGS:
        sendResponse(applyBindings(message));
        return false;
      default:
        return false;
    }
  });

  function init() {
    ensureStyles();
    walk(document.body);

    // Re-scan on dynamic page updates (SPAs, infinite scroll).
    const obs = new MutationObserver((muts) => {
      for (const m of muts) {
        m.addedNodes.forEach((n) => {
          if (n.nodeType === ELEMENT_NODE) walk(n);
          else if (n.nodeType === TEXT_NODE) wrapTextNode(n);
        });
      }
    });
    obs.observe(document.body, { childList: true, subtree: true });
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
