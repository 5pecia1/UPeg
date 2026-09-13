//! Argument-building for the CLI dynamic dispatch route.
//!
//! `upeg {toolkit} {tool} <pos1> <pos2> ...` is the single dynamic CLI
//! entry point. This module turns the positional tokens after
//! `{toolkit} {tool}` into the `{tool_id, args}` envelope that
//! `dispatch_tool` consumes. See `docs/architecture/call-envelope.md` for
//! the positional-to-schema binding contract.

use std::io::IsTerminal;

use serde_json::{Map, Value};
use upeg_core::{
    ChoiceSpec, EXECUTION_CONTEXT_ARG, InputFieldSpec, InputKind, InputSpec, InputValue, Invoker,
};

use crate::CliError;

mod file_input;

use file_input::file_value_from_cli_path;

/// CLI-reserved input name for chain-step approval
/// (docs/architecture/chain.md, E-3/B-2). Never part of a Tool's
/// declared [`InputSpec`]; only accepted for `Invoker::Chain` tools —
/// see [`ReservedInputs`].
pub(crate) const APPROVE_RESERVED_INPUT_NAME: &str = "approve";

/// Which CLI-reserved input names (`approve`, ...) are accepted for one
/// `upeg call` target, derived from the tool's [`Invoker`]. Chain tools
/// accept `approve`; every other invoker accepts none, so
/// `-a approve=…` / a raw-JSON `"approve"` key on a non-chain tool keeps
/// producing the ordinary unknown-input error. Adding a second reserved
/// input is mechanical: a new const name, a field here, a branch in
/// [`Self::accepts`] and [`Self::names`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) struct ReservedInputs {
    chain_approval: bool,
}

impl ReservedInputs {
    pub(crate) const NONE: Self = Self {
        chain_approval: false,
    };

    pub(crate) const fn for_invoker(invoker: Invoker) -> Self {
        Self {
            chain_approval: matches!(invoker, Invoker::Chain),
        }
    }

    fn accepts(self, name: &str) -> bool {
        self.chain_approval && name == APPROVE_RESERVED_INPUT_NAME
    }

    fn names(self) -> impl Iterator<Item = &'static str> {
        self.chain_approval
            .then_some(APPROVE_RESERVED_INPUT_NAME)
            .into_iter()
    }
}

fn read_stdin() -> Result<String, CliError> {
    let mut buf = String::new();
    let mut stdin = std::io::stdin();
    std::io::Read::read_to_string(&mut stdin, &mut buf)
        .map_err(|e| CliError::tool_failed(format!("read stdin: {e}")))?;
    Ok(buf.trim_end_matches(['\r', '\n']).to_string())
}

/// Args for a Tool whose `input_spec` declares no fields: positional
/// tokens fold into the legacy `{ "input": ... }` shape.
///
/// Deliberately does NOT read stdin. The auto-stdin rule
/// (docs/architecture/call-envelope.md, positional binding rule 6) binds
/// stdin to *the first required input field*, and a Tool that declares
/// no field has none — there is nothing for stdin to become, so the
/// bytes would be read only to be thrown away. Reading them anyway hung
/// every zero-input Tool (`upeg time iso-now`, `upeg id uuid-v7`, …)
/// forever whenever the process inherited a pipe that never reaches EOF:
/// an agent harness, a supervisor, a shell whose stdin is a live socket.
/// Non-empty specs are unaffected — [`schema_bound_args_from_cli`] runs
/// the real auto-stdin path for those, which terminates because there is
/// a field waiting for the bytes.
fn dynamic_args_from_cli(rest: Vec<String>) -> serde_json::Value {
    match rest.len() {
        0 => serde_json::json!({}),
        1 => serde_json::json!({ "input": rest[0] }),
        _ => serde_json::json!({
            "input": rest.join(" "),
            "args": rest,
        }),
    }
}

