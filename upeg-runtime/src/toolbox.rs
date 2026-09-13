#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "toolbox lock poisoning is fatal (another thread already panicked); collision panics are honest assertions about programmer-facing invariants the tool macro should have already enforced"
)]

use std::sync::atomic::{AtomicU64, Ordering};
use upeg_core::{
    BoardExecutionContext, StaticToolMeta, Surface, ToolKey, ToolMeta, ToolResult, ToolkitMeta,
};

use crate::manifest::{CollisionError, validate_tool_identity};

#[derive(Clone, Copy)]
struct RuntimeToolEntry {
    meta: &'static ToolMeta,
    generation: u64,
}

static TOOLBOX_TOOLS: std::sync::OnceLock<std::sync::Mutex<Vec<RuntimeToolEntry>>> =
    std::sync::OnceLock::new();
static INVENTORY_TOOLS: std::sync::OnceLock<Vec<ToolMeta>> = std::sync::OnceLock::new();
static TOOLBOX_TOOLKITS: std::sync::OnceLock<std::sync::Mutex<Vec<&'static ToolkitMeta>>> =
    std::sync::OnceLock::new();
static BOARD_CONTEXTS: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<String, BoardExecutionContext>>,
> = std::sync::OnceLock::new();

static NEXT_RUNTIME_TOOL_GENERATION: AtomicU64 = AtomicU64::new(1);

fn runtime_tools_lock() -> &'static std::sync::Mutex<Vec<RuntimeToolEntry>> {
    TOOLBOX_TOOLS.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

fn inventory_tools() -> &'static [ToolMeta] {
    INVENTORY_TOOLS
        .get_or_init(|| {
            upeg_core::inventory::iter::<StaticToolMeta>()
                .map(|meta| {
                    meta.assert_valid();
                    ToolMeta::from_static(meta).expect("inventory StaticToolMeta must materialize")
                })
                .inspect(upeg_core::ToolMeta::assert_valid)
                .collect()
        })
        .as_slice()
}

