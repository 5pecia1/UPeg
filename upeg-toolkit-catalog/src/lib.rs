//! Metadata and artifact contract for independently shipped toolkits.
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::get_unwrap,
        reason = "tests use fixture assertions"
    )
)]

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use upeg_core::{
    InputSpec, Invoker, OutputSpec, PegboardUnits, PinKind, Source, Surface, ToolEffect, ToolMeta,
    ToolPresentation, ToolkitMeta,
};

/// Change this when the guest dispatch result wire or host API changes.
pub const GUEST_ABI: &str = include_str!("guest_abi.txt");
pub const SCHEMA_VERSION: u32 = 1;
pub const EMBEDDED_METADATA: &str = include_str!("metadata.snapshot.json");
pub const WEB_PACK_TOOLKITS: &[&str] = &[
    "color", "convert", "csv", "hash", "id", "media", "num", "qr", "security", "text", "time",
];
pub const NATIVE_PACK_TOOLKITS: &[&str] = &[
    "color",
    "convert",
    "csv",
    "devcontainer",
    "eth",
    "hash",
    "id",
    "media",
    "net",
    "num",
    "qr",
    "security",
    "text",
    "time",
    "weather",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetadataSnapshot {
    pub schema_version: u32,
    pub app_version: String,
    pub toolkits: Vec<ToolkitMetadata>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolkitMetadata {
    pub id: String,
    pub tags: Vec<String>,
    pub description: String,
    pub tools: Vec<ToolMetadata>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolMetadata {
    pub id: String,
    pub toolkit: String,
    pub local_id: String,
    pub tags: Vec<String>,
    pub display_label: String,
    pub description: String,
    pub input_spec: InputSpec,
    pub output_spec: OutputSpec,
    pub primary_output_id: Option<String>,
    pub effect: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation: Option<ToolPresentation>,
    pub source: Source,
    pub pin: PinKind,
    pub pegboard_units: PegboardUnits,
    pub invoker: Invoker,
    pub surfaces: Vec<Surface>,
    pub boards: Vec<String>,
}

impl From<&ToolMeta> for ToolMetadata {
    fn from(meta: &ToolMeta) -> Self {
        Self {
            id: meta.id.to_owned(),
            toolkit: meta.toolkit.to_owned(),
            local_id: meta.local_id.to_owned(),
            tags: meta.tags.iter().map(|value| (*value).to_owned()).collect(),
            display_label: meta.display_label.to_owned(),
            description: meta.description.to_owned(),
            input_spec: meta.input_spec.clone(),
            output_spec: meta.output_spec.clone(),
            primary_output_id: meta.primary_output_id.map(str::to_owned),
            effect: match meta.effect {
                ToolEffect::Read => "read",
                ToolEffect::Write => "write",
                ToolEffect::Unknown => "unknown",
            }
            .to_owned(),
            presentation: meta.presentation.clone(),
            source: meta.source.clone(),
            pin: meta.pin,
            pegboard_units: meta.pegboard_units,
            invoker: meta.invoker,
            surfaces: meta.surfaces.to_vec(),
            boards: meta
                .boards
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WebArtifact {
    pub js: String,
    pub js_sha256: String,
    pub js_size: u64,
    pub wasm: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeArtifact {
    pub url: String,
    pub sha256: String,
    pub size: u64,
    pub encoding: String,
    pub expanded_sha256: String,
    pub expanded_size: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolkitPackage {
    #[serde(flatten)]
    pub metadata: ToolkitMetadata,
    pub version: String,
    pub web: Option<WebArtifact>,
    pub native: BTreeMap<String, NativeArtifact>,
    pub requires_host: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Catalog {
    pub schema_version: u32,
    pub app_version: String,
    pub abi_digest: String,
    pub catalog_digest: String,
    pub toolkits: Vec<ToolkitPackage>,
}

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("invalid catalog JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported schema version {0}")]
    Schema(u32),
    #[error("catalog app version {actual} does not match {expected}")]
    AppVersion { actual: String, expected: String },
    #[error("catalog digest mismatch")]
    Digest,
    #[error("catalog ABI digest mismatch")]
    Abi,
    #[error("toolkit {id} version {actual} does not match app version {expected}")]
    ToolkitVersion {
        id: String,
        actual: String,
        expected: String,
    },
    #[error("invalid tool metadata effect: {0}")]
    Effect(String),
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Hash the metadata and the explicit result ABI token. Artifact bytes do not
/// affect this value; different previews with compatible metadata share it.
pub fn abi_digest(metadata: &MetadataSnapshot) -> Result<String, serde_json::Error> {
    let bytes = serde_jcs::to_vec(&(GUEST_ABI, metadata))?;
    Ok(sha256_hex(&bytes))
}

impl Catalog {
    pub fn metadata_snapshot(&self) -> MetadataSnapshot {
        MetadataSnapshot {
            schema_version: self.schema_version,
            app_version: self.app_version.clone(),
            toolkits: self
                .toolkits
                .iter()
                .map(|entry| entry.metadata.clone())
                .collect(),
        }
    }

    pub fn digest_without_self(&self) -> Result<String, serde_json::Error> {
        let mut value = serde_json::to_value(self)?;
        if let serde_json::Value::Object(ref mut map) = value {
            map.remove("catalog_digest");
        }
        Ok(sha256_hex(&serde_jcs::to_vec(&value)?))
    }

    pub fn validate(&self, expected_abi: &str) -> Result<(), CatalogError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(CatalogError::Schema(self.schema_version));
        }
        if self.app_version != env!("CARGO_PKG_VERSION") {
            return Err(CatalogError::AppVersion {
                actual: self.app_version.clone(),
                expected: env!("CARGO_PKG_VERSION").to_owned(),
            });
        }
        for toolkit in &self.toolkits {
            if toolkit.version != self.app_version {
                return Err(CatalogError::ToolkitVersion {
                    id: toolkit.metadata.id.clone(),
                    actual: toolkit.version.clone(),
                    expected: self.app_version.clone(),
                });
            }
        }
        if self.abi_digest != expected_abi {
            return Err(CatalogError::Abi);
        }
        if abi_digest(&self.metadata_snapshot())? != self.abi_digest {
            return Err(CatalogError::Abi);
        }
        if self.digest_without_self()? != self.catalog_digest {
            return Err(CatalogError::Digest);
        }
        Ok(())
    }
}

fn leak_str(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

fn leak_strings(values: Vec<String>) -> &'static [&'static str] {
    Box::leak(
        values
            .into_iter()
            .map(leak_str)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    )
}

impl ToolMetadata {
    fn into_meta(self) -> Result<ToolMeta, CatalogError> {
        let effect = match self.effect.as_str() {
            "read" => ToolEffect::Read,
            "write" => ToolEffect::Write,
            "unknown" => ToolEffect::Unknown,
            _ => return Err(CatalogError::Effect(self.effect)),
        };
        Ok(ToolMeta {
            id: leak_str(self.id),
            toolkit: leak_str(self.toolkit),
            local_id: leak_str(self.local_id),
            tags: leak_strings(self.tags),
            display_label: leak_str(self.display_label),
            description: leak_str(self.description),
            input_spec: self.input_spec,
            output_spec: self.output_spec,
            primary_output_id: self.primary_output_id.map(leak_str),
            effect,
            presentation: self.presentation,
            source: self.source,
            pin: self.pin,
            pegboard_units: self.pegboard_units,
            invoker: self.invoker,
            surfaces: Box::leak(self.surfaces.into_boxed_slice()),
            boards: leak_strings(self.boards),
        })
    }
}

static REGISTERED: OnceLock<Result<(), String>> = OnceLock::new();
#[cfg(target_arch = "wasm32")]
static WEB_CAPABILITIES_REGISTERED: OnceLock<Result<(), String>> = OnceLock::new();

/// Register the generated metadata, without linking any toolkit engine.
pub fn register_embedded_metadata() -> Result<(), String> {
    REGISTERED
        .get_or_init(|| {
            let snapshot: MetadataSnapshot =
                serde_json::from_str(EMBEDDED_METADATA).map_err(|error| error.to_string())?;
            if snapshot.schema_version != SCHEMA_VERSION {
                return Err(format!(
                    "unsupported metadata schema {}",
                    snapshot.schema_version
                ));
            }
            if snapshot.app_version != env!("CARGO_PKG_VERSION") {
                return Err(format!(
                    "metadata version {} is stale",
                    snapshot.app_version
                ));
            }
            for toolkit in snapshot.toolkits {
                upeg_runtime::toolbox_add_toolkit(ToolkitMeta {
                    id: leak_str(toolkit.id),
                    tags: leak_strings(toolkit.tags),
                    description: leak_str(toolkit.description),
                });
                for tool in toolkit.tools {
                    upeg_runtime::toolbox_add_builtin_catalog_tool(
                        tool.into_meta().map_err(|error| error.to_string())?,
                    );
                }
            }
            Ok(())
        })
        .clone()
}

/// Expose downloadable browser built-ins as runnable capabilities before
/// their guest code arrives. Dart routes those calls through its Worker.
pub fn register_web_capabilities() -> Result<(), String> {
    register_embedded_metadata()?;
    #[cfg(target_arch = "wasm32")]
    {
        WEB_CAPABILITIES_REGISTERED
            .get_or_init(|| {
                let snapshot: MetadataSnapshot =
                    serde_json::from_str(EMBEDDED_METADATA).map_err(|error| error.to_string())?;
                for toolkit in snapshot.toolkits {
                    if !WEB_PACK_TOOLKITS.contains(&toolkit.id.as_str()) {
                        continue;
                    }
                    for tool in toolkit.tools {
                        if tool.invoker != Invoker::Function {
                            continue;
                        }
                        let meta = upeg_runtime::toolbox_tool(&tool.id)
                            .ok_or_else(|| format!("missing browser builtin {}", tool.id))?;
                        upeg_runtime::register_runtime_dispatcher(meta.id, |_args| {
                            upeg_runtime::tool_failure(
                                "toolkit_worker_required",
                                "browser toolkit runs in the dedicated Worker",
                            )
                        });
                    }
                }
                Ok(())
            })
            .clone()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_catalog() -> Catalog {
        let snapshot: MetadataSnapshot = serde_json::from_str(EMBEDDED_METADATA).expect("snapshot");
        let digest = abi_digest(&snapshot).expect("ABI digest");
        let mut catalog = Catalog {
            schema_version: SCHEMA_VERSION,
            app_version: snapshot.app_version,
            abi_digest: digest,
            catalog_digest: String::new(),
            toolkits: snapshot
                .toolkits
                .into_iter()
                .map(|metadata| ToolkitPackage {
                    metadata,
                    version: env!("CARGO_PKG_VERSION").to_owned(),
                    web: None,
                    native: BTreeMap::new(),
                    requires_host: false,
                })
                .collect(),
        };
        catalog.catalog_digest = catalog.digest_without_self().expect("catalog digest");
        catalog
    }

    #[test]
    fn preserves_presentation_through_metadata_snapshot() {
        let snapshot: MetadataSnapshot = serde_json::from_str(EMBEDDED_METADATA).expect("snapshot");
        let mut tool = snapshot.toolkits[0].tools[0].clone();
        let mut presentation: ToolPresentation = serde_json::from_value(serde_json::json!({
            "version": 1,
            "output": "result",
            "rows": "/rows",
            "row_key": "/id",
            "columns": [{"label": "Name", "pointer": "/name", "tone_pointer": null, "filterable": true}],
            "actions": [],
            "title_pointer": null,
            "subtitle_pointer": null,
            "status": null,
            "summary": [],
            "notices": null,
            "detail": null,
            "row_detail": null,
            "empty_message_pointer": null
        }))
        .expect("presentation");
        presentation.actions.push(upeg_core::PresentationAction {
            id: "inspect".into(),
            scope: upeg_core::ActionScope::Row,
            label: "Inspect".into(),
            target_tool: "sample.inspect".into(),
            on_success: Some(upeg_core::ActionSuccess::RefreshOrigin),
            bindings: BTreeMap::from([(
                "item".into(),
                upeg_core::ActionBinding::Row {
                    pointer: "/id".into(),
                },
            )]),
            enabled_pointer: Some("/enabled".into()),
            disabled_reason_pointer: Some("/reason".into()),
        });
        tool.presentation = Some(presentation.clone());
        let encoded = serde_json::to_string(&tool).expect("encoded metadata");
        let decoded: ToolMetadata = serde_json::from_str(&encoded).expect("decoded metadata");
        assert_eq!(
            decoded.into_meta().expect("tool metadata").presentation,
            Some(presentation)
        );
    }

    #[test]
    fn rejects_missing_or_duplicate_tool_metadata_even_with_fresh_catalog_digest() {
        let mut catalog = sample_catalog();
        let expected = catalog.abi_digest.clone();
        catalog
            .validate(&expected)
            .expect("original catalog is valid");
        let tool = catalog.toolkits[0].metadata.tools[0].clone();
        catalog.toolkits[0].metadata.tools.push(tool);
        catalog.catalog_digest = catalog.digest_without_self().expect("new catalog digest");
        assert!(matches!(
            catalog.validate(&expected),
            Err(CatalogError::Abi)
        ));

        let mut catalog = sample_catalog();
        catalog.toolkits[0].metadata.tools.pop();
        catalog.catalog_digest = catalog.digest_without_self().expect("new catalog digest");
        assert!(matches!(
            catalog.validate(&expected),
            Err(CatalogError::Abi)
        ));
    }

    #[test]
    fn detects_catalog_artifact_tampering() {
        let mut catalog = sample_catalog();
        let expected = catalog.abi_digest.clone();
        catalog.toolkits[0].requires_host = !catalog.toolkits[0].requires_host;
        assert!(matches!(
            catalog.validate(&expected),
            Err(CatalogError::Digest)
        ));
    }
}