/// Build a JSON args object from positional CLI tokens by binding them
/// one-to-one to the Tool's typed input fields in declaration order
/// (preserved workspace-wide via `serde_json/preserve_order`). See
/// `docs/architecture/call-envelope.md` for the rules.
///
/// Falls back to [`dynamic_args_from_cli`] when the Tool declares no
/// schema fields, so single-input pure functions keep their compact UX.
pub(crate) fn schema_bound_args_from_cli(
    input_spec: &InputSpec,
    rest: Vec<String>,
) -> Result<serde_json::Value, CliError> {
    let fields = input_spec.fields.as_slice();
    if fields.is_empty() {
        return Ok(dynamic_args_from_cli(rest));
    }
    let tokens: Result<Vec<PositionalToken>, CliError> = rest
        .into_iter()
        .map(|t| {
            Ok(if t == "-" {
                PositionalToken::Stdin
            } else {
                PositionalToken::Literal(t)
            })
        })
        .collect();
    let tokens = tokens?;

    let auto_stdin = !std::io::stdin().is_terminal()
        && !tokens.iter().any(|t| matches!(t, PositionalToken::Stdin));

    bind_positionals_to_schema(fields, tokens, auto_stdin, read_stdin)
}

/// Build a JSON object from repeatable `-a name=value` pairs using the
/// selected Tool's canonical typed input spec, plus whatever `reserved`
/// CLI inputs this target accepts (see [`ReservedInputs`]).
///
/// Unlike the pre-C-4 version of this function, this does NOT reject
/// unknown field names or coercion failures itself (with the single
/// exception of a malformed [`ReservedInputs`] value, e.g. `approve=nope`
/// — that has no raw-JSON equivalent to defer to). Both are inserted
/// as-is instead, so [`validate_call_args`] — the SAME shared validator
/// the raw-JSON (`upeg call TOOL '{...}'`) path runs — reports the SAME
/// message for the SAME mistake regardless of which arg syntax produced
/// it (C-4).
pub(crate) fn named_args_from_cli(
    input_spec: &InputSpec,
    reserved: ReservedInputs,
    arg: Vec<(String, Value)>,
) -> Result<Value, CliError> {
    let mut obj = Map::with_capacity(arg.len());
    for (name, raw_value) in arg {
        let Some(field) = input_spec
            .fields
            .iter()
            .find(|field| field.name.as_str() == name)
        else {
            let value = if reserved.accepts(&name) {
                reserved_boolean_value(&name, raw_value)?
            } else {
                raw_value
            };
            obj.insert(name, value);
            continue;
        };
        insert_named_arg(&mut obj, field, raw_value)?;
    }
    Ok(Value::Object(obj))
}

/// Insert one `-a name=value` pair's coerced form for a field DECLARED
/// on the target's [`InputSpec`]. See [`named_args_from_cli`] for the
/// defer-to-shared-validator design; `Json`/`File` are the two kinds
/// exempt from it (below).
fn insert_named_arg(
    obj: &mut Map<String, Value>,
    field: &InputFieldSpec,
    raw_value: Value,
) -> Result<(), CliError> {
    // These two kinds require real CLI-side interpretation — JSON
    // parsing, file I/O — that has no raw-JSON equivalent to defer to
    // (core's `InputKind::Json` validator accepts any value, and a
    // `File` value is never "one JSON type away" from correct), so they
    // still fail immediately with a CLI-specific message.
    if matches!(field.kind, InputKind::Json | InputKind::File(_)) {
        if let Some(value) = input_value_from_cli_json(field, raw_value)? {
            obj.insert(field.name.as_str().to_string(), value.into_json_value());
        }
        return Ok(());
    }
    // Splitting comma text into a list is CLI-specific shaping with no
    // raw-JSON equivalent step; whether each token is a *valid* choice
    // is left entirely to the shared validator, exactly like an invalid
    // single `Options` choice below.
    if let InputKind::MultiOptions(_) = field.kind
        && let Value::String(text) = &raw_value
    {
        let tokens = comma_tokens(text);
        if !tokens.is_empty() {
            obj.insert(
                field.name.as_str().to_string(),
                Value::Array(tokens.into_iter().map(Value::String).collect()),
            );
        }
        return Ok(());
    }
    match input_value_from_cli_json(field, raw_value.clone()) {
        Ok(Some(value)) => {
            obj.insert(field.name.as_str().to_string(), value.into_json_value());
        }
        Ok(None) => {}
        // Coercion or choice-validity failed (wrong JSON type, unknown
        // option, unparsable number, ...): insert the ORIGINAL raw
        // value and let the shared validator report it.
        Err(_) => {
            obj.insert(field.name.as_str().to_string(), raw_value);
        }
    }
    Ok(())
}