fn runtime_toolkits_lock() -> &'static std::sync::Mutex<Vec<&'static ToolkitMeta>> {
    TOOLBOX_TOOLKITS.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

fn board_contexts_lock()
-> &'static std::sync::Mutex<std::collections::HashMap<String, BoardExecutionContext>> {
    BOARD_CONTEXTS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

fn push_tag(tags: &mut Vec<String>, tag: &str) {
    let tag = tag.trim();
    if tag.is_empty() || tags.iter().any(|existing| existing == tag) {
        return;
    }
    tags.push(tag.to_string());
}

pub trait ToolMetaRuntimeExt {
    fn tag_labels(&self) -> Vec<String>;
    fn has_tag(&self, tag: &str) -> bool;
    fn to_json_object(&self, id_key: &str) -> serde_json::Value;
    /// Does dispatching this Tool stop at a human-approval barrier?
    ///
    /// A UI asks this *before* dispatch so it can put the confirmation
    /// in front of the run instead of letting the run fail with
    /// `approval_required` and no affordance to answer it. Backed by
    /// [`crate::tool_approval_policy`]; false for every Tool without a
    /// gated Chain step.
    fn requires_approval(&self) -> bool;
    /// The effective set of surfaces whose approval this Tool honors —
    /// empty when it has no barrier. A UI compares its own surface
    /// against this to know whether its confirmation would count.
    fn approval_surfaces(&self) -> Vec<Surface>;
}

pub fn tool_json_entries_for_surface(surface: Surface, id_key: &str) -> Vec<serde_json::Value> {
    let mut tools: Vec<_> = toolbox_tools()
        .filter(|tool| tool.is_on_surface(surface))
        .collect();
    tools.sort_by_key(|tool| tool.id);
    tools
        .iter()
        .map(|tool| tool.to_json_object(id_key))
        .collect()
}

pub fn tools_list_json_for_surface(surface: Surface, id_key: &str) -> serde_json::Value {
    serde_json::json!({ "tools": tool_json_entries_for_surface(surface, id_key) })
}

impl ToolMetaRuntimeExt for ToolMeta {
    fn requires_approval(&self) -> bool {
        crate::tool_approval_policy(self.id).requires_approval()
    }

    fn approval_surfaces(&self) -> Vec<Surface> {
        crate::tool_approval_policy(self.id).surfaces().to_vec()
    }

    fn tag_labels(&self) -> Vec<String> {
        let mut tags = Vec::new();
        if let Some(toolkit) = toolbox_toolkit(self.toolkit_id()) {
            for tag in toolkit.tags {
                push_tag(&mut tags, tag);
            }
        }
        push_tag(&mut tags, self.toolkit_id());
        for tag in self.tags {
            push_tag(&mut tags, tag);
        }
        for tag in default_invoker_tags(self) {
            push_tag(&mut tags, tag);
        }
        for tag in default_pin_tags(self) {
            push_tag(&mut tags, tag);
        }
        tags
    }

    fn has_tag(&self, tag: &str) -> bool {
        let tag = tag.trim();
        !tag.is_empty() && self.tag_labels().iter().any(|t| t == tag)
    }

    fn to_json_object(&self, id_key: &str) -> serde_json::Value {
        let mut obj = serde_json::Map::new();
        obj.insert(id_key.into(), self.id.into());
        obj.insert("toolkit".into(), self.toolkit_id().into());
        obj.insert("tool".into(), self.tool_id().into());
        obj.insert(
            "tags".into(),
            serde_json::Value::Array(self.tag_labels().into_iter().map(Into::into).collect()),
        );
        obj.insert("displayLabel".into(), self.display_label.into());
        obj.insert("description".into(), self.description.into());
        obj.insert("inputSchema".into(), self.input_spec.to_json_schema_value());
        obj.insert(
            "outputSchema".into(),
            self.output_spec.to_json_schema_value(),
        );
        obj.insert("pin".into(), self.pin.label().into());
        obj.insert("pegboardUnits".into(), self.pegboard_units.label().into());
        let (cols, rows) = self.pegboard_units.grid_span();
        obj.insert(
            "pegboardSpan".into(),
            serde_json::json!({ "cols": cols, "rows": rows }),
        );
        obj.insert("invoker".into(), self.invoker.label().into());
        obj.insert(
            "surfaces".into(),
            serde_json::Value::Array(self.surface_labels().into_iter().map(Into::into).collect()),
        );
        obj.insert(
            "boards".into(),
            serde_json::Value::Array(self.boards.iter().map(|b| (*b).into()).collect()),
        );
        obj.insert(
            "source".into(),
            crate::tool_provenance(self.id).label().into(),
        );
        obj.insert(
            "embedUrl".into(),
            match crate::embed_url_for(self.id) {
                Some(u) => u.into(),
                None => serde_json::Value::Null,
            },
        );
        obj.insert(
            "selectorBindings".into(),
            serde_json::Value::Array(crate::selector_bindings_json_for(self.id)),
        );
        obj.insert(
            "triggers".into(),
            serde_json::Value::Array(crate::trigger_bindings_json_for(self.id)),
        );
        serde_json::Value::Object(obj)
    }
}

const fn default_invoker_tags(meta: &ToolMeta) -> &'static [&'static str] {
    match meta.invoker {
        upeg_core::Invoker::Function => &["pure"],
        upeg_core::Invoker::External => &["local", "external"],
        upeg_core::Invoker::Http => &["network"],
        // Static = no invocation, just a presentation. No network/local
        // tag because the tool itself doesn't reach out — the user
        // interacts with the rendered surface directly.
        upeg_core::Invoker::Static => &["static"],
        upeg_core::Invoker::Embed => &["network", "embed"],
        upeg_core::Invoker::Chain => &["chain"],
        upeg_core::Invoker::Llm => &["network", "llm"],
        upeg_core::Invoker::Wasm => &["wasm"],
    }
}

const fn default_pin_tags(meta: &ToolMeta) -> &'static [&'static str] {
    match meta.pin {
        upeg_core::PinKind::Chain => &["chain"],
        upeg_core::PinKind::Llm => &["llm"],
        upeg_core::PinKind::Embed => &["embed"],
        upeg_core::PinKind::ControlledEmbed => &["embed", "controlled"],
        _ => &[],
    }
}

pub fn toolbox_add_toolkit(meta: ToolkitMeta) {
    meta.assert_valid();
    let leaked: &'static ToolkitMeta = Box::leak(Box::new(meta));
    let mut guard = runtime_toolkits_lock().lock().expect("toolbox poisoned");
    if let Some(pos) = guard.iter().position(|t| t.id == leaked.id) {
        guard[pos] = leaked;
    } else {
        guard.push(leaked);
    }
}

pub fn toolbox_toolkits() -> impl Iterator<Item = &'static ToolkitMeta> {
    let runtime_snapshot: Vec<&'static ToolkitMeta> = runtime_toolkits_lock()
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_default();
    let runtime_ids: std::collections::HashSet<&'static str> =
        runtime_snapshot.iter().map(|toolkit| toolkit.id).collect();
    let mut toolkits = runtime_snapshot;
    toolkits.extend(
        upeg_core::inventory::iter::<ToolkitMeta>()
            .inspect(|meta| meta.assert_valid())
            .filter(|meta| !runtime_ids.contains(meta.id)),
    );
    toolkits.into_iter().inspect(|meta| meta.assert_valid())
}

pub fn toolbox_toolkit(id: &str) -> Option<&'static ToolkitMeta> {
    toolbox_toolkits().find(|t| t.id == id)
}

