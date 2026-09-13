//! Runtime carrier for the `File` input kind.
//!
//! Unifies single files and directories under one type: `content` carries
//! either the file's bytes or the directory's recursive entries, and that
//! choice alone decides whether the value is a directory. See LEXICON v2.3 §2
//! and the Pin rename + auto-render design doc.

use base64::{Engine as _, engine::general_purpose::STANDARD};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

// `MAX_FILE_NESTING_DEPTH` is declared once in the canonical `file_budget`
// module (shared by input and output). Imported here — rather than
// redeclared — because this decode path is the sole enforcement site:
// `file_value_to_json` cannot report a violation (it is infallible, and its
// callers' signatures are fixed), so the budget is a decode-side contract
// only. Nothing else builds a tree deep enough to care — a wire payload can
// no longer become one. Pinned against the Dart/JS mirrors by
// `upeg-core/tests/file_budget_cross_language_pin.rs`. Only the `serde`
// decode path (below) reads it, hence the cfg gate.
#[cfg(feature = "serde")]
use super::file_budget::MAX_FILE_NESTING_DEPTH;

#[cfg(feature = "serde")]
use super::file_base64::canonical_base64_decoded_len;

/// Runtime carrier for a `File` input or output.
///
/// There is deliberately no `is_dir` field: it would restate what `content`'s
/// variant already says, and a struct with both can be built disagreeing with
/// itself. [`FileValue::is_dir`] derives the answer instead, so the wire keeps
/// the attribute while Rust cannot contradict it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileValue {
    pub name: String,
    pub mime: Option<String>,
    pub content: FileContent,
}

impl FileValue {
    /// Whether this value is a directory — i.e. whether `content` carries
    /// entries rather than bytes. The sole authority for the `is_dir` wire
    /// attribute; both encoders below go through it.
    #[must_use]
    pub fn is_dir(&self) -> bool {
        matches!(self.content, FileContent::Directory(_))
    }
}

/// Key order is load-bearing and byte-visible (`serde_json` is built with
/// `preserve_order`), so this impl writes the map by hand rather than deriving:
/// `is_dir` is computed, and a derive can only emit fields. `name, is_dir,
/// mime, content` is what [`file_value_to_json`] and the Dart
/// `canonicalFileValueToJson` both emit — keep the three in step.
#[cfg(feature = "serde")]
impl Serialize for FileValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry(FILE_KEY_NAME, &self.name)?;
        map.serialize_entry(FILE_KEY_IS_DIR, &self.is_dir())?;
        // Absent `mime` drops the key entirely, matching the
        // `skip_serializing_if` the derive used to carry.
        if let Some(mime) = &self.mime {
            map.serialize_entry(FILE_KEY_MIME, mime)?;
        }
        map.serialize_entry(FILE_KEY_CONTENT, &self.content)?;
        map.end()
    }
}

/// File body — bytes for a regular file, recursive entries for a directory.
///
/// Serde uses hand-written impls (below) rather than a derive: the natural
/// `#[serde(tag = "kind")]` internally-tagged form cannot represent the
/// newtype-with-sequence variants (`Bytes(Vec<u8>)` / `Directory(Vec<_>)`) —
/// serde rejects "tagged newtype variant containing a sequence" at run time.
/// The manual impls emit `{ "kind": "bytes", "bytes": "<base64>" }` /
/// `{ "kind": "directory", "entries": [...] }`, matching [`file_value_to_json`]
/// byte-for-byte so the serde and manual JSON encodings interoperate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileContent {
    Bytes(Vec<u8>),
    Directory(Vec<FileValue>),
}