/// Boolean coercion for a reserved CLI input (currently only `approve`).
/// Unlike declared-field coercion this is NOT deferred to the shared
/// validator: core's `InputSpec::validate_json_args` only ever examines
/// DECLARED fields, so a reserved input's own value would otherwise
/// never be checked at all.
fn reserved_boolean_value(reserved_name: &str, raw: Value) -> Result<Value, CliError> {
    match raw {
        Value::Bool(_) => Ok(raw),
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.eq_ignore_ascii_case("true") {
                Ok(Value::Bool(true))
            } else if trimmed.eq_ignore_ascii_case("false") {
                Ok(Value::Bool(false))
            } else {
                Err(CliError::tool_failed(format!(
                    "reserved input `{reserved_name}` expected boolean; got `{trimmed}`"
                )))
            }
        }
        other => Err(CliError::tool_failed(format!(
            "reserved input `{reserved_name}` expected boolean; got `{}`",
            json_value_kind(&other)
        ))),
    }
}

/// Read `upeg call TOOL <args>`'s positional args string, resolving the
/// `-` stdin placeholder, without parsing it as JSON yet. Shared by
/// [`crate::app::parse_call_args`] and the `run_call_command` pipeline
/// so both build the same raw-text-to-stdin behaviour once.
pub(crate) fn read_call_args_text(args: String) -> Result<String, CliError> {
    if args == "-" {
        use std::io::Read as _;
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| CliError::tool_failed(format!("read stdin: {e}")))?;
        Ok(buf)
    } else {
        Ok(args)
    }
}

/// [`read_call_args_text`] plus the JSON parse — the raw-JSON half of
/// `upeg call TOOL <args>` / `upeg call TOOL -`.
///
/// On a parse failure the target's [`InputSpec`] (when the tool is
/// registered) is folded into the message. serde_json alone reports
/// only where the text stopped being JSON — `expected value at line 1
/// column 1` for `upeg call num.hex_to_decimal input=0xff` — which
/// says "fix your JSON" and hides the fact that `-a key=value` and the
/// dynamic route exist. Since this is exactly the moment a caller is
/// hand-writing JSON and getting it wrong, it is also the only moment
/// the cheaper syntax is worth naming.
pub(crate) fn read_call_args_json(
    args: String,
    input_spec: Option<&InputSpec>,
    reserved: ReservedInputs,
) -> Result<Value, CliError> {
    let raw = read_call_args_text(args)?;
    serde_json::from_str(&raw)
        .map_err(|e| invalid_args_json_error(&e.to_string(), &raw, input_spec, reserved))
}

/// Hint appended when the failed positional text is itself a `key=value`
/// pair — the caller meant `-a` and dropped the flag, so the fix is the
/// flag, not the JSON.
const MISSING_ARG_FLAG_HINT: &str = "looks like a `-a` argument; write";

/// Hint appended to every other raw-JSON parse failure.
const RAW_JSON_ALTERNATIVE_HINT: &str = "raw JSON is only one of three arg forms; `-a key=value`";

