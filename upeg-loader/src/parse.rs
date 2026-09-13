use std::collections::{BTreeMap, VecDeque};

use crate::dispatcher::external::color::{COLOR_POLICIES, ColorPolicy};
use crate::dispatcher::external::pty::HOST_SUPPORTS_PTY;
use crate::invoker_metadata::RUNTIME_INVOKERS;
use crate::{LoadError, ToolEntryToml, ToolToml, ToolkitToml, model::chain_step_key};
use upeg_core::{
    InputFieldSpec, InputSpec, Invoker, OutputFieldSpec, OutputSpec, PegboardUnits, PinKind,
    Surface, ToolId, ToolIdError, ToolMeta, ToolkitMeta, validate_primary_output_id,
};
use upeg_runtime::manifest::{
    RuntimeToolManifest, lower_runtime_tool_manifest, validate_tool_identity,
};
use upeg_runtime::pegboard_project::ProjectBoardDecl;

pub(crate) mod boards;
pub(crate) mod chain;
pub(crate) mod controlled_embed;
pub(crate) mod templates;
pub(crate) mod triggers;

use controlled_embed::{
    reject_flat_controlled_embed_browser_fields, reject_unknown_binding_wait_keys,
    validate_controlled_embed_settings, validate_selector_bindings,
};

/// One toolkit manifest lowered for a caller outside the loader, with
/// *both* halves of the verdict: the tools that will run on this host,
/// and the ones it had to drop ([`SkippedTool`]).
///
/// Exists because a bare tool count cannot tell "this manifest declares
/// two tools" from "this manifest declares three and one of them is
/// invisible here" — a report that shows only the first number leaves a
/// hole the operator has no way to name.
#[derive(Debug)]
pub struct ToolkitParse {
    pub toolkit: ToolkitMeta,
    pub tools: Vec<(ToolMeta, ToolToml)>,
    /// Empty on every machine that can honour everything the file
    /// declared — the common case, and the reason surfaces should only
    /// spend a line on it when it is non-empty.
    pub skipped: Vec<SkippedTool>,
}

/// Parse a toolkit manifest, keeping the host's skip verdict.
///
/// [`parse_toolkit_full`] is the same parse with `skipped` dropped; use
/// this one wherever the result is *reported* to an operator rather than
/// consumed as a tool list.
pub fn parse_toolkit_with_skips(input: &str) -> Result<ToolkitParse, LoadError> {
    let parsed = parse_manifest(input)?;
    Ok(ToolkitParse {
        toolkit: parsed.toolkit,
        tools: parsed.tools,
        skipped: parsed.skipped,
    })
}

pub fn parse_toolkit_full(
    input: &str,
) -> Result<(ToolkitMeta, Vec<(ToolMeta, ToolToml)>), LoadError> {
    let parsed = parse_toolkit_with_skips(input)?;
    Ok((parsed.toolkit, parsed.tools))
}

/// What this machine can offer a tool that asks for it.
///
/// Separate from validation on purpose. A manifest that declares
/// `pty = true` is *correct* — it is this host that may be unable to
/// honour it — so the answer belongs to the machine, not to the file,
/// and the two must not be confused: a wrong manifest is the author's
/// problem everywhere, while a missing capability is one operator's
/// problem on one box.
///
/// Carried as a value rather than read from `cfg!` at the point of use
/// so both answers are reachable from a test on any host; the skip
/// itself can still only happen where the capability is really missing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct HostCapabilities {
    /// Whether this build can open a pseudoterminal — see
    /// [`crate::dispatcher::external::pty`].
    pty: bool,
}

impl HostCapabilities {
    /// What the machine this build runs on can do.
    pub(crate) const CURRENT: Self = Self {
        pty: HOST_SUPPORTS_PTY,
    };

    #[cfg(test)]
    pub(crate) const fn with_pty(pty: bool) -> Self {
        Self { pty }
    }