/// JSON key for a file's own name.
pub(super) const FILE_KEY_NAME: &str = "name";
/// JSON key for the directory discriminant attribute.
pub(super) const FILE_KEY_IS_DIR: &str = "is_dir";
/// JSON key for the optional MIME type.
pub(super) const FILE_KEY_MIME: &str = "mime";
/// JSON key for the file body.
pub(super) const FILE_KEY_CONTENT: &str = "content";
/// JSON key for the body's variant discriminant.
pub(super) const FILE_CONTENT_KEY_KIND: &str = "kind";
/// JSON key for a regular file's byte payload.
pub(super) const FILE_CONTENT_KEY_BYTES: &str = "bytes";
/// JSON key for a directory's recursive entries.
pub(super) const FILE_CONTENT_KEY_ENTRIES: &str = "entries";
/// JSON `content.kind` discriminant for a regular file's byte payload.
pub(super) const FILE_CONTENT_KIND_BYTES: &str = "bytes";
/// JSON `content.kind` discriminant for a directory's recursive entries.
pub(super) const FILE_CONTENT_KIND_DIRECTORY: &str = "directory";

#[cfg(feature = "serde")]
mod nesting {
    use std::cell::Cell;

    use super::MAX_FILE_NESTING_DEPTH;

    thread_local! {
        /// Depth of the `FileValue` decode currently in progress on this
        /// thread. Serde's `Deserialize` carries no state of its own, so the
        /// recursive impls thread the budget through here instead.
        static DEPTH: Cell<usize> = const { Cell::new(0) };
    }

    /// RAII marker for one level of an in-progress decode. Dropping it pops
    /// the level, so an early `Err` return cannot leak the counter.
    pub(super) struct Level {
        depth: usize,
    }

    impl Level {
        /// Push a level and report the depth now occupied.
        pub(super) fn push() -> Self {
            let depth = DEPTH.with(|depth| {
                let pushed = depth.get() + 1;
                depth.set(pushed);
                pushed
            });
            Self { depth }
        }

        /// Whether this level is within [`MAX_FILE_NESTING_DEPTH`].
        pub(super) fn is_within_budget(&self) -> bool {
            self.depth <= MAX_FILE_NESTING_DEPTH
        }
    }

    impl Drop for Level {
        fn drop(&mut self) {
            DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
        }
    }
}

#[cfg(feature = "serde")]
impl Serialize for FileContent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(2))?;
        match self {
            Self::Bytes(bytes) => {
                map.serialize_entry(FILE_CONTENT_KEY_KIND, FILE_CONTENT_KIND_BYTES)?;
                map.serialize_entry(FILE_CONTENT_KEY_BYTES, &STANDARD.encode(bytes))?;
            }
            Self::Directory(entries) => {
                map.serialize_entry(FILE_CONTENT_KEY_KIND, FILE_CONTENT_KIND_DIRECTORY)?;
                map.serialize_entry(FILE_CONTENT_KEY_ENTRIES, entries)?;
            }
        }
        map.end()
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for FileContent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Internally-tagged *struct* variants round-trip cleanly (the newtype
        // form is what serde rejects), so decode through this shim and unwrap.
        // The tag/variant spellings must be literals — serde's attributes take
        // no const paths — so `라운드트립하면_원래_값이_그대로_돌아온다` pins
        // them against the `FILE_CONTENT_*` consts the encoders use.
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
        enum Repr {
            Bytes {
                #[serde(deserialize_with = "deserialize_base64")]
                bytes: Vec<u8>,
            },
            Directory {
                entries: Vec<FileValue>,
            },
        }
        Ok(match Repr::deserialize(deserializer)? {
            Repr::Bytes { bytes } => Self::Bytes(bytes),
            Repr::Directory { entries } => Self::Directory(entries),
        })
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for FileValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error as _;

        // Mirror of the public struct, minus the invariants a derive cannot
        // state. Decoding through it keeps the field/key mapping in one place
        // while the checks below reject what the Dart codec also rejects.
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Repr {
            name: String,
            is_dir: bool,
            #[serde(default)]
            mime: Option<String>,
            content: FileContent,
        }

        // `Repr::deserialize` recurses back into this impl for every directory
        // entry, so the level must be pushed before it runs and stay alive
        // across it.
        let level = nesting::Level::push();
        if !level.is_within_budget() {
            return Err(D::Error::custom(format!(
                "file nesting exceeds the maximum depth of {MAX_FILE_NESTING_DEPTH}"
            )));
        }
        let repr = Repr::deserialize(deserializer)?;
        drop(level);

        // The wire carries `is_dir`, but the decoded value derives it from the
        // body, so the incoming attribute is only ever a claim to check. Dart
        // refuses the disagreeing combinations; so does this. Agreeing ones are
        // dropped on the floor — `content` already said it.
        match (repr.is_dir, &repr.content) {
            (false, FileContent::Directory(_)) => Err(D::Error::custom(format!(
                "`{FILE_KEY_IS_DIR}` is false but `{FILE_KEY_CONTENT}.{FILE_CONTENT_KEY_KIND}` is `{FILE_CONTENT_KIND_DIRECTORY}`"
            ))),
            (true, FileContent::Bytes(_)) => Err(D::Error::custom(format!(
                "`{FILE_KEY_IS_DIR}` is true but `{FILE_KEY_CONTENT}.{FILE_CONTENT_KEY_KIND}` is `{FILE_CONTENT_KIND_BYTES}`"
            ))),
            _ => Ok(Self {
                name: repr.name,
                mime: repr.mime,
                content: repr.content,
            }),
        }
    }
}