fn invalid_args_json_error(
    parse_error: &str,
    raw: &str,
    input_spec: Option<&InputSpec>,
    reserved: ReservedInputs,
) -> CliError {
    let mut message = format!("invalid args JSON: {parse_error}");
    let trimmed = raw.trim();

    // `input=0xff` reaching the JSON parser is an unambiguous
    // dropped-flag: quote it back with the flag it was missing.
    if let Some((key, _)) = trimmed.split_once('=')
        && is_bare_input_key(key)
    {
        message.push_str(&format!(
            "\nhint: `{trimmed}` {MISSING_ARG_FLAG_HINT} `-a {trimmed}`"
        ));
    } else {
        message.push_str(&format!(
            "\nhint: {RAW_JSON_ALTERNATIVE_HINT} avoids JSON syntax; shell quoting still applies to spaces and special characters"
        ));
    }

    if let Some(spec) = input_spec {
        message.push_str(&format!(
            "\nexpected inputs: {}",
            expected_input_names(spec, reserved)
        ));
    }
    CliError::tool_failed(message)
}

/// Whether `key` reads as a bare input name rather than JSON syntax, so
/// `input=0xff` is treated as a dropped `-a` but `{"a":1}=x` is not.
fn is_bare_input_key(key: &str) -> bool {
    let key = key.trim();
    !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

/// SINGLE CLI-level validation entry point for a built args object,
/// shared by the `-a key=value` and raw-JSON (`upeg call TOOL '{...}'`)
/// paths so both surfaces of the same mistake report the SAME message
/// (C-4): unknown keys — ignoring the reserved `_upeg` execution-context
/// key and, for Chain tools ([`ReservedInputs`]), the reserved `approve`
/// key — are rejected first, then [`InputSpec::validate_json_args`]
/// (upeg-core) checks requiredness/type/choice for the declared fields.
pub(crate) fn validate_call_args(
    input_spec: &InputSpec,
    reserved: ReservedInputs,
    args: &Value,
) -> Result<(), CliError> {
    match args {
        Value::Null => input_spec
            .validate_json_args(&Map::new())
            .map_err(input_value_error),
        Value::Object(obj) => {
            reject_unknown_keys(input_spec, reserved, obj)?;
            input_spec
                .validate_json_args(obj)
                .map_err(input_value_error)
        }
        _ => Ok(()),
    }
}

fn reject_unknown_keys(
    input_spec: &InputSpec,
    reserved: ReservedInputs,
    obj: &Map<String, Value>,
) -> Result<(), CliError> {
    for name in obj.keys() {
        if name == EXECUTION_CONTEXT_ARG {
            continue;
        }
        if reserved.accepts(name) {
            continue;
        }
        if input_spec
            .fields
            .iter()
            .any(|field| field.name.as_str() == name)
        {
            continue;
        }
        return Err(CliError::tool_failed(format!(
            "unknown input `{name}`; expected {}",
            expected_input_names(input_spec, reserved)
        )));
    }
    Ok(())
}

/// Positional CLI token after `{toolkit} {tool}`. `Stdin` is the
/// explicit `-` placeholder meaning "read this slot from stdin".
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PositionalToken {
    Literal(String),
    Stdin,
}

/// Pure (no global I/O) schema-positional binder. The caller injects
/// stdin reading so this function is deterministic and unit-testable.
/// `auto_stdin = true` means "stdin is piped and not yet consumed — bind
/// it to the first unfilled slot." Fields that receive nothing are
/// **omitted** from the result so dispatcher-side defaults stay live.
pub(crate) fn bind_positionals_to_schema(
    fields: &[InputFieldSpec],
    tokens: Vec<PositionalToken>,
    auto_stdin: bool,
    mut read_stdin: impl FnMut() -> Result<String, CliError>,
) -> Result<serde_json::Value, CliError> {
    if tokens.len() > fields.len() {
        return Err(CliError::tool_failed(format!(
            "expected at most {} positional argument(s) for input schema; got {}",
            fields.len(),
            tokens.len(),
        )));
    }

    let mut bound: Vec<Option<String>> = vec![None; fields.len()];
    let mut stdin_consumed = false;
    for (i, tok) in tokens.into_iter().enumerate() {
        bound[i] = Some(match tok {
            PositionalToken::Literal(v) => v,
            PositionalToken::Stdin => {
                if stdin_consumed {
                    return Err(CliError::tool_failed(
                        "`-` (stdin) can only appear once in a CLI invocation",
                    ));
                }
                stdin_consumed = true;
                read_stdin()?
            }
        });
    }

    if auto_stdin
        && !stdin_consumed
        && bound.first().is_some_and(Option::is_none)
        && fields.first().is_some_and(|f| f.required)
    {
        bound[0] = Some(read_stdin()?);
    }

    let mut obj = serde_json::Map::new();
    for (field, value) in fields.iter().zip(bound) {
        let Some(raw) = value else { continue };
        let Some(value) = input_value_from_cli_text(field, raw)? else {
            continue;
        };
        obj.insert(field.name.as_str().to_string(), value.into_json_value());
    }
    Ok(serde_json::Value::Object(obj))
}

fn input_value_from_cli_json(
    field: &InputFieldSpec,
    raw: Value,
) -> Result<Option<InputValue>, CliError> {
    if raw.is_null() {
        return Ok(None);
    }
    match (&field.kind, raw) {
        (InputKind::String, Value::String(value)) => Ok(Some(InputValue::String(value))),
        (InputKind::Markdown, Value::String(value)) => Ok(Some(InputValue::Markdown(value))),
        (InputKind::DateTime, Value::String(value)) => Ok(Some(InputValue::DateTime(value))),
        (InputKind::FilePath, Value::String(value)) => Ok(Some(InputValue::FilePath(value))),
        (InputKind::Url, Value::String(value)) => Ok(Some(InputValue::Url(value))),
        (InputKind::Number, Value::Number(value)) => Ok(Some(InputValue::Number(value))),
        (InputKind::Integer, Value::Number(value)) => value.as_i64().map_or_else(
            || Err(invalid_input_value(field, "integer", value.to_string())),
            |value| Ok(Some(InputValue::Integer(value))),
        ),
        (InputKind::Boolean, Value::Bool(value)) => Ok(Some(InputValue::Boolean(value))),
        (InputKind::Options(choices), Value::String(value)) => {
            option_value(field, choices, value).map(|value| value.map(InputValue::Options))
        }
        (InputKind::MultiOptions(choices), Value::Array(values)) => {
            let values = values
                .into_iter()
                .map(|value| match value {
                    Value::String(value) => Ok(value),
                    other => Err(invalid_input_value(
                        field,
                        "string array",
                        json_value_kind(&other).to_string(),
                    )),
                })
                .collect::<Result<Vec<_>, _>>()?;
            multi_options_value(field, choices, values)
                .map(|value| value.map(InputValue::MultiOptions))
        }
        (InputKind::Json, value) if !value.is_string() => Ok(Some(InputValue::Json(value))),
        (_, Value::String(value)) => input_value_from_cli_text(field, value),
        (_, other) => Err(invalid_input_value(
            field,
            field.kind.label(),
            json_value_kind(&other).to_string(),
        )),
    }
}

fn input_value_from_cli_text(
    field: &InputFieldSpec,
    raw: String,
) -> Result<Option<InputValue>, CliError> {
    match &field.kind {
        InputKind::String => Ok(Some(InputValue::String(raw))),
        InputKind::Markdown => Ok(Some(InputValue::Markdown(raw))),
        InputKind::DateTime => Ok(Some(InputValue::DateTime(raw))),
        InputKind::FilePath => Ok(Some(InputValue::FilePath(raw))),
        InputKind::Url => Ok(Some(InputValue::Url(raw))),
        InputKind::Number => number_value(field, &raw),
        InputKind::Integer => integer_value(field, &raw),
        InputKind::Boolean => boolean_value(field, &raw),
        InputKind::Options(choices) => {
            option_value(field, choices, raw).map(|value| value.map(InputValue::Options))
        }
        InputKind::MultiOptions(choices) => multi_options_value(field, choices, comma_tokens(&raw))
            .map(|value| value.map(InputValue::MultiOptions)),
        InputKind::Json => json_value(field, &raw),
        InputKind::File(policy) => file_value_from_cli_path(field, policy, &raw),
    }
}

fn number_value(field: &InputFieldSpec, raw: &str) -> Result<Option<InputValue>, CliError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let value = trimmed
        .parse::<f64>()
        .ok()
        .and_then(serde_json::Number::from_f64)
        .ok_or_else(|| invalid_input_value(field, "number", raw.to_string()))?;
    Ok(Some(InputValue::Number(value)))
}

