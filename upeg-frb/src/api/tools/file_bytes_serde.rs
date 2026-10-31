//! Compact persisted file bytes. The shape uses the same padded standard
//! base64 as `upeg_core::FileContent`; old numeric-array rows still decode.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Deserializer, Serializer};

pub(super) fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&STANDARD.encode(bytes))
}

pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StoredBytes {
        Base64(String),
        Legacy(Vec<u8>),
    }
    match StoredBytes::deserialize(deserializer)? {
        StoredBytes::Base64(encoded) => STANDARD.decode(encoded).map_err(serde::de::Error::custom),
        StoredBytes::Legacy(bytes) => Ok(bytes),
    }
}