    /// Why this host cannot run `tool`, if it cannot.
    ///
    /// `pty = false` says nothing the default did not already say, so
    /// only an explicit `true` can be refused.
    fn skip_reason(self, tool: &ToolToml) -> Option<LoadError> {
        (tool.pty == Some(true) && !self.pty).then_some(LoadError::PtyUnsupportedOnHost)
    }
}

/// A tool that parsed and validated cleanly and still will not run here.
///
/// Not a failure of the file: the declaration is legal, this machine
/// merely cannot honour it, so the tool drops out and every other tool
/// in the same manifest loads. The reason travels with the id so a
/// surface can say *which* tool went missing and *why* instead of
/// leaving a hole.
#[derive(Debug)]
pub struct SkippedTool {
    /// Canonical `<toolkit>.<tool>` id of the tool that dropped out.
    pub id: String,
    /// Why, as a [`LoadError`] rather than a reason enum of its own:
    /// what an operator reads, and what the troubleshooting catalogue
    /// indexes, must be the same sentence whether the condition costs a
    /// tool or a file.
    pub reason: LoadError,
}

/// One manifest's `[[tools]]` lowered, after this host has had its say.
#[derive(Debug)]
pub(crate) struct LoweredToolkit {
    pub(crate) toolkit: ToolkitMeta,
    pub(crate) tools: Vec<(ToolMeta, ToolToml)>,
    pub(crate) skipped: Vec<SkippedTool>,
}

/// A whole manifest file, lowered: the toolkit, its tools, and the
/// boards it declares.
///
/// `[[boards]]` is carried out separately from `parse_toolkit_full`'s
/// tuple because only *one* caller may honour it — the Project Manifest
/// loader. Every other caller (toolkit-directory loading, `upeg toolkit
/// validate`, example fixtures) keeps the tuple and cannot accidentally
/// register a project board.
#[derive(Debug)]
pub(crate) struct ParsedManifest {
    pub(crate) toolkit: ToolkitMeta,
    pub(crate) tools: Vec<(ToolMeta, ToolToml)>,
    pub(crate) boards: Vec<ProjectBoardDecl>,
    /// Tools this host cannot run — see [`SkippedTool`]. Empty on every
    /// machine that can honour everything the file declared.
    pub(crate) skipped: Vec<SkippedTool>,
}

pub(crate) fn parse_manifest(input: &str) -> Result<ParsedManifest, LoadError> {
    reject_retired_fields(input)?;
    let parsed: ToolkitToml = toml::from_str(input).map_err(LoadError::Toml)?;
    let boards = boards::lower_board_entries(&parsed.boards)?;
    let lowered = toolkit_to_meta_and_tools(&parsed, HostCapabilities::CURRENT)?;
    Ok(ParsedManifest {
        toolkit: lowered.toolkit,
        tools: lowered.tools,
        boards,
        skipped: lowered.skipped,
    })
}

fn reject_retired_fields(input: &str) -> Result<(), LoadError> {
    let value: toml::Value = toml::from_str(input).map_err(LoadError::Toml)?;
    let Some(table) = value.as_table() else {
        return Ok(());
    };
    for field in ["category", "cat"] {
        if table.contains_key(field) {
            return Err(LoadError::RetiredField {
                field: field.to_string(),
                scope: "Toolkit",
                replacement: "tags",
            });
        }
    }
    if let Some(tools) = table.get("tools").and_then(toml::Value::as_array) {
        for tool in tools {
            let Some(tool_table) = tool.as_table() else {
                continue;
            };
            for field in ["category", "cat"] {
                if tool_table.contains_key(field) {
                    return Err(LoadError::RetiredField {
                        field: field.to_string(),
                        scope: "Tool",
                        replacement: "tags",
                    });
                }
            }
            reject_flat_controlled_embed_browser_fields(tool_table)?;
            reject_unknown_binding_wait_keys(tool_table)?;
            reject_literal_credential_fields(tool_table)?;
        }
    }
    Ok(())
}

