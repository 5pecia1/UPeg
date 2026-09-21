//! Pure-functional Controlled Embed pipeline — generates JavaScript
//! snippets from a `&[SelectorBinding]` + `(field, value)` pair list.
//!
//! Both surfaces (Flutter live `WebViewController` and Rust headless
//! `ControlledEmbedBackend`) share this generator so the DOM-driving
//! behavior is identical across CLI/TUI/MCP/HTTP/Desktop/PWA.
//!
//! ## SoC
//! - **No IO**: this module never touches a browser, a process, or the
//!   filesystem. It only produces strings.
//! - **No platform assumptions**: emitted JS is valid in any modern
//!   browser DOM context (WKWebView, WebView2, WebKitGTK, Chromium).
//! - **Idempotent**: re-running the same scripts on the same page
//!   gives the same result modulo what the page itself does in
//!   response to the simulated events.
//!
//! ## Three roles
//! - [`BindingRole::Input`] → [`ExecutionPlan::write_script`]: assigns
//!   `el.value = <stringified_value>` and dispatches an `input` event
//!   so React/Vue/vanilla listeners see the change.
//! - [`BindingRole::Trigger`] → [`ExecutionPlan::trigger_script`]:
//!   runs the first Trigger element's configured trigger action. Click
//!   calls `.click()`; Enter focuses the element and dispatches Enter
//!   keyboard events. There is at most one Trigger per pipeline by
//!   design (one element == one action).
//! - [`BindingRole::Output`] → [`ExecutionPlan::read_script`]: an IIFE
//!   that reads `.value` (form fields) or `.textContent` (anything
//!   else) and returns a `JSON.stringify(...)`-serialised object. The
//!   caller `JSON.parse`s it back into a `HashMap<String, String>`.
//!
//! All user-controlled strings cross the JS boundary via
//! `serde_json::to_string` (functionally `JSON.stringify`), so embed
//! pages never see a value that could break out of the assignment and
//! execute arbitrary JS.

use upeg_core::{BindingRole, BindingWaitCondition, ControlledEmbedTriggerAction, SelectorBinding};

#[derive(Debug)]
struct WaitTarget<'a> {
    selector: &'a str,
    condition: BindingWaitCondition,
}

/// Compiled view of a `&[SelectorBinding]` + inputs pair: bindings
/// pre-grouped by role with input values already attached. Borrowed —
/// no allocations beyond a few small `Vec`s the caller owns.
#[derive(Debug)]
pub struct ExecutionPlan<'a> {
    writes: Vec<(&'a SelectorBinding, &'a str)>,
    trigger: Option<&'a SelectorBinding>,
    reads: Vec<&'a SelectorBinding>,
    waits: Vec<WaitTarget<'a>>,
}

impl<'a> ExecutionPlan<'a> {
    /// Build a plan from raw bindings + inputs. Inputs missing a
    /// matching `field` are silently dropped (the page just sees an
    /// untouched DOM field); inputs without a matching binding are
    /// likewise ignored.
    pub fn build(bindings: &'a [SelectorBinding], inputs: &'a [(&'a str, &'a str)]) -> Self {
        let mut writes = Vec::new();
        let mut trigger = None;
        let mut reads = Vec::new();
        let mut waits = Vec::new();
        for b in bindings {
            if let Some(wait) = &b.wait {
                waits.push(WaitTarget {
                    selector: wait.for_selector.as_deref().unwrap_or(b.selector.as_str()),
                    condition: wait.condition,
                });
            }
            match b.role {
                BindingRole::Input => {
                    if let Some(&(_, value)) =
                        inputs.iter().find(|(field, _)| *field == b.field.as_str())
                    {
                        writes.push((b, value));
                    }
                }
                BindingRole::Trigger => {
                    // First Trigger wins; subsequent ones are ignored.
                    // Manifest validator allows multiple but only one
                    // makes sense (one button = one action).
                    if trigger.is_none() {
                        trigger = Some(b);
                    }
                }
                BindingRole::Output => reads.push(b),
            }
        }
        Self {
            writes,
            trigger,
            reads,
            waits,
        }
    }

