//! Load-time validation of `args_template` placeholders.
//!
//! `args_template = ["--out", "{outpath}"]` on a tool whose input is
//! named `out_path` used to be silent: the placeholder matched nothing,
//! the token rendered empty, and the argument simply disappeared from
//! the command line. Nothing reported the typo — the tool just ran with
//! the wrong argument list. Every `{key}` must therefore name something
//! the tool can actually be handed, checked once at load time with the
//! same parser the dispatcher renders with
//! (`crate::dispatcher::external::template`), so the check cannot drift
//! from the substitution it guards.
//!
//! Chain step args are deliberately out of scope: `steps[].args` uses
//! the unrelated `{{ expression }}` engine in `crate::dispatcher::chain`,
//! which resolves step outputs, credentials, and context paths rather
//! than declared inputs.

use std::collections::BTreeSet;

use crate::LoadError;
use crate::ToolToml;
use crate::dispatcher::external::template::ArgTemplate;

/// The one argument key a tool may substitute without declaring it.
///
/// The `Chain` invoker hands every step `{"input": <upstream output>}`
/// (see `crate::dispatcher::chain`), so an External tool written to be a
/// chain node legitimately reads `{input}` even with no
/// `[[tools.inputs]]` of its own. Every other key has to be declared.
const CHAIN_STEP_INPUT_KEY: &str = "input";

pub(super) fn validate_arg_templates(parsed: &ToolToml) -> Result<(), LoadError> {
    let Some(tokens) = parsed.args_template.as_deref() else {
        return Ok(());
    };
    let declared: BTreeSet<&str> = parsed
        .inputs
        .iter()
        .map(|field| field.name.trim())
        .chain(std::iter::once(CHAIN_STEP_INPUT_KEY))
        .collect();
    for (position, token) in tokens.iter().enumerate() {
        let template = ArgTemplate::parse(token);
        for key in template.placeholders() {
            if key.is_empty() {
                return Err(LoadError::EmptyArgTemplatePlaceholder { position });
            }
            if !declared.contains(key) {
                return Err(LoadError::UnknownArgTemplateInput {
                    position,
                    key: key.to_string(),
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn validate(toml: &str) -> Result<(), LoadError> {
        let parsed: ToolToml = toml::from_str(toml).expect("test fixture should parse");
        validate_arg_templates(&parsed)
    }

    #[test]
    fn placeholder_for_declared_input_passes() {
        let result = validate(
            r#"
id = "demo.log"
toolkit = "demo"
pegboard_units = "U1"
invoker = "External"
command = "git"
args_template = ["log", "-n", "{count}", "--format={ count }"]

[[inputs]]
name = "count"
type = "integer"
"#,
        );

        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn placeholder_for_undeclared_input_fails_load() {
        let result = validate(
            r#"
id = "demo.log"
toolkit = "demo"
pegboard_units = "U1"
invoker = "External"
command = "git"
args_template = ["log", "-n", "{cuont}"]

[[inputs]]
name = "count"
type = "integer"
"#,
        );

        assert!(
            matches!(
                &result,
                Err(LoadError::UnknownArgTemplateInput { position: 2, key }) if key == "cuont"
            ),
            "{result:?}"
        );
    }

    #[test]
    fn empty_placeholder_fails_load() {
        let result = validate(
            r#"
id = "demo.log"
toolkit = "demo"
pegboard_units = "U1"
invoker = "External"
command = "git"
args_template = ["{}"]
"#,
        );

        assert!(
            matches!(
                result,
                Err(LoadError::EmptyArgTemplatePlaceholder { position: 0 })
            ),
            "{result:?}"
        );
    }

    #[test]
    fn chain_step_input_key_needs_no_declaration() {
        let result = validate(
            r#"
id = "demo.echo"
toolkit = "demo"
pegboard_units = "U1"
invoker = "External"
command = "printf"
args_template = ["{input}"]
"#,
        );

        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn literal_and_unclosed_braces_are_not_placeholders() {
        let result = validate(
            r#"
id = "demo.echo"
toolkit = "demo"
pegboard_units = "U1"
invoker = "External"
command = "printf"
args_template = ["{{nope}}", "{unclosed"]
"#,
        );

        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn without_args_template_nothing_to_check() {
        let result = validate(
            r#"
id = "demo.echo"
toolkit = "demo"
pegboard_units = "U1"
invoker = "External"
command = "printf"
"#,
        );

        assert!(result.is_ok(), "{result:?}");
    }
}