fn reject_literal_credential_fields(
    tool_table: &toml::map::Map<String, toml::Value>,
) -> Result<(), LoadError> {
    let Some(credentials) = tool_table
        .get("credentials")
        .and_then(toml::Value::as_array)
    else {
        return Ok(());
    };
    for (position, credential) in credentials.iter().enumerate() {
        let Some(credential_table) = credential.as_table() else {
            continue;
        };
        for field in ["value", "secret_value", "literal_secret"] {
            if credential_table.contains_key(field) {
                return Err(LoadError::SecretField {
                    position,
                    field: field.to_string(),
                });
            }
        }
    }
    Ok(())
}

pub(crate) fn toolkit_to_meta_and_tools(
    parsed: &ToolkitToml,
    host: HostCapabilities,
) -> Result<LoweredToolkit, LoadError> {
    let toolkit_id = parsed.id.trim();
    if toolkit_id.is_empty() {
        return Err(LoadError::EmptyToolkit);
    }
    if parsed.id != toolkit_id {
        return Err(LoadError::NonCanonicalToolkit(parsed.id.clone()));
    }
    if parsed.tools.is_empty() {
        return Err(LoadError::EmptyToolkitTools);
    }
    validate_tags(parsed.tags.as_deref())?;

    let toolkit_tags: Vec<String> = parsed
        .tags
        .clone()
        .unwrap_or_default()
        .into_iter()
        .collect();
    let toolkit_meta = ToolkitMeta {
        id: leak_str(toolkit_id.to_string()),
        tags: leak_string_slice(toolkit_tags.clone()),
        description: parsed
            .description
            .clone()
            .map_or("", |description| leak_str(description.trim().to_string())),
    };

    let mut tools = Vec::with_capacity(parsed.tools.len());
    let mut skipped = Vec::new();
    for entry in &parsed.tools {
        let tool = resolve_tool(toolkit_id, &toolkit_tags, entry)?;
        // Validated *before* the host has its say, so an author gets the
        // same verdict on every machine: a tool with a typo is a load
        // error even on the host that would have skipped it anyway.
        let meta = toml_to_meta(&tool)?;
        if let Some(reason) = host.skip_reason(&tool) {
            skipped.push(SkippedTool {
                id: tool.id,
                reason,
            });
            continue;
        }
        tools.push((meta, tool));
    }
    Ok(LoweredToolkit {
        toolkit: toolkit_meta,
        tools,
        skipped,
    })
}

fn resolve_tool(
    toolkit: &str,
    toolkit_tags: &[String],
    entry: &ToolEntryToml,
) -> Result<ToolToml, LoadError> {
    let local_id = entry.id.trim();
    if local_id.is_empty() {
        return Err(LoadError::EmptyId);
    }
    if entry.id != local_id {
        return Err(LoadError::NonCanonicalId(entry.id.clone()));
    }
    if local_id
        .strip_prefix(toolkit)
        .is_some_and(|rest| rest.starts_with('.'))
    {
        return Err(LoadError::ToolIdContainsToolkit {
            id: local_id.to_string(),
        });
    }
    validate_tags(entry.tags.as_deref())?;

    let mut effective_tags = toolkit_tags.to_vec();
    for tag in entry.tags.clone().unwrap_or_default() {
        if !effective_tags.iter().any(|existing| existing == &tag) {
            effective_tags.push(tag);
        }
    }

    Ok(ToolToml {
        id: format!("{toolkit}.{local_id}"),
        toolkit: toolkit.to_string(),
        tags: Some(effective_tags),
        display_label: entry.display_label.clone(),
        description: entry.description.clone(),
        inputs: entry.inputs.clone(),
        outputs: entry.outputs.clone(),
        primary_output_id: entry.primary_output_id.clone(),
        pin: entry.pin.clone(),
        pegboard_units: entry.pegboard_units.clone(),
        invoker: entry
            .invoker
            .clone()
            .or_else(|| entry.steps.as_ref().map(|_| "Chain".to_string())),
        surfaces: entry.surfaces.clone(),
        boards: entry.boards.clone(),
        command: entry.command.clone(),
        args_template: entry.args_template.clone(),
        cwd: entry.cwd.clone(),
        env: entry.env.clone(),
        timeout_ms: entry.timeout_ms,
        color: entry.color.clone(),
        pty: entry.pty,
        steps: entry.steps.clone(),
        connections: entry.connections.clone(),
        approval_surfaces: entry.approval_surfaces.clone(),
        output: entry.output.clone(),
        url: entry.url.clone(),
        method: entry.method.clone(),
        headers: entry.headers.clone(),
        body: entry.body.clone(),
        prompt: entry.prompt.clone(),
        provider: entry.provider.clone(),
        model: entry.model.clone(),
        base_url: entry.base_url.clone(),
        credential: entry.credential.clone(),
        credentials: entry.credentials.clone(),
        wasm_path: entry.wasm_path.clone(),
        triggers: entry.triggers.clone(),
        embed_url: entry.embed_url.clone(),
        controlled_embed: entry.controlled_embed.clone(),
    })
}