    /// JS that writes every Input binding's matched value into the
    /// DOM. Empty string when there are no writes (callers can skip
    /// `evaluate("")` no-ops if they want).
    pub fn write_script(&self) -> String {
        if self.writes.is_empty() {
            return String::new();
        }
        let mut out = String::from("(function(){");
        for (b, value) in &self.writes {
            let sel = json_string(&b.selector);
            let val = json_string(value);
            out.push_str(&format!(
                "var el=document.querySelector({sel}); \
                 if(el){{el.value={val};\
                 el.dispatchEvent(new Event('input',{{bubbles:true}}));\
                 el.dispatchEvent(new Event('change',{{bubbles:true}}));}}",
            ));
        }
        out.push_str("})();");
        out
    }

    /// JS that fires the Trigger's configured action. Empty string
    /// when no Trigger is declared (rejected by manifest validator for
    /// ControlledEmbed, but allowed in this pure module for tests).
    pub fn trigger_script(&self) -> String {
        let Some(b) = self.trigger else {
            return String::new();
        };
        let sel = json_string(&b.selector);
        match b.trigger_action {
            ControlledEmbedTriggerAction::Click => {
                format!(
                    "(function(){{var el=document.querySelector({sel}); if(el){{el.click();}}}})();"
                )
            }
            ControlledEmbedTriggerAction::Enter => format!(
                "(function(){{var el=document.querySelector({sel}); if(el){{if(el.focus){{el.focus();}}['keydown','keypress','keyup'].forEach(function(t){{el.dispatchEvent(new KeyboardEvent(t,{{key:'Enter',code:'Enter',bubbles:true,cancelable:true}}));}});}}}})();"
            ),
        }
    }

    /// JS IIFE that returns `JSON.stringify({field1: value1, ...})`
    /// — caller parses the resulting string into a map. Returns
    /// `"{}"` when there are no Output bindings.
    pub fn read_script(&self) -> String {
        if self.reads.is_empty() {
            return String::from("\"{}\"");
        }
        let mut out = String::from("(function(){var o={};");
        for b in &self.reads {
            let sel = json_string(&b.selector);
            let field = json_string(&b.field);
            // Form fields expose `.value`; everything else uses
            // `.textContent`. Pages that need richer extraction (innerHTML,
            // attributes) can be supported by extending BindingRole
            // later — keep the pipe simple today.
            out.push_str(&format!(
                "var el=document.querySelector({sel}); \
                 if(el){{o[{field}]=('value' in el && el.value !== undefined && el.value !== '')\
                 ? String(el.value) : String(el.textContent||'');}}\
                 else{{o[{field}]='';}}",
            ));
        }
        out.push_str("return JSON.stringify(o);})();");
        out
    }

    /// JS IIFE that returns `true` when every declared binding wait is ready.
    pub fn wait_script(&self) -> String {
        if self.waits.is_empty() {
            return String::new();
        }

        let mut out = String::from(
            "(function(){function upegWaitReady(for_selector,condition){var elements=document.querySelectorAll(for_selector);if(condition==='exists'){return document.querySelectorAll(for_selector).length>0;}if(condition==='visible'){return Array.prototype.some.call(elements,function(el){var rects=el.getClientRects();var hasPositiveRect=rects.length>0&&Array.prototype.some.call(rects,function(rect){return rect.width>0&&rect.height>0;});if(!hasPositiveRect){return false;}var style=window.getComputedStyle(el);return style.display!=='none'&&style.visibility!=='hidden'&&style.opacity!=='0';});}return false;}return ",
        );
        for (index, wait) in self.waits.iter().enumerate() {
            if index > 0 {
                out.push_str("&&");
            }
            let selector = json_string(wait.selector);
            let condition = json_string(wait.condition.label());
            out.push_str(&format!("upegWaitReady({selector},{condition})"));
        }
        out.push_str(";})();");
        out
    }