fn integer_value(field: &InputFieldSpec, raw: &str) -> Result<Option<InputValue>, CliError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let value = trimmed
        .parse::<i64>()
        .map_err(|_| invalid_input_value(field, "integer", raw.to_string()))?;
    Ok(Some(InputValue::Integer(value)))
}

fn boolean_value(field: &InputFieldSpec, raw: &str) -> Result<Option<InputValue>, CliError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.eq_ignore_ascii_case("true") {
        return Ok(Some(InputValue::Boolean(true)));
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return Ok(Some(InputValue::Boolean(false)));
    }
    Err(invalid_input_value(field, "boolean", raw.to_string()))
}

fn option_value(
    field: &InputFieldSpec,
    choices: &ChoiceSpec,
    raw: String,
) -> Result<Option<String>, CliError> {
    let value = raw.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if choices.contains(value) {
        Ok(Some(value.to_string()))
    } else {
        Err(invalid_choice_value(field, choices, value))
    }
}

fn multi_options_value(
    field: &InputFieldSpec,
    choices: &ChoiceSpec,
    values: Vec<String>,
) -> Result<Option<Vec<String>>, CliError> {
    if values.is_empty() {
        return Ok(None);
    }
    for value in &values {
        if !choices.contains(value) {
            return Err(invalid_choice_value(field, choices, value));
        }
    }
    Ok(Some(values))
}