/// Iter 201: extract the iter-195/196/197 boundary-validation rules
/// out of `toml_to_meta` so each function tells one story. Returns
/// `Err(LoadError::Empty*)` for the first content-strict violation
/// found. Pure (no I/O, no leaks) so unit tests can target each rule
/// directly without going through the conversion path.
///
/// Validation order matches user mental model: identity first
/// (tool id/toolkit), then composition (chain), then sidecar collections
/// (boards, `controlled_embed.bindings`). Surface- and enum-level errors
/// (`UnknownInvoker` / `UnknownPinKind` / `UnknownSurface`) stay in the
/// conversion path because they're tied to upeg-core's `parse()` helpers;
/// missing invoker is a loader contract error and is caught here.
fn validate_toml(parsed: &ToolToml) -> Result<(), LoadError> {
    validate_identity(parsed)?;
    validate_steps(parsed)?;
    validate_steps_invoker_conflict(parsed)?;
    chain::validate_chain_contract(parsed)?;
    validate_boards(parsed)?;
    validate_selector_bindings(parsed)?;
    triggers::validate_triggers(parsed)?;
    validate_credentials(parsed)?;
    validate_present_enum_fields(parsed)?;
    validate_surfaces(parsed)?;
    validate_embed_url(parsed)?;
    validate_controlled_embed_settings(parsed)?;
    validate_required_invoker_fields(parsed)?;
    validate_external_process_fields(parsed)?;
    templates::validate_arg_templates(parsed)?;
    Ok(())
}

