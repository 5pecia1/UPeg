//! Optional per-field constraints attached to [`super::InputFieldSpec`].
//!
//! Constraints are inline-declared on each input via macro syntax (e.g.
//! `Number(min=1, max=255, default=128)`) or TOML keys. Each sub-field of
//! [`FieldConstraints`] is `None` when not declared so the default is
//! zero-cost.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Optional per-field constraints attached to
/// [`super::InputFieldSpec`].
///
/// Each sub-field is `None` when not declared. The macro and the TOML
/// parser fill the relevant sub-struct based on the field's inline
/// declaration (e.g. `Number(min=1, max=255, default=128)`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FieldConstraints {
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub number: Option<NumberConstraints>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub string: Option<StringConstraints>,
}

impl FieldConstraints {
    pub fn is_empty(&self) -> bool {
        self.number.is_none() && self.string.is_none()
    }
}

/// Range and default for numeric inputs.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct NumberConstraints {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub default: Option<f64>,
}

impl Eq for NumberConstraints {}

/// Regex, placeholder, and default for string inputs.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct StringConstraints {
    pub regex: Option<String>,
    pub placeholder: Option<String>,
    pub default: Option<String>,
}

/// Inventory-friendly `&'static`-friendly form of [`FieldConstraints`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StaticFieldConstraints {
    pub number: Option<StaticNumberConstraints>,
    pub string: Option<StaticStringConstraints>,
}

impl StaticFieldConstraints {
    pub const fn empty() -> Self {
        Self {
            number: None,
            string: None,
        }
    }

    pub fn to_owned(self) -> FieldConstraints {
        FieldConstraints {
            number: self.number.map(|c| NumberConstraints {
                min: c.min,
                max: c.max,
                default: c.default,
            }),
            string: self.string.map(|c| StringConstraints {
                regex: c.regex.map(str::to_string),
                placeholder: c.placeholder.map(str::to_string),
                default: c.default.map(str::to_string),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaticNumberConstraints {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub default: Option<f64>,
}

impl Eq for StaticNumberConstraints {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticStringConstraints {
    pub regex: Option<&'static str>,
    pub placeholder: Option<&'static str>,
    pub default: Option<&'static str>,
}
