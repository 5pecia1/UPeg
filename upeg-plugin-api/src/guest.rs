//! Guest runtime traits bridging plugin JSON args/results to typed Rust
//! values.
//!
//! The host calls a plugin export with the JSON-stringified args object; the
//! `#[upeg::tool]`-generated wrapper (in `upeg-plugin-macros`, a sibling
//! crate not yet built) turns that object into the user fn's typed
//! parameters via [`FromPluginArg`], calls the user fn, and encodes its
//! return value as the UTF-8 string the export must produce via
//! [`ToPluginOutput`].
//!
//! Both traits are pure `serde_json` + `std`: this crate is shared by the
//! host and the guest, so it must never depend on `extism-pdk` or any
//! wasm-only crate.

use serde_json::Value;

/// Error message returned by `Option<T>::to_plugin_output` when the tool
/// returned `None`. A dedicated constant avoids the literal drifting
/// between this trait impl and anything that matches on the message text.
pub const NO_VALUE_ERROR: &str = "tool returned no value";

/// Build the error for an argument that is absent or JSON `null`.
///
/// Every non-`Option<T>` extraction has no fallback value, so both the
/// `required` and "declared optional but read as a bare scalar" paths are
/// errors; `required` only changes the wording so the message stays
/// accurate about why the field had to be present.
fn missing_argument_message(name: &str, required: bool) -> String {
    if required {
        format!("missing required argument `{name}`")
    } else {
        format!("missing argument `{name}`")
    }
}

/// Build the error for a JSON value that cannot be coerced to `expected`.
fn type_mismatch_message(name: &str, expected: &str, actual: &Value) -> String {
    format!("argument `{name}` expected {expected}, got `{actual}`")
}

/// Build the error for a numeric string/JSON value outside `expected`'s range.
fn out_of_range_message(name: &str, expected: &str, actual: &Value) -> String {
    format!("argument `{name}` does not fit in {expected}: `{actual}`")
}

/// Look up `name` in the args object, rejecting absent or JSON `null` values.
///
/// Scalar `FromPluginArg` impls all start here: only `serde_json::Value`
/// and `Option<T>` know how to represent "no value" themselves.
fn require_present<'args>(
    args: &'args Value,
    name: &str,
    required: bool,
) -> Result<&'args Value, String> {
    match args.get(name) {
        Some(value) if !value.is_null() => Ok(value),
        _ => Err(missing_argument_message(name, required)),
    }
}

/// Extract a typed parameter from a plugin export's JSON args object.
///
/// Implemented for the closed set of guest parameter types the
/// `#[upeg::tool]` attribute supports: `String`, every built-in integer
/// type, `f32`/`f64`, `bool`, `serde_json::Value`, and `Option<T>` over any
/// of the above.
pub trait FromPluginArg: Sized {
    /// Extract `name` from `args`.
    ///
    /// `required` selects the missing-value policy for scalar types
    /// (missing/null is always an error — only `Option<T>` can represent
    /// "no value"). `Option<T>` ignores the flag it receives and always
    /// resolves missing/null to `None`, delegating to `T::from_plugin_arg`
    /// with `required = true` otherwise.
    ///
    /// # Errors
    ///
    /// Returns `Err` describing the field name and mismatch when the value
    /// is missing, `null` (for non-`Option` types), or cannot be coerced to
    /// `Self`.
    fn from_plugin_arg(args: &Value, name: &str, required: bool) -> Result<Self, String>;
}

impl FromPluginArg for String {
    fn from_plugin_arg(args: &Value, name: &str, required: bool) -> Result<Self, String> {
        let value = require_present(args, name, required)?;
        if let Some(text) = value.as_str() {
            return Ok(text.to_string());
        }
        match value {
            // Any other JSON scalar renders through its own Display, e.g.
            // `42`, `true`. Objects/arrays are not scalars and are rejected.
            Value::Number(_) | Value::Bool(_) => Ok(value.to_string()),
            Value::Null | Value::String(_) | Value::Array(_) | Value::Object(_) => {
                Err(type_mismatch_message(name, "string", value))
            }
        }
    }
}

macro_rules! impl_from_plugin_arg_int {
    ($($int:ty),+ $(,)?) => {
        $(
            impl FromPluginArg for $int {
                fn from_plugin_arg(args: &Value, name: &str, required: bool) -> Result<Self, String> {
                    let value = require_present(args, name, required)?;
                    if let Some(signed) = value.as_i64() {
                        return <$int>::try_from(signed)
                            .map_err(|_| out_of_range_message(name, stringify!($int), value));
                    }
                    if let Some(unsigned) = value.as_u64() {
                        return <$int>::try_from(unsigned)
                            .map_err(|_| out_of_range_message(name, stringify!($int), value));
                    }
                    if let Some(text) = value.as_str() {
                        return text
                            .parse::<$int>()
                            .map_err(|_| out_of_range_message(name, stringify!($int), value));
                    }
                    Err(type_mismatch_message(name, stringify!($int), value))
                }
            }
        )+
    };
}

