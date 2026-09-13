use upeg_core::{
    InputSpec, Invoker, OutputSpec, PegboardUnits, PinKind, PrimaryOutputIdError, StaticToolMeta,
    Surface, ToolId, ToolIdError, ToolKey, ToolMeta, validate_primary_output_id,
};

/// Source-neutral raw Tool manifest lowered by runtime adapters into [`ToolMeta`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalToolManifest {
    pub id: String,
    pub toolkit: String,
    pub tags: Vec<String>,
    pub display_label: Option<String>,
    pub description: Option<String>,
    pub input_spec: InputSpec,
    pub output_spec: OutputSpec,
    pub primary_output_id: Option<String>,
    pub pin: Option<String>,
    pub pegboard_units: Option<String>,
    pub invoker: Option<String>,
    pub surfaces: Option<Vec<String>>,
    pub boards: Vec<String>,
}

/// Canonical runtime-ready Tool manifest after source adapters have parsed and
/// validated source-specific schema syntax.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeToolManifest {
    pub id: String,
    pub toolkit: String,
    pub tags: Vec<String>,
    pub display_label: Option<String>,
    pub description: Option<String>,
    pub input_spec: InputSpec,
    pub output_spec: OutputSpec,
    pub primary_output_id: Option<String>,
    pub pin: PinKind,
    pub pegboard_units: PegboardUnits,
    pub invoker: Invoker,
    pub surfaces: Vec<Surface>,
    pub boards: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