fn json_value(field: &InputFieldSpec, raw: &str) -> Result<Option<InputValue>, CliError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    serde_json::from_str(trimmed)
        .map(InputValue::Json)
        .map(Some)
        .map_err(|err| invalid_input_value(field, "json", err.to_string()))
}

fn comma_tokens(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn expected_input_names(input_spec: &InputSpec, reserved: ReservedInputs) -> String {
    let names: Vec<String> = input_spec
        .fields
        .iter()
        .map(|field| format!("`{}`", field.name))
        .chain(reserved.names().map(|name| format!("`{name}`")))
        .collect();
    if names.is_empty() {
        "no inputs".to_string()
    } else {
        names.join(", ")
    }
}

fn invalid_input_value(field: &InputFieldSpec, expected: &str, actual: String) -> CliError {
    CliError::tool_failed(format!(
        "input `{}` expected {expected}; got `{actual}`",
        field.name
    ))
}

fn invalid_choice_value(field: &InputFieldSpec, choices: &ChoiceSpec, value: &str) -> CliError {
    CliError::tool_failed(format!(
        "input `{}` must be one of {:?}; got `{value}`",
        field.name,
        choices.allowed_values()
    ))
}

fn input_value_error(err: upeg_core::InputValueError) -> CliError {
    CliError::tool_failed(err.to_string())
}

fn json_value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod schema_bound_tests {
    use super::{PositionalToken, bind_positionals_to_schema, dynamic_args_from_cli};
    use serde_json::json;
    use upeg_core::{ChoiceOption, ChoiceSpec, InputFieldSpec, InputKind, InputName, InputSpec};

    fn field(name: &str, kind: InputKind, required: bool) -> InputFieldSpec {
        InputFieldSpec::new(
            InputName::new(name).expect("test field names are valid"),
            None,
            None,
            required,
            kind,
        )
        .expect("test field specs are valid")
    }

    fn choices(values: &[&str]) -> ChoiceSpec {
        ChoiceSpec::new(
            values
                .iter()
                .map(|value| ChoiceOption::new(*value, None, None).expect("test choice is valid"))
                .collect(),
        )
        .expect("test choice spec is valid")
    }

    fn no_stdin() -> Result<String, super::CliError> {
        Err(super::CliError::tool_failed("stdin must not be read"))
    }

    #[test]
    fn 입력을_선언하지_않은_도구는_표준입력을_읽지_않는다() {
        // `upeg time iso-now` / `upeg id uuid-v7`: no declared field means
        // nothing for stdin to bind to. This branch used to read stdin
        // whenever it was not a TTY, which hung forever on an inherited
        // pipe that never reaches EOF.
        assert_eq!(dynamic_args_from_cli(Vec::new()), json!({}));
    }

    #[test]
    fn 입력을_선언하지_않은_도구의_위치인자는_input으로_접힌다() {
        assert_eq!(
            dynamic_args_from_cli(vec!["0xff".to_string()]),
            json!({ "input": "0xff" })
        );
        assert_eq!(
            dynamic_args_from_cli(vec!["a".to_string(), "b".to_string()]),
            json!({ "input": "a b", "args": ["a", "b"] })
        );
    }

    #[test]
    fn 위치인자는_선언_순서대로_바인드된다() {
        let fields = vec![
            field("a", InputKind::String, true),
            field("b", InputKind::String, true),
        ];
        let tokens = vec![
            PositionalToken::Literal("hello".into()),
            PositionalToken::Literal("world".into()),
        ];
        let v = bind_positionals_to_schema(&fields, tokens, false, no_stdin).unwrap();
        assert_eq!(v, json!({ "a": "hello", "b": "world" }));
    }

    #[test]
    fn 채워지지_않은_선택_필드는_빈_문자열이_아니라_생략된다() {
        let fields = vec![
            field("input", InputKind::String, true),
            field("delim", InputKind::String, false),
        ];
        let tokens = vec![PositionalToken::Literal(r#"["a","b","c"]"#.into())];
        let v = bind_positionals_to_schema(&fields, tokens, false, no_stdin).unwrap();
        let obj = v.as_object().unwrap();
        assert!(obj.contains_key("input"));
        assert!(
            !obj.contains_key("delim"),
            "omitted optional must not appear in args (got {obj:?})",
        );
    }

    #[test]
    fn 너무_너무많은_위치인자들은_하나의_도구_오류이다() {
        let fields = vec![field("input", InputKind::String, true)];
        let tokens = vec![
            PositionalToken::Literal("a".into()),
            PositionalToken::Literal("b".into()),
        ];
        let err = bind_positionals_to_schema(&fields, tokens, false, no_stdin).unwrap_err();
        assert!(
            err.message().contains("at most 1"),
            "got: {}",
            err.message()
        );
    }

    #[test]
    fn 표준입력_자리표시자는_주입된_리더에서_값을_가져온다() {
        let fields = vec![
            field("a", InputKind::String, true),
            field("b", InputKind::String, true),
        ];
        let tokens = vec![
            PositionalToken::Literal("first".into()),
            PositionalToken::Stdin,
        ];
        let v =
            bind_positionals_to_schema(&fields, tokens, false, || Ok("from-stdin".into())).unwrap();
        assert_eq!(v, json!({ "a": "first", "b": "from-stdin" }));
    }

    #[test]
    fn 중복된_표준입력_자리표시자는_하나의_오류이다() {
        let fields = vec![
            field("a", InputKind::String, true),
            field("b", InputKind::String, true),
        ];
        let tokens = vec![PositionalToken::Stdin, PositionalToken::Stdin];
        let err =
            bind_positionals_to_schema(&fields, tokens, false, || Ok("x".into())).unwrap_err();
        assert!(err.message().contains("stdin"), "got: {}", err.message());
    }

    #[test]
    fn 위치인자가_주어지지_않으면_자동_표준입력이_첫번째_슬롯을_채운다() {
        let fields = vec![field("input", InputKind::String, true)];
        let tokens = vec![];
        let v = bind_positionals_to_schema(&fields, tokens, true, || Ok("piped".into())).unwrap();
        assert_eq!(v, json!({ "input": "piped" }));
    }

    #[test]
    fn 자동_표준입력은_명시적_위치인자를_덮어쓰기하지_않는다() {
        let fields = vec![field("input", InputKind::String, true)];
        let tokens = vec![PositionalToken::Literal("explicit".into())];
        let v = bind_positionals_to_schema(&fields, tokens, true, || {
            panic!("auto_stdin must not fire when a positional is present")
        })
        .unwrap();
        assert_eq!(v, json!({ "input": "explicit" }));
    }

    #[test]
    fn 첫번째_필드가_선택이면_자동_표준입력은_실행되지_않는다() {
        let fields = vec![field("n", InputKind::Integer, false)];
        let tokens = vec![];
        let v = bind_positionals_to_schema(&fields, tokens, true, || {
            panic!("auto_stdin must not fire when leading field is optional")
        })
        .unwrap();
        assert_eq!(v, json!({}));
    }

    #[test]
    fn 정수_필드는_강제변환된_에서_문자열_토큰이다() {
        let fields = vec![
            field("input", InputKind::String, true),
            field("n", InputKind::Integer, false),
        ];
        let tokens = vec![
            PositionalToken::Literal("hi".into()),
            PositionalToken::Literal("  3  ".into()),
        ];
        let v = bind_positionals_to_schema(&fields, tokens, false, no_stdin).unwrap();
        assert_eq!(v, json!({ "input": "hi", "n": 3 }));
    }

    #[test]
    fn 불리언_필드는_대소문자_무시_참이다() {
        let fields = vec![field("flag", InputKind::Boolean, false)];
        let tokens = vec![PositionalToken::Literal("TRUE".into())];
        let v = bind_positionals_to_schema(&fields, tokens, false, no_stdin).unwrap();
        assert_eq!(v, json!({ "flag": true }));
    }

    #[test]
    fn 다중_옵션_필드는_빈_입력_단일_입력_중복_유효하지_않은_토큰을_모두_처리한다() {
        let fields = vec![field(
            "flags",
            InputKind::MultiOptions(choices(&["dry", "verbose"])),
            false,
        )];

        let empty = bind_positionals_to_schema(
            &fields,
            vec![PositionalToken::Literal(" , ".into())],
            false,
            no_stdin,
        )
        .unwrap();
        assert_eq!(empty, json!({}));

        let single = bind_positionals_to_schema(
            &fields,
            vec![PositionalToken::Literal("dry".into())],
            false,
            no_stdin,
        )
        .unwrap();
        assert_eq!(single, json!({ "flags": ["dry"] }));

        let duplicate = bind_positionals_to_schema(
            &fields,
            vec![PositionalToken::Literal("dry,dry".into())],
            false,
            no_stdin,
        )
        .unwrap();
        assert_eq!(duplicate, json!({ "flags": ["dry", "dry"] }));

        let invalid = bind_positionals_to_schema(
            &fields,
            vec![PositionalToken::Literal("dry,loud".into())],
            false,
            no_stdin,
        )
        .unwrap_err();
        assert!(
            invalid.message().contains("must be one of") && invalid.message().contains("loud"),
            "got: {}",
            invalid.message()
        );
    }

    #[test]
    fn 빈_필수_multi_options는_input_spec_검증에서_거부된다() {
        let input_spec = InputSpec::new(vec![field(
            "flags",
            InputKind::MultiOptions(choices(&["dry", "verbose"])),
            true,
        )])
        .expect("test input spec is valid");

        let args = bind_positionals_to_schema(
            &input_spec.fields,
            vec![PositionalToken::Literal(String::new())],
            false,
            no_stdin,
        )
        .unwrap();
        assert_eq!(args, json!({}));

        let err = input_spec
            .validate_json_args(args.as_object().expect("args object"))
            .expect_err("required empty multi_options should fail at validation");
        assert_eq!(err.to_string(), "input `flags` is required");
    }

    #[test]
    fn 빈_토큰과_자동_표준입력이_없으면_빈_객체를_생성한다() {
        let fields = vec![field("input", InputKind::String, true)];
        let v = bind_positionals_to_schema(&fields, vec![], false, no_stdin).unwrap();
        assert_eq!(v, json!({}));
    }
}

#[cfg(test)]
mod file_input_policy_tests;
