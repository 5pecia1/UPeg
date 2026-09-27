//! Reversible runtime overlay for one active directory project.

#![allow(
    clippy::expect_used,
    reason = "poisoned project registry locks are unrecoverable after a writer panic"
)]

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use upeg_core::{ToolMeta, ToolkitMeta};

use crate::dispatch::ToolDispatcher;

static SWITCHING: AtomicBool = AtomicBool::new(false);
static ACTIVE_CALLS: AtomicUsize = AtomicUsize::new(0);
static ACTIVE_READS: AtomicUsize = AtomicUsize::new(0);
static BLOCKED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
static ACTIVE_ROOT: OnceLock<Mutex<Option<std::path::PathBuf>>> = OnceLock::new();
static PROJECT_TOOLKITS: OnceLock<Mutex<HashMap<String, &'static ToolkitMeta>>> = OnceLock::new();
thread_local! { static TRANSITION_OWNER: Cell<bool> = const { Cell::new(false) }; }
thread_local! { static READ_DEPTH: Cell<usize> = const { Cell::new(0) }; }
thread_local! { static CALL_DEPTH: Cell<usize> = const { Cell::new(0) }; }

fn project_toolkits() -> &'static Mutex<HashMap<String, &'static ToolkitMeta>> {
    PROJECT_TOOLKITS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn set_project_toolkit_meta(meta: ToolkitMeta) {
    meta.assert_valid();
    let leaked = Box::leak(Box::new(meta));
    project_toolkits()
        .lock()
        .expect("project toolkits poisoned")
        .insert(leaked.id.to_string(), leaked);
}

pub fn project_toolkit_meta(id: &str) -> Option<&'static ToolkitMeta> {
    project_toolkits().lock().ok()?.get(id).copied()
}

pub fn project_toolkit_metas() -> Vec<&'static ToolkitMeta> {
    project_toolkits()
        .lock()
        .map(|metas| metas.values().copied().collect())
        .unwrap_or_default()
}

pub fn take_project_toolkits() -> HashMap<String, &'static ToolkitMeta> {
    std::mem::take(
        &mut *project_toolkits()
            .lock()
            .expect("project toolkits poisoned"),
    )
}

pub fn restore_project_toolkits(metas: impl IntoIterator<Item = (String, &'static ToolkitMeta)>) {
    *project_toolkits()
        .lock()
        .expect("project toolkits poisoned") = metas.into_iter().collect();
}

fn active_root() -> &'static Mutex<Option<std::path::PathBuf>> {
    ACTIVE_ROOT.get_or_init(|| Mutex::new(None))
}

pub fn set_active_project_root(root: Option<std::path::PathBuf>) {
    *active_root().lock().expect("project root poisoned") = root;
}

pub fn active_project_root() -> Option<std::path::PathBuf> {
    active_root().lock().ok()?.clone()
}

fn blocked() -> &'static Mutex<HashSet<String>> {
    BLOCKED.get_or_init(|| Mutex::new(HashSet::new()))
}

pub fn is_project_tool_blocked(id: &str) -> bool {
    blocked().lock().is_ok_and(|ids| ids.contains(id))
}

pub fn set_blocked_project_tools(ids: impl IntoIterator<Item = String>) {
    *blocked().lock().expect("project blocklist poisoned") = ids.into_iter().collect();
}

pub fn blocked_project_tools() -> HashSet<String> {
    blocked().lock().map(|ids| ids.clone()).unwrap_or_default()
}

pub struct ProjectTransitionGuard;

impl Drop for ProjectTransitionGuard {
    fn drop(&mut self) {
        TRANSITION_OWNER.with(|owner| owner.set(false));
        SWITCHING.store(false, Ordering::Release);
    }
}

pub fn begin_project_transition() -> Result<ProjectTransitionGuard, &'static str> {
    if SWITCHING.swap(true, Ordering::AcqRel) {
        return Err("a project transition is already in progress");
    }
    if ACTIVE_CALLS.load(Ordering::Acquire) != 0 {
        SWITCHING.store(false, Ordering::Release);
        return Err("wait for running tools before switching projects");
    }
    while ACTIVE_READS.load(Ordering::Acquire) != 0 {
        std::thread::yield_now();
    }
    TRANSITION_OWNER.with(|owner| owner.set(true));
    Ok(ProjectTransitionGuard)
}

pub struct CatalogReadGuard {
    counted: bool,
}