    /// `true` when there is something to do — either a write, a
    /// trigger, or a read. Useful for callers that want to skip
    /// `evaluate(...)` round-trips when the plan is empty.
    pub fn is_actionable(&self) -> bool {
        !self.writes.is_empty() || self.trigger.is_some() || !self.reads.is_empty()
    }
}

/// `serde_json::to_string` wrapper that always returns a JS-safe
/// string literal. `serde_json` cannot fail on `&str`, so the
/// `unwrap_or_else` fallback is dead code; we centralize the
/// conversion here for clarity and to silence the workspace's
/// `clippy::expect_used` lint without having to suppress it
/// per-call.
fn json_string(s: &str) -> String {
    // `serde_json::to_string(&str)` is infallible — `Serialize` for
    // `&str` never returns an error. The fallback is defense in
    // depth so a future code change that swaps the input type still
    // compiles and behaves correctly.
    serde_json::to_string(s).unwrap_or_else(|_| format!("\"{}\"", s.replace('"', "\\\"")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use upeg_core::{BindingWait, BindingWaitCondition, ControlledEmbedTriggerAction};

    fn binding(role: BindingRole, field: &str, selector: &str) -> SelectorBinding {
        binding_with_action(role, field, selector, ControlledEmbedTriggerAction::Click)
    }

    fn binding_with_action(
        role: BindingRole,
        field: &str,
        selector: &str,
        trigger_action: ControlledEmbedTriggerAction,
    ) -> SelectorBinding {
        SelectorBinding {
            role,
            field: field.to_string(),
            selector: selector.to_string(),
            trigger_action,
            wait: None,
        }
    }

    fn binding_with_wait(
        role: BindingRole,
        field: &str,
        selector: &str,
        wait: BindingWait,
    ) -> SelectorBinding {
        SelectorBinding {
            wait: Some(wait),
            ..binding(role, field, selector)
        }
    }

    fn wait(for_selector: Option<&str>, condition: BindingWaitCondition) -> BindingWait {
        BindingWait {
            for_selector: for_selector.map(str::to_string),
            condition,
            ..BindingWait::default()
        }
    }

    // ─── build() ─────────────────────────────────────────────

    #[test]
    fn build_returns_empty_plan_for_empty_bindings() {
        let plan = ExecutionPlan::build(&[], &[]);
        assert!(!plan.is_actionable());
        assert_eq!(plan.write_script(), "");
        assert_eq!(plan.trigger_script(), "");
        assert_eq!(plan.read_script(), "\"{}\"");
    }

    #[test]
    fn build_matches_input_bindings_with_input_values() {
        let bindings = vec![binding(BindingRole::Input, "q", "#q")];
        let inputs: &[(&str, &str)] = &[("q", "hello")];
        let plan = ExecutionPlan::build(&bindings, inputs);
        let script = plan.write_script();
        assert!(script.contains("\"#q\""), "selector escaped: {script}");
        assert!(script.contains("\"hello\""), "value escaped: {script}");
        assert!(script.contains("el.value="));
    }

    #[test]
    fn build_drops_unmatched_inputs() {
        let bindings = vec![binding(BindingRole::Input, "missing", "#x")];
        let plan = ExecutionPlan::build(&bindings, &[("other", "v")]);
        // No write generated for the unmatched binding.
        assert_eq!(plan.write_script(), "");
    }

    #[test]
    fn build_uses_only_first_trigger_binding() {
        let bindings = vec![
            binding(BindingRole::Trigger, "", "#first"),
            binding(BindingRole::Trigger, "", "#second"),
        ];
        let plan = ExecutionPlan::build(&bindings, &[]);
        let script = plan.trigger_script();
        assert!(script.contains("\"#first\""));
        assert!(!script.contains("\"#second\""));
    }

    #[test]
    fn build_preserves_all_outputs() {
        let bindings = vec![
            binding(BindingRole::Output, "a", "#a"),
            binding(BindingRole::Output, "b", "#b"),
        ];
        let plan = ExecutionPlan::build(&bindings, &[]);
        let script = plan.read_script();
        assert!(script.contains("\"#a\""));
        assert!(script.contains("\"#b\""));
        assert!(script.contains("\"a\""));
        assert!(script.contains("\"b\""));
        assert!(script.contains("JSON.stringify"));
    }

    // ─── Escaping ───────────────────────────────────────────

    #[test]
    fn write_script_escapes_quotes() {
        let bindings = vec![binding(BindingRole::Input, "q", "#q")];
        let plan = ExecutionPlan::build(&bindings, &[("q", "He said \"hi\"")]);
        let script = plan.write_script();
        // serde_json escapes inner quotes to \"
        assert!(script.contains("\\\""), "must escape quotes: {script}");
    }

    #[test]
    fn write_script_escapes_backslashes() {
        let bindings = vec![binding(BindingRole::Input, "p", "#p")];
        let plan = ExecutionPlan::build(&bindings, &[("p", "C:\\path")]);
        let script = plan.write_script();
        assert!(script.contains("\\\\"), "must escape backslash: {script}");
    }

    #[test]
    fn write_script_escapes_newlines() {
        let bindings = vec![binding(BindingRole::Input, "t", "#t")];
        let plan = ExecutionPlan::build(&bindings, &[("t", "line1\nline2")]);
        let script = plan.write_script();
        assert!(script.contains("\\n"), "must escape newline: {script}");
    }

    #[test]
    fn read_script_returns_empty_object_string_for_empty_bindings() {
        let plan = ExecutionPlan::build(&[], &[]);
        // The read returns `"{}"` so caller's JSON.parse gets an
        // empty object without a separate special case.
        assert_eq!(plan.read_script(), "\"{}\"");
    }

    #[test]
    fn selector_pipeline_wait_exists_condition_succeeds_when_element_present() {
        let bindings = vec![binding_with_wait(
            BindingRole::Input,
            "q",
            "#q",
            wait(Some("#ready"), BindingWaitCondition::Exists),
        )];
        let plan = ExecutionPlan::build(&bindings, &[("q", "value")]);
        let script = plan.wait_script();

        assert!(
            script.contains("function upegWaitReady(for_selector,condition)"),
            "wait helper parameters not found: {script}"
        );
        assert!(
            script.contains("document.querySelectorAll(for_selector).length>0"),
            "exists condition not found: {script}"
        );
        assert!(
            script.contains("upegWaitReady(\"#ready\",\"exists\")"),
            "exists invocation not found: {script}"
        );
    }

    #[test]
    fn selector_pipeline_wait_visible_condition_succeeds_on_visible_element() {
        let bindings = vec![binding_with_wait(
            BindingRole::Trigger,
            "",
            "#submit",
            wait(Some("#result"), BindingWaitCondition::Visible),
        )];
        let plan = ExecutionPlan::build(&bindings, &[]);
        let script = plan.wait_script();

        assert!(
            script.contains("condition==='visible'"),
            "visible branch not found: {script}"
        );
        assert!(
            script.contains("var rects=el.getClientRects()"),
            "client rect lookup not found: {script}"
        );
        assert!(
            script.contains("rects.length>0"),
            "non-empty client rect check not found: {script}"
        );
        assert!(
            script.contains("rect.width>0&&rect.height>0"),
            "positive rect size check not found: {script}"
        );
        assert!(
            script.contains("var style=window.getComputedStyle(el)"),
            "computed style lookup not found: {script}"
        );
        assert!(
            script.contains("style.display!=='none'"),
            "display check not found: {script}"
        );
        assert!(
            script.contains("style.visibility!=='hidden'"),
            "visibility check not found: {script}"
        );
        assert!(
            script.contains("style.opacity!=='0'"),
            "opacity check not found: {script}"
        );
        assert!(
            script.contains("upegWaitReady(\"#result\",\"visible\")"),
            "visible invocation not found: {script}"
        );
    }

    #[test]
    fn selector_pipeline_wait_visible_succeeds_when_any_match_is_visible() {
        let bindings = vec![binding_with_wait(
            BindingRole::Output,
            "result",
            ".item",
            wait(Some(".item"), BindingWaitCondition::Visible),
        )];
        let plan = ExecutionPlan::build(&bindings, &[]);
        let script = plan.wait_script();

        assert!(
            script.contains("Array.prototype.some.call(elements"),
            "matched elements must use any-match semantics: {script}"
        );
        assert!(
            script.contains("Array.prototype.some.call(rects"),
            "client rects must accept any positive rect: {script}"
        );
        assert!(
            !script.contains("Array.prototype.every.call(elements"),
            "matched elements must not require every element: {script}"
        );
        assert!(
            !script.contains("elements[0]"),
            "matched elements must not check only the first element: {script}"
        );
    }

    #[test]
    fn selector_pipeline_wait_uses_binding_selector_when_for_selector_omitted() {
        let bindings = vec![binding_with_wait(
            BindingRole::Input,
            "q",
            "#binding-selector",
            wait(None, BindingWaitCondition::Exists),
        )];
        let plan = ExecutionPlan::build(&bindings, &[("q", "value")]);
        let script = plan.wait_script();

        assert!(
            script.contains("upegWaitReady(\"#binding-selector\",\"exists\")"),
            "binding selector default not found: {script}"
        );
    }

    #[test]
    fn selector_pipeline_wait_selector_escapes_quotes_and_backslashes() {
        let selector = "input[name=\"q\\\\path\"]";
        let bindings = vec![binding_with_wait(
            BindingRole::Input,
            "q",
            selector,
            wait(None, BindingWaitCondition::Exists),
        )];
        let plan = ExecutionPlan::build(&bindings, &[("q", "value")]);
        let script = plan.wait_script();
        let escaped_selector = json_string(selector);

        assert!(
            escaped_selector.contains("\\\""),
            "quote must be json escaped: {escaped_selector}"
        );
        assert!(
            escaped_selector.contains("\\\\"),
            "backslash must be json escaped: {escaped_selector}"
        );
        assert!(
            script.contains(&format!("upegWaitReady({escaped_selector},\"exists\")")),
            "escaped selector invocation not found: {script}"
        );
    }

    #[test]
    fn selector_pipeline_without_wait_leaves_existing_scripts_untouched() {
        let bindings = vec![
            binding(BindingRole::Input, "q", "#q"),
            binding(BindingRole::Trigger, "", "#submit"),
            binding(BindingRole::Output, "result", "#result"),
        ];
        let plan = ExecutionPlan::build(&bindings, &[("q", "value")]);

        assert_eq!(plan.wait_script(), "");
        assert!(!plan.write_script().contains("upegWaitReady"));
        assert!(!plan.trigger_script().contains("upegWaitReady"));
        assert!(!plan.read_script().contains("upegWaitReady"));
    }

    // ─── Integration scenario ────────────────────────────────

    #[test]
    fn full_pipeline_produces_write_trigger_read() {
        let bindings = vec![
            binding(BindingRole::Input, "q", "#q"),
            binding(BindingRole::Trigger, "", "button[type='submit']"),
            binding(BindingRole::Output, "r", "#result"),
        ];
        let inputs: &[(&str, &str)] = &[("q", "test")];
        let plan = ExecutionPlan::build(&bindings, inputs);
        assert!(plan.is_actionable());
        assert!(!plan.write_script().is_empty());
        assert!(plan.trigger_script().contains("button[type="));
        assert!(plan.trigger_script().contains("submit"));
        assert!(plan.read_script().contains("#result"));
    }

    #[test]
    fn is_actionable_is_true_with_write_only() {
        let bindings = vec![binding(BindingRole::Input, "q", "#q")];
        let inputs: &[(&str, &str)] = &[("q", "v")];
        let plan = ExecutionPlan::build(&bindings, inputs);
        assert!(plan.is_actionable());
    }

    // ─── missing-selector and guard semantics ─────────────────────

    #[test]
    fn trigger_script_generates_click_guard_even_when_selector_missing() {
        let bindings = vec![binding(BindingRole::Trigger, "", "#missing-button")];
        let plan = ExecutionPlan::build(&bindings, &[]);
        let script = plan.trigger_script();
        assert!(
            script.contains("\"#missing-button\""),
            "selector not found: {script}"
        );
        assert!(script.contains("if(el)"), "guard not found: {script}");
        assert!(script.contains("el.click()"), "click not found: {script}");
    }

    #[test]
    fn trigger_click_preserves_existing_click_js_shape() {
        let bindings = vec![binding(BindingRole::Trigger, "", "button[type='submit']")];
        let plan = ExecutionPlan::build(&bindings, &[]);
        let script = plan.trigger_script();
        assert!(
            script.contains("document.querySelector"),
            "querySelector not found: {script}"
        );
        assert!(script.contains("if(el)"), "guard not found: {script}");
        assert!(script.contains("el.click()"), "click not found: {script}");
        assert!(
            !script.contains("KeyboardEvent"),
            "KeyboardEvent must not be in click script: {script}"
        );
    }

    #[test]
    fn trigger_enter_generates_focus_and_enter_keyboard_events() {
        let bindings = vec![binding_with_action(
            BindingRole::Trigger,
            "",
            "input[name=\"q\"]",
            ControlledEmbedTriggerAction::Enter,
        )];
        let plan = ExecutionPlan::build(&bindings, &[]);
        let script = plan.trigger_script();
        assert!(
            script.contains("document.querySelector"),
            "querySelector not found: {script}"
        );
        assert!(script.contains("if(el)"), "guard not found: {script}");
        assert!(script.contains("el.focus()"), "focus not found: {script}");
        assert!(
            script.contains("KeyboardEvent"),
            "KeyboardEvent not found: {script}"
        );
        assert!(script.contains("'keydown'"), "keydown not found: {script}");
        assert!(
            script.contains("'keypress'"),
            "keypress not found: {script}"
        );
        assert!(script.contains("'keyup'"), "keyup not found: {script}");
        assert!(script.contains("key:'Enter'"), "key not found: {script}");
        assert!(script.contains("code:'Enter'"), "code not found: {script}");
        assert!(
            script.contains("bubbles:true"),
            "bubbles not found: {script}"
        );
        assert!(
            script.contains("cancelable:true"),
            "cancelable not found: {script}"
        );
        assert!(
            script.contains("input[name=\\\"q\\\"]"),
            "selector quotes must be json escaped: {script}"
        );
        assert!(
            !script.contains("el.click()"),
            "click must not be in enter script: {script}"
        );
    }

    #[test]
    fn read_script_records_empty_string_for_missing_output_selector() {
        let bindings = vec![binding(BindingRole::Output, "result", "#missing")];
        let plan = ExecutionPlan::build(&bindings, &[]);
        let script = plan.read_script();
        assert!(
            script.contains("\"#missing\""),
            "selector not found: {script}"
        );
        assert!(
            script.contains("o[\"result\"]"),
            "field not found: {script}"
        );
        assert!(
            script.contains("else{o[\"result\"]=''"),
            "empty fallback not found: {script}"
        );
    }

    #[test]
    fn write_script_dispatches_both_input_and_change_events() {
        let bindings = vec![binding(BindingRole::Input, "q", "#q")];
        let inputs: &[(&str, &str)] = &[("q", "test-value")];
        let plan = ExecutionPlan::build(&bindings, inputs);
        let script = plan.write_script();
        assert!(
            script.contains("'input'"),
            "input event not found: {script}"
        );
        assert!(
            script.contains("'change'"),
            "change event not found: {script}"
        );
    }

    #[test]
    #[allow(
        non_snake_case,
        reason = "test name intentionally mentions DOM textContent casing"
    )]
    fn read_script_falls_back_to_textContent_when_value_empty() {
        let bindings = vec![binding(BindingRole::Output, "out", "#out")];
        let plan = ExecutionPlan::build(&bindings, &[]);
        let script = plan.read_script();
        assert!(
            script.contains("el.value !== ''"),
            "value check not found: {script}"
        );
        assert!(
            script.contains("textContent"),
            "textContent fallback not found: {script}"
        );
    }
}