/// Error raised when a runtime manifest collides with an existing tool id.
pub enum CollisionError {
    /// The manifest declares a tool id that is already registered at runtime.
    #[error("tool `{id}` duplicates an existing runtime tool")]
    DuplicateTool {
        id: String,
        toolkit: String,
        local_id: String,
    },
    /// The manifest declares a tool id that would hide a built-in tool.
    #[error("tool `{id}` shadows a built-in tool")]
    ShadowsBuiltIn {
        id: String,
        toolkit: String,
        local_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
/// Error raised while validating and lowering a source-neutral tool manifest.
pub enum ManifestError {
    /// Tool id parsing failed for the manifest identity.
    #[error("{source}")]
    ToolId {
        source: ToolIdError,
        id: String,
        toolkit: String,
    },
    /// The manifest id conflicts with an existing runtime or built-in tool.
    #[error(transparent)]
    Collision(#[from] CollisionError),
    /// A tag entry is empty after trimming.
    #[error("`tags[{position}]` is empty")]
    EmptyTag { position: usize },
    /// A tag entry is not canonical.
    #[error("`tags[{position}] = \"{tag}\"` must be canonical and unpadded")]
    NonCanonicalTag { position: usize, tag: String },
    /// Display label is present but empty.
    #[error("`display_label` is empty")]
    EmptyDisplayLabel,
    /// Pin kind is present but empty.
    #[error("`pin` is empty")]
    EmptyPinKind,
    /// Pin kind is not recognized by upeg.
    #[error("unknown pin `{0}`")]
    UnknownPinKind(String),
    /// Pegboard units are required so every UI surface shares one footprint.
    #[error("`pegboard_units` is required")]
    MissingPegboardUnits,
    /// Pegboard units are present but empty.
    #[error("`pegboard_units` is empty")]
    EmptyPegboardUnits,
    /// Pegboard units are not recognized by upeg.
    #[error("unknown pegboard_units `{0}`")]
    UnknownPegboardUnits(String),
    /// Invoker is present but empty.
    #[error("`invoker` is empty")]
    EmptyInvoker,
    /// Invoker is required for external manifests.
    #[error("`invoker` is required")]
    MissingInvoker,
    /// Invoker name is not recognized by upeg.
    #[error("unknown invoker `{0}`")]
    UnknownInvoker(String),
    /// A surface entry is empty after trimming.
    #[error("`surfaces[{position}]` is empty")]
    EmptyInSurfaces { position: usize },
    /// Surface name is not recognized by upeg.
    #[error("unknown surface `{0}`")]
    UnknownSurface(String),
    /// A board entry is empty after trimming.
    #[error("`boards[{position}]` is empty")]
    EmptyBoard { position: usize },
    /// A board entry is not canonical.
    #[error("`boards[{position}] = \"{board}\"` must be canonical and unpadded")]
    NonCanonicalBoard { position: usize, board: String },
    /// Primary output id is missing, unexpected, or does not name an output field.
    #[error(transparent)]
    PrimaryOutputId(#[from] PrimaryOutputIdError),
}

/// Lower a source-neutral manifest into the canonical runtime [`ToolMeta`].
pub fn lower_manifest_to_tool_meta(
    manifest: ExternalToolManifest,
) -> Result<ToolMeta, ManifestError> {
    let pin = parse_pin(manifest.pin.as_deref())?;
    let pegboard_units = parse_pegboard_units(manifest.pegboard_units.as_deref())?;
    let invoker = parse_invoker(manifest.invoker.as_deref())?;
    let surfaces = parse_surfaces(manifest.surfaces, invoker)?;

    lower_runtime_tool_manifest(RuntimeToolManifest {
        id: manifest.id,
        toolkit: manifest.toolkit,
        tags: manifest.tags,
        display_label: manifest.display_label,
        description: manifest.description,
        input_spec: manifest.input_spec,
        output_spec: manifest.output_spec,
        primary_output_id: manifest.primary_output_id,
        pin,
        pegboard_units,
        invoker,
        surfaces,
        boards: manifest.boards,
    })
}

/// Lower a runtime-ready manifest into the canonical runtime [`ToolMeta`] shape.
pub fn lower_runtime_tool_manifest(
    manifest: RuntimeToolManifest,
) -> Result<ToolMeta, ManifestError> {
    let identity =
        ToolId::parse_canonical_in_toolkit(&manifest.id, &manifest.toolkit).map_err(|source| {
            ManifestError::ToolId {
                source,
                id: manifest.id.clone(),
                toolkit: manifest.toolkit.clone(),
            }
        })?;
    validate_tool_identity(&identity.key(), &[])?;
    validate_tags(&manifest.tags)?;
    validate_boards(&manifest.boards)?;
    validate_primary_output_id(
        &manifest.output_spec.fields,
        manifest.primary_output_id.as_deref(),
    )?;

    let display_label = manifest
        .display_label
        .as_deref()
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .map_or_else(|| display_label_from_slug(identity.local()), str::to_string);

    let surfaces = manifest.surfaces;

    let local_id = identity.local().to_string();
    Ok(ToolMeta {
        id: leak_str(manifest.id),
        toolkit: leak_str(manifest.toolkit),
        local_id: leak_str(local_id),
        tags: leak_str_slice(manifest.tags),
        display_label: leak_str(display_label),
        description: manifest.description.map_or("", leak_str),
        input_spec: manifest.input_spec,
        output_spec: manifest.output_spec,
        primary_output_id: manifest.primary_output_id.map(leak_str),
        source: upeg_core::Source::UserInput,
        pin: manifest.pin,
        pegboard_units: manifest.pegboard_units,
        invoker: manifest.invoker,
        surfaces: Box::leak(surfaces.into_boxed_slice()),
        boards: leak_str_slice(manifest.boards),
    })
}

/// Derive a deterministic presentation label for runtime adapters when a Tool
/// source does not provide an explicit label.
pub fn display_label_from_slug(value: &str) -> String {
    let slug = value.rsplit_once('.').map_or(value, |(_, local)| local);
    slug.split(['_', '-', ' '])
        .filter(|part| !part.is_empty())
        .map(display_label_word)
        .collect::<Vec<_>>()
        .join(" ")
}

fn display_label_word(word: &str) -> String {
    match word.to_ascii_lowercase().as_str() {
        "api" => "API".to_string(),
        "base32" => "Base32".to_string(),
        "base64" => "Base64".to_string(),
        "crc32" => "CRC32".to_string(),
        "csv" => "CSV".to_string(),
        "html" => "HTML".to_string(),
        "id" => "ID".to_string(),
        "iso" => "ISO".to_string(),
        "json" => "JSON".to_string(),
        "md5" => "MD5".to_string(),
        "nanoid" => "NanoID".to_string(),
        "qr" => "QR".to_string(),
        "rgb" => "RGB".to_string(),
        "sha1" => "SHA-1".to_string(),
        "sha256" => "SHA-256".to_string(),
        "sha512" => "SHA-512".to_string(),
        "url" => "URL".to_string(),
        "uuid" => "UUID".to_string(),
        "wcag" => "WCAG".to_string(),
        lower => {
            let mut chars = lower.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect(),
                None => String::new(),
            }
        }
    }
}

/// Validate that a dynamic Tool key does not collide with runtime or built-in Tools.
pub fn validate_tool_identity(
    key: &ToolKey<'_>,
    existing: &[ToolMeta],
) -> Result<(), CollisionError> {
    let id = full_id_for_key(*key);
    if upeg_core::inventory::iter::<StaticToolMeta>()
        .inspect(|tool| tool.assert_valid())
        .any(|tool| tool.id == id || tool.key() == *key)
    {
        return Err(CollisionError::ShadowsBuiltIn {
            id,
            toolkit: key.toolkit().to_string(),
            local_id: key.local().to_string(),
        });
    }

    if existing
        .iter()
        .inspect(|tool| tool.assert_valid())
        .any(|tool| tool.id == id || tool.key() == *key)
    {
        return Err(CollisionError::DuplicateTool {
            id,
            toolkit: key.toolkit().to_string(),
            local_id: key.local().to_string(),
        });
    }

    Ok(())
}

fn validate_tags(tags: &[String]) -> Result<(), ManifestError> {
    for (position, tag) in tags.iter().enumerate() {
        if tag.trim().is_empty() {
            return Err(ManifestError::EmptyTag { position });
        }
        if tag != tag.trim() {
            return Err(ManifestError::NonCanonicalTag {
                position,
                tag: tag.clone(),
            });
        }
    }
    Ok(())
}

fn validate_boards(boards: &[String]) -> Result<(), ManifestError> {
    for (position, board) in boards.iter().enumerate() {
        if board.trim().is_empty() {
            return Err(ManifestError::EmptyBoard { position });
        }
        if board != board.trim() {
            return Err(ManifestError::NonCanonicalBoard {
                position,
                board: board.clone(),
            });
        }
    }
    Ok(())
}

fn parse_pin(value: Option<&str>) -> Result<PinKind, ManifestError> {
    if value.is_some_and(|s| s.trim().is_empty()) {
        return Err(ManifestError::EmptyPinKind);
    }
    let value = value.unwrap_or("Inline");
    PinKind::parse(value).ok_or_else(|| ManifestError::UnknownPinKind(value.to_string()))
}

fn parse_pegboard_units(value: Option<&str>) -> Result<PegboardUnits, ManifestError> {
    let Some(value) = value else {
        return Err(ManifestError::MissingPegboardUnits);
    };
    if value.trim().is_empty() {
        return Err(ManifestError::EmptyPegboardUnits);
    }
    PegboardUnits::parse(value)
        .ok_or_else(|| ManifestError::UnknownPegboardUnits(value.to_string()))
}

fn parse_invoker(value: Option<&str>) -> Result<Invoker, ManifestError> {
    let Some(value) = value else {
        return Err(ManifestError::MissingInvoker);
    };
    if value.trim().is_empty() {
        return Err(ManifestError::EmptyInvoker);
    }
    Invoker::parse(value).ok_or_else(|| ManifestError::UnknownInvoker(value.to_string()))
}

fn parse_surfaces(
    value: Option<Vec<String>>,
    invoker: Invoker,
) -> Result<Vec<Surface>, ManifestError> {
    match value {
        None if invoker == Invoker::Embed => Ok(upeg_core::EMBED_SURFACES.to_vec()),
        None => Ok(upeg_core::ALL_SURFACES.to_vec()),
        Some(list) => {
            if let Some(position) = list.iter().position(|surface| surface.trim().is_empty()) {
                return Err(ManifestError::EmptyInSurfaces { position });
            }
            list.into_iter()
                .map(|surface| {
                    Surface::parse(&surface).ok_or(ManifestError::UnknownSurface(surface))
                })
                .collect()
        }
    }
}

fn full_id_for_key(key: ToolKey<'_>) -> String {
    format!("{}.{}", key.toolkit(), key.local())
}

fn leak_str(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

fn leak_str_slice(values: Vec<String>) -> &'static [&'static str] {
    let leaked: Vec<&'static str> = values.into_iter().map(leak_str).collect();
    Box::leak(leaked.into_boxed_slice())
}

#[cfg(test)]
mod tests {
    use super::*;

    upeg_core::inventory::submit! {
        StaticToolMeta {
            id: "shadow.fixture",
            toolkit: "shadow",
            local_id: "fixture",
            tags: &[],
            display_label: "Shadow fixture",
            description: "test built-in collision fixture",
            input_spec: upeg_core::StaticInputSpec::empty(),
            output_spec: upeg_core::StaticOutputSpec::empty(),
            primary_output_id: None,
            source: upeg_core::StaticSource::UserInput,
            pin: PinKind::Inline,
            pegboard_units: PegboardUnits::U1,
            invoker: Invoker::Function,
            surfaces: upeg_core::ALL_SURFACES,
            boards: &[],
        }
    }

    fn manifest(id: &str, invoker: &str) -> ExternalToolManifest {
        ExternalToolManifest {
            id: id.to_string(),
            toolkit: id.split('.').next().unwrap_or(id).to_string(),
            tags: vec!["pure".to_string(), "test".to_string()],
            display_label: None,
            description: Some("test tool".to_string()),
            input_spec: InputSpec::try_from(&serde_json::json!({
                "type": "object",
                "properties": {"input": {"type": "string"}}
            }))
            .expect("테스트 입력 명세가 파싱되어야 한다"),
            output_spec: OutputSpec::empty(),
            primary_output_id: None,
            pin: Some("Launcher".to_string()),
            pegboard_units: Some("U2".to_string()),
            invoker: Some(invoker.to_string()),
            surfaces: Some(vec!["cli".to_string(), "mcp".to_string()]),
            boards: vec!["main".to_string()],
        }
    }

    #[test]
    fn 동등한_외부_manifest는_같은_필드로_낮춰진다() {
        let toml_meta = lower_manifest_to_tool_meta(manifest("rtmanifest.echo", "External"))
            .expect("TOML 형태 매니페스트가 낮춰져야 한다");
        let wasm_meta = lower_manifest_to_tool_meta(manifest("rtmanifest.echo", "External"))
            .expect("WASM 형태 매니페스트가 낮춰져야 한다");

        assert_eq!(toml_meta.id, wasm_meta.id);
        assert_eq!(toml_meta.toolkit, wasm_meta.toolkit);
        assert_eq!(toml_meta.local_id, wasm_meta.local_id);
        assert_eq!(toml_meta.tags, wasm_meta.tags);
        assert_eq!(toml_meta.display_label, wasm_meta.display_label);
        assert_eq!(toml_meta.description, wasm_meta.description);
        assert_eq!(toml_meta.input_spec, wasm_meta.input_spec);
        assert_eq!(toml_meta.pin, wasm_meta.pin);
        assert_eq!(toml_meta.pegboard_units, wasm_meta.pegboard_units);
        assert_eq!(toml_meta.invoker, wasm_meta.invoker);
        assert_eq!(toml_meta.surfaces, wasm_meta.surfaces);
        assert_eq!(toml_meta.boards, wasm_meta.boards);
    }

    #[test]
    fn 중복과_가리기는_타입_있는_충돌_오류를_반환한다() {
        let existing = lower_manifest_to_tool_meta(manifest("rtmanifest.duplicate", "External"))
            .expect("시드 메타데이터");
        let duplicate_key = ToolKey::parse_canonical("rtmanifest", "duplicate").unwrap();
        assert!(matches!(
            validate_tool_identity(&duplicate_key, &[existing]),
            Err(CollisionError::DuplicateTool { id, .. }) if id == "rtmanifest.duplicate"
        ));

        let builtin_key = ToolKey::parse_canonical("shadow", "fixture").unwrap();
        assert!(matches!(
            validate_tool_identity(&builtin_key, &[]),
            Err(CollisionError::ShadowsBuiltIn { id, .. }) if id == "shadow.fixture"
        ));
    }
}
