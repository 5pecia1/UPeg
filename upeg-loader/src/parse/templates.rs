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

    fn 검증(toml: &str) -> Result<(), LoadError> {
        let parsed: ToolToml = toml::from_str(toml).expect("test fixture should parse");
        validate_arg_templates(&parsed)
    }

    #[test]
    fn 선언된_입력을_가리키는_자리표시자는_통과한다() {
        let result = 검증(
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
    fn 선언되지_않은_입력을_가리키면_로드에_실패한다() {
        let result = 검증(
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
    fn 빈_자리표시자는_로드에_실패한다() {
        let result = 검증(
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
    fn chain_step_입력_키는_선언하지_않아도_된다() {
        let result = 검증(
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
    fn 리터럴_중괄호와_닫히지_않은_중괄호는_자리표시자가_아니다() {
        let result = 검증(
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
    fn args_template이_없으면_검사할_것이_없다() {
        let result = 검증(
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