impl Drop for CatalogReadGuard {
    fn drop(&mut self) {
        READ_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
        if self.counted {
            ACTIVE_READS.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

pub fn catalog_read_guard() -> CatalogReadGuard {
    if TRANSITION_OWNER.with(Cell::get) {
        READ_DEPTH.with(|depth| depth.set(depth.get() + 1));
        return CatalogReadGuard { counted: false };
    }
    if READ_DEPTH.with(Cell::get) != 0 {
        READ_DEPTH.with(|depth| depth.set(depth.get() + 1));
        return CatalogReadGuard { counted: false };
    }
    loop {
        if !SWITCHING.load(Ordering::Acquire) {
            ACTIVE_READS.fetch_add(1, Ordering::AcqRel);
            if !SWITCHING.load(Ordering::Acquire) {
                READ_DEPTH.with(|depth| depth.set(1));
                return CatalogReadGuard { counted: true };
            }
            ACTIVE_READS.fetch_sub(1, Ordering::AcqRel);
        }
        std::thread::yield_now();
    }
}

pub struct ActiveCallGuard {
    counted: bool,
    _same_thread: std::marker::PhantomData<std::rc::Rc<()>>,
}

pub fn begin_call() -> Result<ActiveCallGuard, &'static str> {
    if CALL_DEPTH.with(Cell::get) != 0 {
        CALL_DEPTH.with(|depth| depth.set(depth.get() + 1));
        return Ok(ActiveCallGuard {
            counted: false,
            _same_thread: std::marker::PhantomData,
        });
    }
    if SWITCHING.load(Ordering::Acquire) {
        return Err("project context is switching");
    }
    ACTIVE_CALLS.fetch_add(1, Ordering::AcqRel);
    if SWITCHING.load(Ordering::Acquire) {
        ACTIVE_CALLS.fetch_sub(1, Ordering::AcqRel);
        return Err("project context is switching");
    }
    CALL_DEPTH.with(|depth| depth.set(1));
    Ok(ActiveCallGuard {
        counted: true,
        _same_thread: std::marker::PhantomData,
    })
}

impl Drop for ActiveCallGuard {
    fn drop(&mut self) {
        CALL_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
        if self.counted {
            ACTIVE_CALLS.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

#[derive(Clone)]
pub struct ProjectToolSnapshot {
    id: &'static str,
    previous_meta: Option<&'static ToolMeta>,
    previous_dispatcher: Option<ToolDispatcher>,
    previous_provenance: crate::ToolProvenance,
    previous_approval: crate::ToolApprovalPolicy,
    previous_credentials: Vec<String>,
    previous_requirements: Option<crate::execution_requirements::ToolExecutionRequirements>,
    previous_embed_url: Option<&'static str>,
    previous_selectors: Vec<crate::SelectorBinding>,
    previous_settings: crate::ControlledEmbedSettings,
    previous_triggers: Vec<crate::TriggerBinding>,
}

impl ProjectToolSnapshot {
    pub fn id(&self) -> &'static str {
        self.id
    }
    pub fn take(id: &'static str) -> Self {
        let snapshot = Self {
            id,
            previous_meta: crate::toolbox::take_runtime_tool(id),
            previous_dispatcher: crate::dispatch::take_runtime_dispatcher(id),
            previous_provenance: crate::tool_provenance(id),
            previous_approval: crate::tool_approval_policy(id),
            previous_credentials: crate::tool_credential_names(id),
            previous_requirements: crate::execution_requirements::tool_execution_requirements(id),
            previous_embed_url: crate::embed_url_for(id),
            previous_selectors: crate::selector_bindings_for(id),
            previous_settings: crate::controlled_embed_settings_for(id),
            previous_triggers: crate::trigger_bindings_for(id),
        };
        clear_sidecars(id);
        snapshot
    }

    pub fn previous_source_label(&self) -> Option<String> {
        self.previous_meta
            .as_ref()
            .map(|_| self.previous_provenance.label())
    }

    pub fn restore(self) {
        crate::toolbox::take_runtime_tool(self.id);
        crate::dispatch::take_runtime_dispatcher(self.id);
        clear_sidecars(self.id);
        if let Some(meta) = self.previous_meta {
            crate::toolbox_add_tool(meta.clone());
        }
        if let Some(dispatcher) = self.previous_dispatcher {
            crate::dispatch::restore_runtime_dispatcher(self.id, dispatcher);
        }
        crate::register_tool_provenance(self.id, self.previous_provenance);
        crate::set_tool_approval_policy(self.id, self.previous_approval);
        crate::set_tool_credential_names(self.id, self.previous_credentials);
        crate::execution_requirements::set_tool_execution_requirements(
            self.id,
            self.previous_requirements,
        );
        if let Some(url) = self.previous_embed_url {
            crate::register_embed_url(self.id, url);
        }
        crate::set_selector_bindings(self.id, self.previous_selectors);
        crate::set_controlled_embed_settings(self.id, self.previous_settings);
        crate::set_trigger_bindings(self.id, self.previous_triggers);
    }
}

fn clear_sidecars(id: &'static str) {
    crate::clear_tool_provenance(id);
    crate::set_tool_approval_policy(id, crate::ToolApprovalPolicy::none());
    crate::set_tool_credential_names(id, Vec::new());
    crate::execution_requirements::set_tool_execution_requirements(id, None);
    crate::clear_embed_url(id);
    crate::set_selector_bindings(id, Vec::new());
    crate::set_controlled_embed_settings(id, crate::ControlledEmbedSettings::default());
    crate::set_trigger_bindings(id, Vec::new());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn nested_catalog_read_remains_reentrant_when_transition_is_waiting() {
        let _serial = TEST_LOCK.lock().expect("test lock");
        let outer = catalog_read_guard();
        let (started_tx, started_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            started_tx.send(()).expect("started");
            let result = begin_project_transition();
            done_tx.send(result.is_ok()).expect("done");
        });
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("transition began");
        for _ in 0..1000 {
            if SWITCHING.load(Ordering::Acquire) {
                break;
            }
            std::thread::yield_now();
        }
        let nested = catalog_read_guard();
        drop(nested);
        drop(outer);
        assert!(
            done_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("transition completed")
        );
        worker.join().expect("worker joined");
    }

    #[test]
    fn nested_dispatch_call_stays_valid_during_a_rejected_transition() {
        let _serial = TEST_LOCK.lock().expect("test lock");
        let outer = begin_call().expect("outer call");
        let worker = std::thread::spawn(begin_project_transition);
        assert!(worker.join().expect("transition worker").is_err());
        SWITCHING.store(true, Ordering::Release);
        let nested = begin_call().expect("same-thread nested call");
        assert_eq!(ACTIVE_CALLS.load(Ordering::Acquire), 1);
        drop(nested);
        SWITCHING.store(false, Ordering::Release);
        drop(outer);
        assert_eq!(ACTIVE_CALLS.load(Ordering::Acquire), 0);
    }
}