impl_from_plugin_arg_int!(
    i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize
);

macro_rules! impl_from_plugin_arg_float {
    ($($float:ty),+ $(,)?) => {
        $(
            impl FromPluginArg for $float {
                fn from_plugin_arg(args: &Value, name: &str, required: bool) -> Result<Self, String> {
                    let value = require_present(args, name, required)?;
                    if let Some(number) = value.as_f64() {
                        return Ok(number as $float);
                    }
                    if let Some(text) = value.as_str() {
                        return text
                            .parse::<$float>()
                            .map_err(|_| type_mismatch_message(name, stringify!($float), value));
                    }
                    Err(type_mismatch_message(name, stringify!($float), value))
                }
            }
        )+
    };
}

impl_from_plugin_arg_float!(f32, f64);

impl FromPluginArg for bool {
    fn from_plugin_arg(args: &Value, name: &str, required: bool) -> Result<Self, String> {
        let value = require_present(args, name, required)?;
        if let Some(flag) = value.as_bool() {
            return Ok(flag);
        }
        match value.as_str() {
            Some("true") => Ok(true),
            Some("false") => Ok(false),
            _ => Err(type_mismatch_message(name, "bool", value)),
        }
    }
}

impl FromPluginArg for Value {
    fn from_plugin_arg(args: &Value, name: &str, _required: bool) -> Result<Self, String> {
        // `Value` always succeeds: it is how a tool opts into seeing the
        // raw JSON, missing value included, without an error path.
        Ok(args.get(name).cloned().unwrap_or(Self::Null))
    }
}

impl<T: FromPluginArg> FromPluginArg for Option<T> {
    fn from_plugin_arg(args: &Value, name: &str, _required: bool) -> Result<Self, String> {
        match args.get(name) {
            None | Some(Value::Null) => Ok(None),
            Some(_) => T::from_plugin_arg(args, name, true).map(Some),
        }
    }
}

/// Convert a guest tool's typed return value into the UTF-8 string a
/// plugin export must produce.
///
/// Implemented for the closed set of guest return types the
/// `#[upeg::tool]` attribute supports: `String`, `&str`, every built-in
/// integer type, `f32`/`f64`, `bool`, `Result<T, E: Display>`, and
/// `Option<T>` over any `ToPluginOutput` type.
pub trait ToPluginOutput {
    /// Render `self` as the export's success string, or an error message
    /// that the generated wrapper turns into a host-visible tool error.
    ///
    /// # Errors
    ///
    /// Returns `Err` when `self` carries no renderable value (`Result::Err`,
    /// `Option::None`).
    fn to_plugin_output(self) -> Result<String, String>;
}

impl ToPluginOutput for String {
    fn to_plugin_output(self) -> Result<String, String> {
        Ok(self)
    }
}

impl ToPluginOutput for &str {
    fn to_plugin_output(self) -> Result<String, String> {
        Ok(self.to_string())
    }
}

macro_rules! impl_to_plugin_output_display {
    ($($scalar:ty),+ $(,)?) => {
        $(
            impl ToPluginOutput for $scalar {
                fn to_plugin_output(self) -> Result<String, String> {
                    Ok(self.to_string())
                }
            }
        )+
    };
}

impl_to_plugin_output_display!(
    i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, f32, f64, bool
);

impl<T, E> ToPluginOutput for Result<T, E>
where
    T: ToPluginOutput,
    E: std::fmt::Display,
{
    fn to_plugin_output(self) -> Result<String, String> {
        match self {
            Ok(value) => value.to_plugin_output(),
            Err(error) => Err(error.to_string()),
        }
    }
}