pub fn register_board_context(context: BoardExecutionContext) {
    board_contexts_lock()
        .lock()
        .expect("board context store poisoned")
        .insert(context.board.clone(), context);
}

pub fn board_context(board: &str) -> BoardExecutionContext {
    board_contexts_lock()
        .lock()
        .expect("board context store poisoned")
        .get(board)
        .cloned()
        .unwrap_or_else(|| BoardExecutionContext::new(board))
}

pub fn toolbox_add_tool(meta: ToolMeta) {
    let _ = insert_runtime_tool(meta);
}

#[derive(Debug)]
pub struct ToolboxRegistration {
    id: &'static str,
    generation: u64,
}

impl ToolboxRegistration {
    pub const fn id(&self) -> &'static str {
        self.id
    }
}

impl Drop for ToolboxRegistration {
    fn drop(&mut self) {
        let Ok(mut guard) = runtime_tools_lock().lock() else {
            return;
        };
        let before = guard.len();
        guard.retain(|entry| !(entry.meta.id == self.id && entry.generation == self.generation));
        if guard.len() != before {
            crate::execution_requirements::set_tool_execution_requirements(self.id, None);
        }
    }
}

pub fn toolbox_add_tool_managed(meta: ToolMeta) -> ToolboxRegistration {
    let (id, generation) = insert_runtime_tool(meta);
    ToolboxRegistration { id, generation }
}

fn insert_runtime_tool(meta: ToolMeta) -> (&'static str, u64) {
    meta.assert_valid();
    let key = meta.key();
    if let Err(error) = validate_tool_identity(&key, &[]) {
        panic_collision(error);
    }

    let mut guard = runtime_tools_lock().lock().expect("toolbox poisoned");
    let colliding_runtime_tools: Vec<ToolMeta> = guard
        .iter()
        .filter(|entry| entry.meta.key() != key)
        .map(|entry| entry.meta.clone())
        .collect();
    if let Err(error) = validate_tool_identity(&key, &colliding_runtime_tools) {
        drop(guard);
        panic_collision(error);
    }

    let leaked: &'static ToolMeta = Box::leak(Box::new(meta));
    let generation = NEXT_RUNTIME_TOOL_GENERATION.fetch_add(1, Ordering::Relaxed);
    let entry = RuntimeToolEntry {
        meta: leaked,
        generation,
    };
    if let Some(pos) = guard
        .iter()
        .position(|entry| entry.meta.key() == leaked.key())
    {
        guard[pos] = entry;
    } else {
        guard.push(entry);
    }
    // A replacement may come from any source or invoker. The registering
    // loader attaches fresh requirements after this insertion succeeds.
    crate::execution_requirements::set_tool_execution_requirements(leaked.id, None);
    (leaked.id, generation)
}

/// Validate whether a runtime Tool key can be registered without colliding
/// with built-in inventory or existing runtime tools.
pub fn validate_toolbox_addition(key: &ToolKey<'_>) -> Result<(), CollisionError> {
    let guard = runtime_tools_lock().lock().expect("toolbox poisoned");
    let colliding_runtime_tools: Vec<ToolMeta> = guard
        .iter()
        .filter(|entry| entry.meta.key() != *key)
        .map(|entry| entry.meta.clone())
        .collect();
    validate_tool_identity(key, &colliding_runtime_tools)
}

#[track_caller]
fn panic_collision(error: CollisionError) -> ! {
    match error {
        CollisionError::ShadowsBuiltIn { id, toolkit, .. } => panic!(
            "ToolMeta id `{id}` shadows a built-in tool and cannot be registered at runtime under Toolkit `{toolkit}`"
        ),
        CollisionError::DuplicateTool { id, toolkit, .. } => panic!(
            "ToolMeta id `{id}` is already registered and cannot also be registered under Toolkit `{toolkit}`"
        ),
    }
}

pub fn toolbox_add_tool_with_dispatcher<F>(meta: ToolMeta, f: F) -> &'static str
where
    F: for<'a> Fn(crate::DispatchArgs<'a>) -> ToolResult + Send + Sync + 'static,
{
    let id = meta.id;
    toolbox_add_tool(meta);
    crate::register_runtime_dispatcher(id, f);
    id
}

pub fn toolbox_add_single_text_tool_with_dispatcher<F>(meta: ToolMeta, f: F) -> &'static str
where
    F: for<'a> Fn(crate::DispatchArgs<'a>) -> Result<String, String> + Send + Sync + 'static,
{
    let id = meta.id;
    toolbox_add_tool(meta);
    crate::register_single_text_runtime_dispatcher(id, f);
    id
}