#[cfg(feature = "serde")]
fn deserialize_base64<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error as _;

    let encoded = String::deserialize(deserializer)?;
    canonical_base64_decoded_len(&encoded).map_err(D::Error::custom)?;
    STANDARD.decode(encoded).map_err(D::Error::custom)
}

/// Lossless conversion of a [`FileValue`] into a JSON object. Used by
/// `InputValue::into_json_value` and `OutputValue`'s JSON form so the
/// upeg-core crate avoids requiring `FileValue: Serialize` (the serde derive
/// is feature-gated). The sole encoder for this shape on the Rust side —
/// callers holding a borrow clone into it rather than growing a second one.
pub(crate) fn file_value_to_json(file: FileValue) -> serde_json::Value {
    // Read the derived attribute before `file` is taken apart below.
    let is_dir = file.is_dir();
    let mut object = serde_json::Map::new();
    object.insert(
        FILE_KEY_NAME.to_string(),
        serde_json::Value::String(file.name),
    );
    object.insert(FILE_KEY_IS_DIR.to_string(), serde_json::Value::Bool(is_dir));
    if let Some(mime) = file.mime {
        object.insert(FILE_KEY_MIME.to_string(), serde_json::Value::String(mime));
    }
    object.insert(
        FILE_KEY_CONTENT.to_string(),
        file_content_to_json(file.content),
    );
    serde_json::Value::Object(object)
}

fn file_content_to_json(content: FileContent) -> serde_json::Value {
    let mut object = serde_json::Map::new();
    match content {
        FileContent::Bytes(bytes) => {
            object.insert(
                FILE_CONTENT_KEY_KIND.to_string(),
                serde_json::Value::String(FILE_CONTENT_KIND_BYTES.to_string()),
            );
            object.insert(
                FILE_CONTENT_KEY_BYTES.to_string(),
                serde_json::Value::String(STANDARD.encode(bytes)),
            );
        }
        FileContent::Directory(entries) => {
            object.insert(
                FILE_CONTENT_KEY_KIND.to_string(),
                serde_json::Value::String(FILE_CONTENT_KIND_DIRECTORY.to_string()),
            );
            object.insert(
                FILE_CONTENT_KEY_ENTRIES.to_string(),
                serde_json::Value::Array(entries.into_iter().map(file_value_to_json).collect()),
            );
        }
    }
    serde_json::Value::Object(object)
}

#[cfg(all(test, feature = "serde"))]
#[path = "file_value_tests.rs"]
mod tests;
