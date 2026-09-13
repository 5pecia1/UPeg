'use strict';

// How the popup routes a pinned tool when the user activates it.
//
// Three destinations, chosen per tool — this is the popup's whole dispatch
// policy and it is a pure function of the tool's own metadata plus one bit
// of context ("is the current tab enabled for in-page work?"):
//
//   * `IN_PAGE`   — a `ControlledEmbed` pin, on an enabled tab. Its selector
//                   bindings are applied to the page the user is already
//                   looking at (content.js + selector_adapter.js). This is
//                   the extension's own answer to Controlled Embed: Desktop
//                   drives a webview it owns, the extension drives the tab.
//   * `DISPATCH`  — `POST /v1/tools/{id}` on the local host, result inline.
//   * `DEEP_LINK` — `upeg://open?...`, for everything the popup cannot run:
//                   `static` (Passive Embed — the webview IS the tool) and
//                   any Controlled Embed with no page to drive.
//
// Field names come from `upeg_runtime::ToolMetaRuntimeExt::to_json_object`
// (upeg-cli surfaces/http/mod.rs calls it with `id_key = "name"`).

const UpegToolRouting = (() => {
  const TOOL_FIELD = Object.freeze({
    ID: 'name',
    LABEL: 'displayLabel',
    INVOKER: 'invoker',
    PIN: 'pin',
    INPUT_SCHEMA: 'inputSchema',
    SELECTOR_BINDINGS: 'selectorBindings',
  });

  // `upeg_core::PinKind::label()`.
  const PIN_KIND = Object.freeze({
    CONTROLLED_EMBED: 'ControlledEmbed',
  });

  // `upeg_core::Invoker::label()` values that cannot answer a
  // `POST /v1/tools/{id}`: `static` is a presentation with nothing to run,
  // `embed` needs a browser to drive.
  const NON_DISPATCHABLE_INVOKERS = new Set(['static', 'embed']);

  const ACTIVATION_ROUTE = Object.freeze({
    IN_PAGE: 'in_page',
    DISPATCH: 'dispatch',
    DEEP_LINK: 'deep_link',
  });

  function toolId(tool) {
    return tool[TOOL_FIELD.ID];
  }

  function toolLabel(tool) {
    return tool[TOOL_FIELD.LABEL] || toolId(tool);
  }

  function toolInputProperties(tool) {
    const schema = tool[TOOL_FIELD.INPUT_SCHEMA];
    const properties = schema && schema.properties;
    return properties && typeof properties === 'object' ? properties : {};
  }

  function toolRequiredFields(tool) {
    const schema = tool[TOOL_FIELD.INPUT_SCHEMA];
    return schema && Array.isArray(schema.required) ? schema.required : [];
  }

  /// The tool's `{role, field, selector, action}` rows, as served by
  /// `upeg_runtime::selector_bindings_json_for`.
  function toolSelectorBindings(tool) {
    const bindings = tool[TOOL_FIELD.SELECTOR_BINDINGS];
    return Array.isArray(bindings) ? bindings : [];
  }

  function isControlledEmbed(tool) {
    return tool[TOOL_FIELD.PIN] === PIN_KIND.CONTROLLED_EMBED;
  }

  /// Pure routing decision. `inPageAvailable` is true only when the active
  /// tab's site is enabled AND its content script answered a ping — the
  /// popup must never offer an in-page run it cannot perform.
  function activationRouteFor(tool, { inPageAvailable = false } = {}) {
    if (isControlledEmbed(tool)) {
      return inPageAvailable && toolSelectorBindings(tool).length > 0
        ? ACTIVATION_ROUTE.IN_PAGE
        : ACTIVATION_ROUTE.DEEP_LINK;
    }
    return NON_DISPATCHABLE_INVOKERS.has(tool[TOOL_FIELD.INVOKER])
      ? ACTIVATION_ROUTE.DEEP_LINK
      : ACTIVATION_ROUTE.DISPATCH;
  }

  return Object.freeze({
    ACTIVATION_ROUTE,
    NON_DISPATCHABLE_INVOKERS,
    PIN_KIND,
    TOOL_FIELD,
    activationRouteFor,
    isControlledEmbed,
    toolId,
    toolInputProperties,
    toolLabel,
    toolRequiredFields,
    toolSelectorBindings,
  });
})();

if (typeof module === 'object' && module.exports) {
  module.exports = UpegToolRouting;
}