#[derive(Debug, Default)]
pub struct ToolboxRegistrationGroup {
    tools: Vec<ToolboxRegistration>,
    dispatchers: Vec<crate::RuntimeDispatcherRegistration>,
}

impl ToolboxRegistrationGroup {
    pub fn push_tool(&mut self, guard: ToolboxRegistration) {
        self.tools.push(guard);
    }

    pub fn push_dispatcher(&mut self, guard: crate::RuntimeDispatcherRegistration) {
        self.dispatchers.push(guard);
    }

    pub fn ids(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.tools.iter().map(ToolboxRegistration::id)
    }
}

pub fn toolbox_add_tool_with_dispatcher_managed<F>(meta: ToolMeta, f: F) -> ToolboxRegistrationGroup
where
    F: for<'a> Fn(crate::DispatchArgs<'a>) -> ToolResult + Send + Sync + 'static,
{
    let id = meta.id;
    let mut group = ToolboxRegistrationGroup::default();
    group.push_tool(toolbox_add_tool_managed(meta));
    group.push_dispatcher(crate::register_runtime_dispatcher_managed(id, f));
    group
}

pub fn toolbox_add_single_text_tool_with_dispatcher_managed<F>(
    meta: ToolMeta,
    f: F,
) -> ToolboxRegistrationGroup
where
    F: for<'a> Fn(crate::DispatchArgs<'a>) -> Result<String, String> + Send + Sync + 'static,
{
    let id = meta.id;
    let mut group = ToolboxRegistrationGroup::default();
    group.push_tool(toolbox_add_tool_managed(meta));
    group.push_dispatcher(crate::register_single_text_runtime_dispatcher_managed(
        id, f,
    ));
    group
}

pub fn toolbox_tools() -> impl Iterator<Item = &'static ToolMeta> {
    let runtime_snapshot: Vec<&'static ToolMeta> = runtime_tools_lock()
        .lock()
        .map(|guard| guard.iter().map(|entry| entry.meta).collect())
        .unwrap_or_default();
    inventory_tools()
        .iter()
        .chain(runtime_snapshot)
        .inspect(|meta| meta.assert_valid())
}

pub fn toolbox_tool(id: &str) -> Option<&'static ToolMeta> {
    toolbox_tools().find(|t| t.id == id)
}

pub fn toolbox_tool_in_toolkit(toolkit: &str, local: &str) -> Option<&'static ToolMeta> {
    let key = ToolKey::parse_canonical(toolkit, local).ok()?;
    toolbox_tools().find(|t| t.key() == key)
}

fn sorted_tools_for_surface(surface: Surface) -> Vec<&'static ToolMeta> {
    let mut tools: Vec<_> = toolbox_tools()
        .filter(|t| t.is_on_surface(surface))
        .collect();
    tools.sort_by_key(|t| t.id);
    tools
}

pub fn toolkits_for_surface(surface: Surface) -> Vec<&'static str> {
    let mut ids: Vec<_> = sorted_tools_for_surface(surface)
        .into_iter()
        .map(|t| t.toolkit)
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

pub fn tools_for_toolkit_on_surface(toolkit: &str, surface: Surface) -> Vec<&'static ToolMeta> {
    sorted_tools_for_surface(surface)
        .into_iter()
        .filter(|t| t.toolkit == toolkit)
        .collect()
}

pub fn tags_for_surface(surface: Surface) -> Vec<String> {
    let mut tags: Vec<_> = sorted_tools_for_surface(surface)
        .into_iter()
        .flat_map(|t| t.tag_labels().into_iter())
        .collect();
    tags.sort();
    tags.dedup();
    tags
}

pub fn tools_with_tag_on_surface(tag: &str, surface: Surface) -> Vec<&'static ToolMeta> {
    sorted_tools_for_surface(surface)
        .into_iter()
        .filter(|t| t.has_tag(tag))
        .collect()
}

pub fn boards_for_surface(surface: Surface) -> Vec<&'static str> {
    let mut boards: Vec<_> = sorted_tools_for_surface(surface)
        .into_iter()
        .flat_map(|t| t.boards.iter().copied())
        .collect();
    boards.sort_unstable();
    boards.dedup();
    boards
}

pub fn tools_on_board_for_surface(board: &str, surface: Surface) -> Vec<&'static ToolMeta> {
    sorted_tools_for_surface(surface)
        .into_iter()
        .filter(|t| t.is_on_board(board))
        .collect()
}

pub fn toolbox_has_id(id: &str) -> bool {
    upeg_core::inventory::iter::<StaticToolMeta>().any(|t| {
        t.assert_valid();
        t.id == id
    })
}

pub fn toolbox_has_tool_key(key: ToolKey<'_>) -> bool {
    upeg_core::inventory::iter::<StaticToolMeta>().any(|t| {
        t.assert_valid();
        t.key() == key
    })
}