impl<T: ToPluginOutput> ToPluginOutput for Option<T> {
    fn to_plugin_output(self) -> Result<String, String> {
        match self {
            Some(value) => value.to_plugin_output(),
            None => Err(NO_VALUE_ERROR.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(json: &str) -> Value {
        serde_json::from_str(json).expect("test fixture must be valid JSON")
    }

    #[test]
    fn 정수_입력은_json_숫자와_문자열_모두_허용한다() {
        let numeric = args(r#"{"count": 42}"#);
        let stringy = args(r#"{"count": "42"}"#);

        assert_eq!(i32::from_plugin_arg(&numeric, "count", true), Ok(42));
        assert_eq!(i32::from_plugin_arg(&stringy, "count", true), Ok(42));
    }

    #[test]
    fn 범위를_초과하는_정수는_거부한다() {
        let too_big = args(r#"{"count": 999}"#);
        let negative = args(r#"{"count": -1}"#);

        assert!(u8::from_plugin_arg(&too_big, "count", true).is_err());
        assert!(u8::from_plugin_arg(&negative, "count", true).is_err());
    }

    #[test]
    fn 큰_u128은_문자열_인코딩으로_받는다() {
        let huge = args(r#"{"count": "340282366920938463463374607431768211455"}"#);

        assert_eq!(u128::from_plugin_arg(&huge, "count", true), Ok(u128::MAX));
    }

    #[test]
    fn 필수_필드가_누락되면_필드명을_포함한_에러를_반환한다() {
        let empty = args("{}");

        let err = String::from_plugin_arg(&empty, "name", true).unwrap_err();

        assert!(
            err.contains("name"),
            "error message should name the missing field: {err}"
        );
    }

    #[test]
    fn 필수_필드가_null이면_에러를_반환한다() {
        let with_null = args(r#"{"name": null}"#);

        assert!(String::from_plugin_arg(&with_null, "name", true).is_err());
    }

    #[test]
    fn 옵션_필드가_누락되면_none을_반환한다() {
        let empty = args("{}");

        assert_eq!(
            Option::<i32>::from_plugin_arg(&empty, "count", false),
            Ok(None)
        );
    }

    #[test]
    fn 옵션_필드가_null이면_none을_반환한다() {
        let with_null = args(r#"{"count": null}"#);

        assert_eq!(
            Option::<i32>::from_plugin_arg(&with_null, "count", false),
            Ok(None)
        );
    }

    #[test]
    fn 옵션_필드가_존재하면_내부_타입으로_역직렬화한다() {
        let present = args(r#"{"count": 7}"#);

        assert_eq!(
            Option::<i32>::from_plugin_arg(&present, "count", false),
            Ok(Some(7))
        );
    }

    #[test]
    fn bool_문자열은_true_false로_코어션한다() {
        let stringy_true = args(r#"{"flag": "true"}"#);
        let stringy_false = args(r#"{"flag": "false"}"#);
        let native = args(r#"{"flag": true}"#);

        assert_eq!(bool::from_plugin_arg(&stringy_true, "flag", true), Ok(true));
        assert_eq!(
            bool::from_plugin_arg(&stringy_false, "flag", true),
            Ok(false)
        );
        assert_eq!(bool::from_plugin_arg(&native, "flag", true), Ok(true));
    }

    #[test]
    fn bool_이외의_문자열은_거부한다() {
        let garbage = args(r#"{"flag": "yes"}"#);

        assert!(bool::from_plugin_arg(&garbage, "flag", true).is_err());
    }

    #[test]
    fn value_필드는_누락되어도_null로_성공한다() {
        let empty = args("{}");

        assert_eq!(
            Value::from_plugin_arg(&empty, "payload", true),
            Ok(Value::Null)
        );
    }

    #[test]
    fn value_필드는_원본_서브트리를_복제한다() {
        let nested = args(r#"{"payload": {"a": 1}}"#);

        assert_eq!(
            Value::from_plugin_arg(&nested, "payload", true),
            Ok(args(r#"{"a": 1}"#))
        );
    }

    #[test]
    fn result_err은_문자열_에러로_변환된다() {
        let failed: Result<String, String> = Err("boom".to_string());

        assert_eq!(failed.to_plugin_output(), Err("boom".to_string()));
    }

    #[test]
    fn result_ok은_내부_값을_렌더링한다() {
        let ok: Result<i32, String> = Ok(42);

        assert_eq!(ok.to_plugin_output(), Ok("42".to_string()));
    }

    #[test]
    fn 옵션_none_출력은_고정_에러_메시지를_반환한다() {
        let none: Option<String> = None;

        assert_eq!(none.to_plugin_output(), Err(NO_VALUE_ERROR.to_string()));
    }

    #[test]
    fn f64와_스칼라는_to_string으로_렌더링한다() {
        assert_eq!(3.5_f64.to_plugin_output(), Ok("3.5".to_string()));
        assert_eq!(42_i32.to_plugin_output(), Ok("42".to_string()));
        assert_eq!(true.to_plugin_output(), Ok("true".to_string()));
    }

    #[test]
    fn 문자열_슬라이스는_소유_문자열로_렌더링한다() {
        let borrowed: &str = "hello";

        assert_eq!(borrowed.to_plugin_output(), Ok("hello".to_string()));
    }
}