/// `cwd`, `env`, `timeout_ms`, `color`, and `pty` describe a spawned
/// child process, so they only mean something for
/// `invoker = "External"`. Declaring them on an Http or Chain tool is a
/// manifest bug that would otherwise be silently ignored at dispatch
/// time.
fn validate_external_process_fields(parsed: &ToolToml) -> Result<(), LoadError> {
    const EXTERNAL_INVOKER: &str = "External";

    let declared: &[(bool, &'static str)] = &[
        (parsed.cwd.is_some(), "cwd"),
        (parsed.env.is_some(), "env"),
        (parsed.timeout_ms.is_some(), "timeout_ms"),
        (parsed.color.is_some(), "color"),
        (parsed.pty.is_some(), "pty"),
    ];
    if let Some(invoker) = parsed.invoker.as_deref().map(str::trim)
        && invoker != EXTERNAL_INVOKER
        && let Some((_, field)) = declared.iter().find(|(is_declared, _)| *is_declared)
    {
        return Err(LoadError::InvokerFieldConflict {
            invoker: invoker.to_string(),
            field,
            expected_invoker: EXTERNAL_INVOKER,
        });
    }
    if parsed.timeout_ms == Some(0) {
        return Err(LoadError::ZeroExternalTimeout);
    }
    for (position, entry) in parsed.env.as_deref().unwrap_or_default().iter().enumerate() {
        if entry.name.trim().is_empty() {
            return Err(LoadError::EmptyEnvName { position });
        }
    }
    // Parsed here rather than at dispatch time so a typo (`color =
    // "always"`) is a load error naming the accepted values instead of a
    // silently ignored declaration.
    ColorPolicy::parse_declaration(parsed.color.as_deref()).map_err(|error| {
        LoadError::UnknownExternalColor {
            value: error.value,
            allowed: COLOR_POLICIES.join(", "),
        }
    })?;
    // Whether this host can *open* a pseudoterminal is deliberately not
    // asked here. The declaration is legal everywhere; a machine that
    // cannot honour it skips the one tool
    // ([`HostCapabilities::skip_reason`]) instead of failing the file,
    // because the manifest's other tools have nothing to do with it.
    Ok(())
}

fn validate_identity(parsed: &ToolToml) -> Result<(), LoadError> {
    ToolId::parse_canonical_in_toolkit(&parsed.id, &parsed.toolkit)
        .map_err(|error| load_error_for_tool_id(error, &parsed.id, &parsed.toolkit))?;
    validate_tags(parsed.tags.as_deref())?;
    Ok(())
}

fn validate_tags(tags: Option<&[String]>) -> Result<(), LoadError> {
    let Some(tags) = tags else {
        return Ok(());
    };
    for (position, tag) in tags.iter().enumerate() {
        if tag.trim().is_empty() {
            return Err(LoadError::EmptyTag { position });
        }
        if tag != tag.trim() {
            return Err(LoadError::NonCanonicalTag {
                position,
                tag: tag.clone(),
            });
        }
    }
    Ok(())
}

fn load_error_for_tool_id(error: ToolIdError, id: &str, toolkit: &str) -> LoadError {
    match error {
        ToolIdError::EmptyId | ToolIdError::EmptyLocal => LoadError::EmptyId,
        ToolIdError::EmptyToolkit => LoadError::EmptyToolkit,
        ToolIdError::NonCanonicalId { id } => LoadError::NonCanonicalId(id),
        ToolIdError::NonCanonicalToolkit { toolkit } => LoadError::NonCanonicalToolkit(toolkit),
        ToolIdError::MissingSeparator => LoadError::ToolkitMismatch {
            id: id.to_string(),
            toolkit: toolkit.to_string(),
        },
        ToolIdError::ToolkitMismatch { id, toolkit } => LoadError::ToolkitMismatch { id, toolkit },
    }
}

fn validate_steps(parsed: &ToolToml) -> Result<(), LoadError> {
    let Some(steps) = &parsed.steps else {
        if parsed.connections.is_some() {
            return Err(LoadError::EmptyChain);
        }
        return Ok(());
    };
    if steps.is_empty() {
        return Err(LoadError::EmptyChain);
    }
    let mut step_positions = BTreeMap::new();
    for (position, step) in steps.iter().enumerate() {
        if step.tool.trim().is_empty() || step.id.as_deref().is_some_and(|s| s.trim().is_empty()) {
            return Err(LoadError::EmptyChainStep { position });
        }
        let id = chain_step_key(position, step.id.as_deref());
        if step_positions.insert(id.clone(), position).is_some() {
            return Err(LoadError::DuplicateChainStepId { position, id });
        }
    }
    validate_chain_connections(parsed, &step_positions)?;
    Ok(())
}

fn validate_chain_connections(
    parsed: &ToolToml,
    step_positions: &BTreeMap<String, usize>,
) -> Result<(), LoadError> {
    let Some(connections) = &parsed.connections else {
        return Ok(());
    };
    let mut outgoing = vec![Vec::new(); step_positions.len()];
    let mut incoming_counts = vec![0_usize; step_positions.len()];
    for (position, connection) in connections.iter().enumerate() {
        let from = connection.from.trim();
        let to = connection.to.trim();
        if from.is_empty() {
            return Err(LoadError::EmptyChainConnection {
                position,
                field: "from",
            });
        }
        if to.is_empty() {
            return Err(LoadError::EmptyChainConnection {
                position,
                field: "to",
            });
        }
        let Some(&from_index) = step_positions.get(from) else {
            return Err(LoadError::UnknownChainConnectionStep {
                position,
                step: from.to_string(),
            });
        };
        let Some(&to_index) = step_positions.get(to) else {
            return Err(LoadError::UnknownChainConnectionStep {
                position,
                step: to.to_string(),
            });
        };
        outgoing[from_index].push(to_index);
        incoming_counts[to_index] += 1;
    }
    reject_chain_connection_cycle(outgoing, incoming_counts)
}

fn reject_chain_connection_cycle(
    outgoing: Vec<Vec<usize>>,
    mut incoming_counts: Vec<usize>,
) -> Result<(), LoadError> {
    let mut ready = incoming_counts
        .iter()
        .enumerate()
        .filter_map(|(index, count)| (*count == 0).then_some(index))
        .collect::<VecDeque<_>>();
    let mut visited = 0_usize;
    while let Some(index) = ready.pop_front() {
        visited += 1;
        for &next in &outgoing[index] {
            incoming_counts[next] -= 1;
            if incoming_counts[next] == 0 {
                ready.push_back(next);
            }
        }
    }
    if visited == incoming_counts.len() {
        Ok(())
    } else {
        Err(LoadError::ChainConnectionCycle)
    }
}

fn validate_steps_invoker_conflict(parsed: &ToolToml) -> Result<(), LoadError> {
    if (parsed.steps.is_some() || parsed.connections.is_some())
        && let Some(invoker) = parsed.invoker.as_deref()
        && matches!(
            invoker,
            "Function" | "External" | "Http" | "Embed" | "Llm" | "Wasm"
        )
    {
        return Err(LoadError::InvokerFieldConflict {
            invoker: invoker.to_string(),
            field: "steps",
            expected_invoker: "Chain",
        });
    }
    Ok(())
}

fn validate_boards(parsed: &ToolToml) -> Result<(), LoadError> {
    let Some(boards) = &parsed.boards else {
        return Ok(());
    };
    for (position, board) in boards.iter().enumerate() {
        if board.trim().is_empty() {
            return Err(LoadError::EmptyBoard { position });
        }
        if board != board.trim() {
            return Err(LoadError::NonCanonicalBoard {
                position,
                board: board.clone(),
            });
        }
    }
    Ok(())
}

fn validate_credentials(parsed: &ToolToml) -> Result<(), LoadError> {
    let Some(credentials) = &parsed.credentials else {
        return Ok(());
    };
    for (position, credential) in credentials.iter().enumerate() {
        validate_credential_name(position, credential)?;
        validate_credential_store(position, credential)?;
    }
    Ok(())
}

fn validate_credential_name(
    position: usize,
    credential: &crate::CredentialRefToml,
) -> Result<(), LoadError> {
    if credential.name.trim().is_empty() {
        return Err(LoadError::EmptyCredentialField {
            position,
            field: "name",
        });
    }
    if credential
        .value_type
        .as_deref()
        .is_some_and(|s| s.trim().is_empty())
    {
        return Err(LoadError::EmptyCredentialField {
            position,
            field: "type",
        });
    }
    Ok(())
}

fn validate_credential_store(
    position: usize,
    credential: &crate::CredentialRefToml,
) -> Result<(), LoadError> {
    let store = credential
        .store
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("env");
    match store {
        "env" => {
            if credential
                .env
                .as_deref()
                .is_some_and(|s| s.trim().is_empty())
            {
                return Err(LoadError::EmptyCredentialField {
                    position,
                    field: "env",
                });
            }
        }
        "keychain" => {
            if credential
                .keychain_service
                .as_deref()
                .is_none_or(|s| s.trim().is_empty())
            {
                return Err(LoadError::EmptyCredentialField {
                    position,
                    field: "keychain_service",
                });
            }
            if credential
                .keychain_account
                .as_deref()
                .is_none_or(|s| s.trim().is_empty())
            {
                return Err(LoadError::EmptyCredentialField {
                    position,
                    field: "keychain_account",
                });
            }
        }
        other => {
            return Err(LoadError::UnknownCredentialStore {
                position,
                store: other.to_string(),
            });
        }
    }
    Ok(())
}

fn validate_present_enum_fields(parsed: &ToolToml) -> Result<(), LoadError> {
    if parsed
        .invoker
        .as_deref()
        .is_some_and(|s| s.trim().is_empty())
    {
        return Err(LoadError::EmptyInvoker);
    }
    if parsed.pin.as_deref().is_some_and(|s| s.trim().is_empty()) {
        return Err(LoadError::EmptyPinKind);
    }
    match parsed.pegboard_units.as_deref() {
        None => return Err(LoadError::MissingPegboardUnits),
        Some(s) if s.trim().is_empty() => return Err(LoadError::EmptyPegboardUnits),
        Some(_) => {}
    }
    Ok(())
}

fn validate_surfaces(parsed: &ToolToml) -> Result<(), LoadError> {
    if let Some(surfaces) = &parsed.surfaces
        && let Some(position) = surfaces.iter().position(|s| s.trim().is_empty())
    {
        return Err(LoadError::EmptyInSurfaces { position });
    }
    Ok(())
}

fn validate_embed_url(parsed: &ToolToml) -> Result<(), LoadError> {
    if parsed
        .embed_url
        .as_deref()
        .is_some_and(|s| s.trim().is_empty())
    {
        return Err(LoadError::EmptyEmbedUrl);
    }
    Ok(())
}

fn validate_required_invoker_fields(parsed: &ToolToml) -> Result<(), LoadError> {
    let Some(invoker) = parsed.invoker.as_deref() else {
        return Err(LoadError::MissingInvoker);
    };
    if invoker == "Function" {
        return Err(LoadError::UnsupportedRuntimeInvoker {
            invoker: "Function",
        });
    }
    let Some(spec) = RUNTIME_INVOKERS.iter().find(|s| s.name == invoker) else {
        return Ok(());
    };
    for &field in spec.required_fields {
        if is_invoker_field_missing(parsed, field) {
            return Err(invoker_missing_field_error(spec.name, field));
        }
    }
    Ok(())
}

fn is_invoker_field_missing(parsed: &ToolToml, field: &'static str) -> bool {
    match field {
        "command" => missing_string(&parsed.command),
        "url" => missing_string(&parsed.url),
        "prompt" => missing_string(&parsed.prompt),
        "wasm_path" => missing_string(&parsed.wasm_path),
        "steps" => parsed.steps.is_none(),
        _ => true,
    }
}

fn invoker_missing_field_error(invoker: &'static str, field: &'static str) -> LoadError {
    match (invoker, field) {
        ("External", "command") => LoadError::ExternalRequiresCommand,
        ("Chain", "steps") => LoadError::EmptyChain,
        _ => LoadError::MissingInvokerField { invoker, field },
    }
}

fn missing_string(value: &Option<String>) -> bool {
    value.as_deref().is_none_or(|s| s.trim().is_empty())
}

/// Convert an already-parsed `ToolToml` into a `ToolMeta`. Same leak
/// semantics as `parse_str`. Pulled out so `load_and_register_dir` can
/// keep the original `ToolToml` around to build an External dispatcher
/// without re-parsing the file.
pub(crate) fn toml_to_meta(parsed: &ToolToml) -> Result<ToolMeta, LoadError> {
    let identity = ToolId::parse_canonical_in_toolkit(&parsed.id, &parsed.toolkit)
        .map_err(|error| load_error_for_tool_id(error, &parsed.id, &parsed.toolkit))?;
    validate_tool_identity(&identity.key(), &[])
        .map_err(upeg_runtime::manifest::ManifestError::from)?;

    // Iter 201: boundary-validation rules extracted into `validate_toml`.
    validate_toml(parsed)?;
    let input_spec = resolve_input_spec(parsed)?;
    let output_spec = resolve_output_spec(parsed)?;
    let invoker = resolve_invoker(parsed.invoker.as_deref())?;

    lower_runtime_tool_manifest(RuntimeToolManifest {
        id: parsed.id.clone(),
        toolkit: parsed.toolkit.clone(),
        tags: parsed.tags.clone().unwrap_or_default(),
        display_label: parsed.display_label.clone(),
        description: parsed.description.clone(),
        input_spec,
        output_spec,
        primary_output_id: parsed.primary_output_id.clone(),
        pin: resolve_pin(parsed.pin.as_deref())?,
        pegboard_units: resolve_pegboard_units(parsed.pegboard_units.as_deref())?,
        invoker,
        surfaces: parse_runtime_surfaces(parsed.surfaces.clone(), invoker)?,
        boards: parsed.boards.clone().unwrap_or_default(),
    })
    .map_err(LoadError::from)
}

fn resolve_input_spec(parsed: &ToolToml) -> Result<InputSpec, LoadError> {
    if parsed.inputs.is_empty() {
        return Ok(InputSpec::empty());
    }
    let fields = parsed
        .inputs
        .iter()
        .cloned()
        .map(InputFieldSpec::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(InputSpec::new(fields)?)
}

fn resolve_output_spec(parsed: &ToolToml) -> Result<OutputSpec, LoadError> {
    if parsed.outputs.is_empty() {
        validate_primary_output_id(&[], parsed.primary_output_id.as_deref())?;
        return Ok(OutputSpec::empty());
    }
    let fields = parsed
        .outputs
        .iter()
        .cloned()
        .map(OutputFieldSpec::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    let output_spec = OutputSpec::new(fields)?;
    validate_primary_output_id(&output_spec.fields, parsed.primary_output_id.as_deref())?;
    Ok(output_spec)
}

fn parse_runtime_surfaces(
    surfaces: Option<Vec<String>>,
    invoker: Invoker,
) -> Result<Vec<Surface>, LoadError> {
    match surfaces {
        None if invoker == Invoker::Embed => Ok(upeg_core::EMBED_SURFACES.to_vec()),
        None => Ok(upeg_core::ALL_SURFACES.to_vec()),
        Some(list) => list
            .into_iter()
            .enumerate()
            .map(|(position, surface)| {
                if surface.trim().is_empty() {
                    return Err(LoadError::EmptyInSurfaces { position });
                }
                Surface::parse(&surface).ok_or(LoadError::UnknownSurface(surface))
            })
            .collect(),
    }
}

fn resolve_pin(value: Option<&str>) -> Result<PinKind, LoadError> {
    if value.is_some_and(|s| s.trim().is_empty()) {
        return Err(LoadError::EmptyPinKind);
    }
    let value = value.unwrap_or("Inline");
    PinKind::parse(value).ok_or_else(|| LoadError::UnknownPinKind(value.to_string()))
}

fn resolve_pegboard_units(value: Option<&str>) -> Result<PegboardUnits, LoadError> {
    let Some(value) = value else {
        return Err(LoadError::MissingPegboardUnits);
    };
    if value.trim().is_empty() {
        return Err(LoadError::EmptyPegboardUnits);
    }
    PegboardUnits::parse(value).ok_or_else(|| LoadError::UnknownPegboardUnits(value.to_string()))
}

fn resolve_invoker(value: Option<&str>) -> Result<Invoker, LoadError> {
    let Some(value) = value else {
        return Err(LoadError::MissingInvoker);
    };
    if value.trim().is_empty() {
        return Err(LoadError::EmptyInvoker);
    }
    Invoker::parse(value).ok_or_else(|| LoadError::UnknownInvoker(value.to_string()))
}

fn leak_str(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn leak_string_slice(v: Vec<String>) -> &'static [&'static str] {
    let leaked_strs: Vec<&'static str> = v.into_iter().map(leak_str).collect();
    Box::leak(leaked_strs.into_boxed_slice())
}
