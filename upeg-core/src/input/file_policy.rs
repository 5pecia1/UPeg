use std::collections::BTreeSet;
use std::num::NonZeroU32;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use super::file_resource_limits::MAX_FILE_INPUT_COUNT;

pub const DEFAULT_FILE_MAX_COUNT: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileInputPolicy {
    max_count: NonZeroU32,
    extensions: BTreeSet<FileExtension>,
    max_file_bytes: Option<u64>,
    max_total_bytes: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct FileInputPolicyParams {
    pub max_count: u32,
    pub extensions: Vec<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub max_file_bytes: Option<u64>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub max_total_bytes: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticFileInputPolicy {
    pub max_count: u32,
    pub extensions: &'static [&'static str],
    pub max_file_bytes: Option<u64>,
    pub max_total_bytes: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FileInputPolicyError {
    #[error("file input max_count must be greater than zero")]
    ZeroMaxCount,
    #[error("file input max_count {actual} exceeds the maximum of {max}")]
    MaxCountExceeded { max: u32, actual: u32 },
    #[error("invalid file extension `{extension}`")]
    InvalidExtension { extension: String },
    #[error("file extension `{extension}` appears more than once")]
    DuplicateExtension { extension: String },
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct FileExtension(String);

impl Default for FileInputPolicy {
    fn default() -> Self {
        Self {
            max_count: NonZeroU32::MIN,
            extensions: BTreeSet::new(),
            max_file_bytes: None,
            max_total_bytes: None,
        }
    }
}

impl Default for FileInputPolicyParams {
    fn default() -> Self {
        Self {
            max_count: DEFAULT_FILE_MAX_COUNT,
            extensions: Vec::new(),
            max_file_bytes: None,
            max_total_bytes: None,
        }
    }
}

impl StaticFileInputPolicy {
    pub const DEFAULT: Self = Self {
        max_count: DEFAULT_FILE_MAX_COUNT,
        extensions: &[],
        max_file_bytes: None,
        max_total_bytes: None,
    };
}

impl Default for StaticFileInputPolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl TryFrom<FileInputPolicyParams> for FileInputPolicy {
    type Error = FileInputPolicyError;

    fn try_from(params: FileInputPolicyParams) -> Result<Self, Self::Error> {
        let max_count =
            NonZeroU32::new(params.max_count).ok_or(FileInputPolicyError::ZeroMaxCount)?;
        if params.max_count > MAX_FILE_INPUT_COUNT {
            return Err(FileInputPolicyError::MaxCountExceeded {
                max: MAX_FILE_INPUT_COUNT,
                actual: params.max_count,
            });
        }
        let mut extensions = BTreeSet::new();
        for extension in params.extensions {
            let extension = FileExtension::try_from(extension)?;
            if !extensions.insert(extension.clone()) {
                return Err(FileInputPolicyError::DuplicateExtension {
                    extension: extension.0,
                });
            }
        }
        Ok(Self {
            max_count,
            extensions,
            max_file_bytes: params.max_file_bytes,
            max_total_bytes: params.max_total_bytes,
        })
    }
}

impl From<&FileInputPolicy> for FileInputPolicyParams {
    fn from(policy: &FileInputPolicy) -> Self {
        Self {
            max_count: policy.max_count(),
            extensions: policy.extensions().map(str::to_string).collect(),
            max_file_bytes: policy.max_file_bytes,
            max_total_bytes: policy.max_total_bytes,
        }
    }
}

#[cfg(feature = "serde")]
impl Serialize for FileInputPolicy {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        FileInputPolicyParams::from(self).serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for FileInputPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Self::try_from(FileInputPolicyParams::deserialize(deserializer)?)
            .map_err(serde::de::Error::custom)
    }
}

impl FileInputPolicy {
    #[must_use]
    pub fn max_count(&self) -> u32 {
        self.max_count.get()
    }

    pub fn extensions(&self) -> impl ExactSizeIterator<Item = &str> {
        self.extensions.iter().map(FileExtension::as_str)
    }

    #[must_use]
    pub const fn max_file_bytes(&self) -> Option<u64> {
        self.max_file_bytes
    }

    #[must_use]
    pub const fn max_total_bytes(&self) -> Option<u64> {
        self.max_total_bytes
    }

    pub(super) fn accepts_extension(&self, file_name: &str) -> bool {
        if self.extensions.is_empty() {
            return true;
        }
        let file_name = file_name.to_ascii_lowercase();
        self.extensions.iter().any(|extension| {
            file_name
                .strip_suffix(extension.as_str())
                .is_some_and(|prefix| prefix.ends_with('.'))
        })
    }
}

impl TryFrom<String> for FileExtension {
    type Error = FileInputPolicyError;

    fn try_from(raw: String) -> Result<Self, Self::Error> {
        let unprefixed = raw.strip_prefix('.').unwrap_or(&raw);
        let invalid = unprefixed.is_empty()
            || !unprefixed.is_ascii()
            || unprefixed.starts_with('.')
            || unprefixed.ends_with('.')
            || unprefixed.chars().any(|character| {
                character.is_ascii_whitespace() || matches!(character, '/' | '\\' | '\0')
            });
        if invalid {
            return Err(FileInputPolicyError::InvalidExtension { extension: raw });
        }
        Ok(Self(unprefixed.to_ascii_lowercase()))
    }
}

impl FileExtension {
    fn as_str(&self) -> &str {
        &self.0
    }
}
