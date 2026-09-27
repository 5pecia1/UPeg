#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::get_unwrap,
    clippy::tests_outside_test_module,
    clippy::print_stdout,
    clippy::string_add,
    clippy::manual_let_else,
    reason = "integration tests use unwrap/expect/panic idiomatically and need not satisfy production restriction lints"
)]

//! Workspace layering gate.
//!
//! Dependencies always point inward: `surface → adapter → runtime →
//! domain`. The layers and their owned scopes:
//!
//! | Crate | Layer | Owns | Does not own |
//! |---|---|---|---|
//! | `upeg-core` | domain | Toolkit/Tool/Chain/Board value types, schema contracts, pure validation, pure positional binding, capability verdicts | runtime toolbox, dispatch, host I/O, UX labels |
//! | `upeg-runtime` | runtime | toolbox overlay, dispatch, trigger binding/execution, embed binding, manifest lowering, conflict policy | source-format parsing, surface UI flow |
//! | `upeg-toolkit-catalog` | metadata | generated built-in metadata and artifact digest contract | executable engines, network download |
//! | `upeg-toolkit-native` | adapter | verified native pack download, cache, sidecar dispatch | tool implementations |
//! | `upeg-toolkit-guest` / `upeg-toolkit-pack` | build artifacts | per-toolkit guest entry point and pack builder | shell UI |
//! | `upeg-loader` | adapter | parses `upeg.toml`-style sources, calls runtime lowering | its own toolbox/dispatch semantics |
//! | `upeg-wasm` | adapter | parses/hosts WASM sources, calls runtime lowering | its own toolbox/conflict policy |
//! | `upeg-sources` | source boundary | runtime source discovery/registration: user Toolkits, project manifest, WASM plugins, upstream MCP servers | surface UI flow, parser internals |
//! | `upeg-cli` | surface + host | CLI/TUI/HTTP/MCP entry points and user I/O; the host runtime: `server.json` discovery, bearer auth, daemon supervision, embedded HTTP, pause state, MCP-import loading, `pid_alive` | domain policy duplicated from core/runtime, pure path resolution (`upeg_core::paths`) |
//! | `upeg-pegboard-ui` | UI state | framework-independent pegboard state, deep-link contract, pin chrome, i18n catalog | rendering, widget code, grid geometry (Dart), placement algorithm (`upeg-runtime`) |
//! | `upeg-frb` | surface boundary | the Rust↔Dart FRB surface, host bootstrap, instance lock | domain/runtime policy |
//! | `flutter_app/` | surface | Flutter desktop/PWA UI flow | toolbox semantics, domain validation |
//!
//! The one allowed surface→surface edge is `upeg-frb → upeg-cli`: the
//! desktop shell embeds the host instead of spawning a separate process,
//! so the host runtime (`embedded_http_with_ready`, `ServerInfo`,
//! pause, `load_mcp_imports_for_host`, `pid_alive`) lives in `upeg-cli`.
//! `upeg-frb → upeg-loader` exists only under dev-dependencies — the
//! shipped graph never sees it. The loader never creates runtime truth:
//! overlay precedence, static-id protection, duplicate replacement,
//! trigger registration, embed-binding normalization, and
//! manifest→toolbox conversion are applied only in `upeg-runtime`
//! lowering.
//!
//! Reads every workspace member's `Cargo.toml` and reconstructs the
//! `upeg-*` → `upeg-*` edge set, then holds it against three rules:
//!
//! 1. **Exact edge set.** The graph must equal [`ALLOWED_EDGES`]. An exact-set
//!    comparison (not a subset check) so a *new* undeclared edge fails AND a
//!    *stale* allowed entry fails once the code stops needing it.
//! 2. **Inward only.** Every edge lands on a layer whose rank is ≤ the
//!    source's. Layers are data ([`LAYERS`]); same-rank edges inside one layer
//!    are peers and allowed (e.g. `upeg-sources` composing `upeg-loader`).
//! 3. **Nothing depends on an entry point.** A crate that owns process entry
//!    points ([`LayerKind::EntryPoint`]) may be depended on only via an entry
//!    in [`DOCUMENTED_EXCEPTIONS`].
//!
//! Lives in `upeg-core` — the leaf crate — deliberately: the gate is about
//! manifests, not code, so it should not pay for compiling any crate above it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Prefix every crate this gate governs shares. Non-`upeg` dependencies
/// (serde, tokio, …) are out of scope — they carry no layering meaning.
const CRATE_PREFIX: &str = "upeg-";

/// Cargo tables that declare dependencies. `[target.*]` sub-tables are
/// scanned for the same three names.
const DEPENDENCY_TABLES: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

const TARGET_TABLE: &str = "target";
const WORKSPACE_TABLE: &str = "workspace";
const MEMBERS_KEY: &str = "members";
const PACKAGE_RENAME_KEY: &str = "package";
/// The `dep = { workspace = true }` inheritance flag. Same spelling as
/// [`WORKSPACE_TABLE`], different meaning: this one is a *key inside a
/// dependency entry*, saying "take this dependency's definition from the
/// root `[workspace.dependencies]`".
const WORKSPACE_INHERIT_KEY: &str = "workspace";
const DEPENDENCIES_KEY: &str = "dependencies";
const MANIFEST_FILE: &str = "Cargo.toml";

/// Whether a layer owns process entry points, and is therefore something
/// nothing else may depend on without a documented reason.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LayerKind {
    /// A library layer: free to be depended on from anywhere above it.
    Library,
    /// Owns entry points into the process — `main`, an FFI export, an HTTP
    /// router, a stdio protocol loop. Depending on one of these drags an
    /// entire application into the dependent's build.
    EntryPoint,
}

struct Layer {
    name: &'static str,
    /// Position in the inward/outward ordering. A dependency may only point
    /// at a rank ≤ its own.
    rank: u8,
    kind: LayerKind,
    crates: &'static [&'static str],
}

/// The workspace's layering, as data. `rank` is the whole contract: a crate
/// may depend on strictly-lower ranks freely and on its own rank (peers in
/// the same layer), never on a higher one.
///
/// `domain-contracts` sits *below* `macros` because the proc-macro crates
/// consume the shared `#[tool]` grammar at compile time, while `upeg-core`
/// consumes the proc-macros — the three are one domain concern spread over
/// three ranks by Rust's compile-order, not three architectural layers.
const LAYERS: &[Layer] = &[
    Layer {
        name: "domain-contracts",
        rank: 0,
        kind: LayerKind::Library,
        crates: &["upeg-tool-grammar", "upeg-plugin-api"],
    },
    Layer {
        name: "macros",
        rank: 1,
        kind: LayerKind::Library,
        crates: &["upeg-macros", "upeg-plugin-macros"],
    },
    Layer {
        name: "domain",
        rank: 2,
        kind: LayerKind::Library,
        crates: &["upeg-core"],
    },
    Layer {
        name: "runtime",
        rank: 3,
        kind: LayerKind::Library,
        crates: &["upeg-runtime"],
    },
    Layer {
        name: "tools",
        rank: 4,
        kind: LayerKind::Library,
        crates: &["upeg-tools", "upeg-toolkit-catalog"],
    },
    Layer {
        name: "toolkit-artifacts",
        rank: 5,
        kind: LayerKind::Library,
        crates: &["upeg-toolkit-native"],
    },
    Layer {
        name: "toolkit-artifact-entrypoints",
        rank: 6,
        kind: LayerKind::EntryPoint,
        crates: &["upeg-toolkit-guest", "upeg-toolkit-pack"],
    },
    Layer {
        name: "adapters",
        rank: 7,
        kind: LayerKind::Library,
        crates: &["upeg-loader", "upeg-wasm", "upeg-sources"],
    },
    Layer {
        name: "application+surface",
        rank: 8,
        kind: LayerKind::EntryPoint,
        crates: &["upeg-cli"],
    },
    Layer {
        name: "ui-state",
        rank: 9,
        kind: LayerKind::Library,
        crates: &["upeg-pegboard-ui"],
    },
    Layer {
        name: "bridge",
        rank: 10,
        kind: LayerKind::EntryPoint,
        crates: &["upeg-frb"],
    },
];

/// Edges into an [`LayerKind::EntryPoint`] crate that the architecture
/// deliberately accepts.
///
/// `upeg-frb → upeg-cli`: the Flutter desktop shell *embeds the host* rather
/// than shelling out to one (PRD §5.9). The host runtime — discovery
/// (`server.json`), bearer auth, the embedded HTTP server, pause state, MCP
/// import loading — lives inside `upeg-cli`, which wears two hats: it is both
/// the CLI/TUI surface and the application/host layer. Until that crate is
/// split, the bridge has to reach it here. The edge is kept narrow: pure path
/// resolution (`config_root`, `desktop.lock`) comes from `upeg-core::paths`
/// instead, so only genuinely host-shaped calls cross this line.
const DOCUMENTED_EXCEPTIONS: [(&str, &str); 1] = [("upeg-frb", "upeg-cli")];

/// The complete `upeg-*` → `upeg-*` dependency graph. Sorted, deduplicated,
/// and compared for exact equality against what the manifests actually say.
const ALLOWED_EDGES: &[(&str, &str)] = &[
    ("upeg-cli", "upeg-core"),
    ("upeg-cli", "upeg-loader"),
    ("upeg-cli", "upeg-runtime"),
    ("upeg-cli", "upeg-sources"),
    ("upeg-cli", "upeg-toolkit-native"),
    ("upeg-cli", "upeg-wasm"),
    ("upeg-core", "upeg-macros"),
    // The one documented surface→surface edge; see DOCUMENTED_EXCEPTIONS.
    ("upeg-frb", "upeg-cli"),
    ("upeg-frb", "upeg-core"),
    // dev-dependency, native targets only: the bridge's own tests need a
    // real loader-registered Chain (approval guards are installed at
    // registration time) and a real `SkippedTool` to assert the boot
    // summary on a host that never skips anything itself.
    ("upeg-frb", "upeg-loader"),
    ("upeg-frb", "upeg-pegboard-ui"),
    ("upeg-frb", "upeg-runtime"),
    ("upeg-frb", "upeg-sources"),
    ("upeg-frb", "upeg-toolkit-catalog"),
    ("upeg-frb", "upeg-toolkit-native"),
    ("upeg-loader", "upeg-core"),
    ("upeg-loader", "upeg-runtime"),
    ("upeg-loader", "upeg-wasm"),
    ("upeg-macros", "upeg-tool-grammar"),
    ("upeg-pegboard-ui", "upeg-core"),
    ("upeg-pegboard-ui", "upeg-runtime"),
    ("upeg-pegboard-ui", "upeg-sources"),
    ("upeg-pegboard-ui", "upeg-toolkit-catalog"),
    ("upeg-plugin-macros", "upeg-tool-grammar"),
    ("upeg-runtime", "upeg-core"),
    ("upeg-sources", "upeg-core"),
    ("upeg-sources", "upeg-loader"),
    ("upeg-sources", "upeg-runtime"),
    ("upeg-sources", "upeg-toolkit-native"),
    ("upeg-toolkit-catalog", "upeg-core"),
    ("upeg-toolkit-catalog", "upeg-runtime"),
    ("upeg-toolkit-guest", "upeg-core"),
    // dev-only parity test compares the guest's embedded ABI to the catalog.
    ("upeg-toolkit-guest", "upeg-toolkit-catalog"),
    ("upeg-toolkit-guest", "upeg-tools"),
    ("upeg-toolkit-native", "upeg-core"),
    ("upeg-toolkit-native", "upeg-runtime"),
    ("upeg-toolkit-native", "upeg-toolkit-catalog"),
    ("upeg-toolkit-pack", "upeg-core"),
    ("upeg-toolkit-pack", "upeg-runtime"),
    ("upeg-toolkit-pack", "upeg-toolkit-catalog"),
    ("upeg-toolkit-pack", "upeg-tools"),
    ("upeg-sources", "upeg-wasm"),
    ("upeg-tools", "upeg-core"),
    ("upeg-tools", "upeg-runtime"),
    ("upeg-wasm", "upeg-core"),
    ("upeg-wasm", "upeg-plugin-api"),
    ("upeg-wasm", "upeg-runtime"),
    // dev-dependency: the WASM adapter's own tests load a `upeg.toml` fixture.
    ("upeg-wasm", "upeg-loader"),
];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("upeg-core must sit directly under the workspace root")
        .to_path_buf()
}

fn parse_manifest(path: &Path) -> toml::Table {
    let text =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    text.parse::<toml::Table>()
        .unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

/// Aliases declared once in the root `[workspace.dependencies]`, as
/// alias → real package name. Only `upeg-*` targets are worth keeping;
/// everything else carries no layering meaning.
type WorkspaceRenames = BTreeMap<String, String>;

/// Read the root manifest's `[workspace.dependencies]` renames.
///
/// A member that inherits an aliased dependency writes only
/// `alias = { workspace = true }` — the real package name lives solely in
/// the root manifest. Without this map an aliased `upeg-*` dependency
/// would be invisible to the whole gate: the edge simply would not appear
/// in the graph, and rule 1's exact-set comparison would pass while the
/// layering was actually violated.
fn workspace_renames(root_manifest: &toml::Table) -> WorkspaceRenames {
    let Some(deps) = root_manifest
        .get(WORKSPACE_TABLE)
        .and_then(toml::Value::as_table)
        .and_then(|workspace| workspace.get(DEPENDENCIES_KEY))
        .and_then(toml::Value::as_table)
    else {
        return WorkspaceRenames::new();
    };
    deps.iter()
        .filter_map(|(alias, value)| {
            let real = value
                .get(PACKAGE_RENAME_KEY)
                .and_then(toml::Value::as_str)?;
            Some((alias.clone(), normalize(real)))
        })
        .collect()
}

/// Cargo allows renaming: `alias = { package = "upeg-core" }` on the member
/// itself, or the same rename inherited from the root workspace table via
/// `alias = { workspace = true }`. Resolve either form to the real package
/// name, then normalize `_` → `-` so `upeg_frb` (the package name, chosen
/// for cargokit's artifact stem) and `upeg-frb` (the directory) are the
/// same node.
fn edge_target(dep_key: &str, dep_value: &toml::Value, renames: &WorkspaceRenames) -> String {
    // A member's own `package = ` wins: Cargo forbids pairing it with
    // `workspace = true`, so the two forms never both apply.
    if let Some(local) = dep_value
        .get(PACKAGE_RENAME_KEY)
        .and_then(toml::Value::as_str)
    {
        return normalize(local);
    }
    if inherits_from_workspace(dep_value)
        && let Some(inherited) = renames.get(dep_key)
    {
        return inherited.clone();
    }
    normalize(dep_key)
}

/// Does this dependency entry take its definition from the root
/// `[workspace.dependencies]`? Only then may a root-declared alias be
/// applied to it — a member that spells out its own `alias = "1.0"` means
/// the crate literally named `alias`.
fn inherits_from_workspace(dep_value: &toml::Value) -> bool {
    dep_value
        .get(WORKSPACE_INHERIT_KEY)
        .and_then(toml::Value::as_bool)
        == Some(true)
}

fn normalize(name: &str) -> String {
    name.replace('_', "-")
}

fn collect_from_tables(
    table: &toml::Table,
    renames: &WorkspaceRenames,
    out: &mut BTreeSet<String>,
) {
    for name in DEPENDENCY_TABLES {
        let Some(deps) = table.get(name).and_then(toml::Value::as_table) else {
            continue;
        };
        for (key, value) in deps {
            let target = edge_target(key, value, renames);
            if target.starts_with(CRATE_PREFIX) {
                out.insert(target);
            }
        }
    }
}

/// Every `upeg-*` dependency of `member`, across plain, dev, build and
/// `[target.'cfg(...)']` tables. Target-gated edges count: a `cfg`-gated
/// dependency is still a dependency on some platform.
fn dependencies_of(root: &Path, member: &str, renames: &WorkspaceRenames) -> BTreeSet<String> {
    let manifest = parse_manifest(&root.join(member).join(MANIFEST_FILE));
    let mut out = BTreeSet::new();
    collect_from_tables(&manifest, renames, &mut out);
    if let Some(targets) = manifest.get(TARGET_TABLE).and_then(toml::Value::as_table) {
        for cfg_table in targets.values() {
            if let Some(cfg_table) = cfg_table.as_table() {
                collect_from_tables(cfg_table, renames, &mut out);
            }
        }
    }
    out
}

fn workspace_members(root: &Path) -> Vec<String> {
    parse_manifest(&root.join(MANIFEST_FILE))
        .get(WORKSPACE_TABLE)
        .and_then(toml::Value::as_table)
        .and_then(|w| w.get(MEMBERS_KEY))
        .and_then(toml::Value::as_array)
        .expect("[workspace] members must be an array")
        .iter()
        .filter_map(toml::Value::as_str)
        .map(normalize)
        .collect()
}

/// The live graph, read out of the manifests.
fn actual_edges() -> BTreeSet<(String, String)> {
    let root = workspace_root();
    let renames = workspace_renames(&parse_manifest(&root.join(MANIFEST_FILE)));
    let mut edges = BTreeSet::new();
    for member in workspace_members(&root) {
        for dep in dependencies_of(&root, &member, &renames) {
            edges.insert((member.clone(), dep));
        }
    }
    edges
}

fn layer_of(krate: &str) -> &'static Layer {
    LAYERS
        .iter()
        .find(|layer| layer.crates.contains(&krate))
        .unwrap_or_else(|| panic!("crate `{krate}` has no layer — add it to LAYERS in this test"))
}

fn allowed_edge_set() -> BTreeSet<(String, String)> {
    ALLOWED_EDGES
        .iter()
        .map(|(from, to)| ((*from).to_string(), (*to).to_string()))
        .collect()
}

fn render(edges: &BTreeSet<(String, String)>) -> String {
    use std::fmt::Write as _;
    edges.iter().fold(String::new(), |mut out, (from, to)| {
        let _ = writeln!(out, "  {from} -> {to}");
        out
    })
}

/// Reproduces the hole where an aliased dependency vanished from the
/// graph entirely.
///
/// When a member inherits `alias = { workspace = true }`, the real
/// package name lives only in the root `[workspace.dependencies]`.
/// Without reading it, the node is named `alias`, falls out of the
/// `upeg-` prefix filter, and the exact-set comparison passes even when
/// a layering violation exists.
#[test]
fn dependency_renamed_at_the_root_resolves_to_the_real_package() {
    let root: toml::Table = r#"
[workspace.dependencies]
domain = { package = "upeg-core", version = "0.1" }
serde = "1"
"#
    .parse()
    .expect("synthetic root manifest");
    let renames = workspace_renames(&root);

    let member: toml::Table = "
[dependencies]
domain = { workspace = true }
serde = { workspace = true }
"
    .parse()
    .expect("synthetic member manifest");

    let mut out = BTreeSet::new();
    collect_from_tables(&member, &renames, &mut out);

    assert_eq!(
        out,
        BTreeSet::from(["upeg-core".to_string()]),
        "aliases resolve to real package names and non-upeg dependencies are ignored"
    );
}

#[test]
fn member_local_package_alias_wins_over_the_root_alias() {
    let root: toml::Table = r#"
[workspace.dependencies]
domain = { package = "upeg-core", version = "0.1" }
"#
    .parse()
    .expect("synthetic root manifest");
    let renames = workspace_renames(&root);

    let member: toml::Table = r#"
[dependencies]
domain = { package = "upeg-runtime", version = "0.1" }
"#
    .parse()
    .expect("synthetic member manifest");

    let mut out = BTreeSet::new();
    collect_from_tables(&member, &renames, &mut out);

    assert_eq!(out, BTreeSet::from(["upeg-runtime".to_string()]));
}

#[test]
fn same_name_dependency_that_does_not_inherit_ignores_the_root_alias() {
    let root: toml::Table = r#"
[workspace.dependencies]
domain = { package = "upeg-core", version = "0.1" }
"#
    .parse()
    .expect("synthetic root manifest");
    let renames = workspace_renames(&root);

    // Same key but no `workspace = true` — this really uses a crate named
    // `domain`, so it is not a node in the upeg graph.
    let member: toml::Table = r#"
[dependencies]
domain = "1.0"
"#
    .parse()
    .expect("synthetic member manifest");

    let mut out = BTreeSet::new();
    collect_from_tables(&member, &renames, &mut out);

    assert!(
        out.is_empty(),
        "non-inherited same-name dependency: {out:?}"
    );
}

#[test]
fn crate_dependency_graph_matches_the_allowed_table_exactly() {
    let actual = actual_edges();
    let allowed = allowed_edge_set();

    let unexpected: BTreeSet<_> = actual.difference(&allowed).cloned().collect();
    let stale: BTreeSet<_> = allowed.difference(&actual).cloned().collect();

    assert!(
        unexpected.is_empty() && stale.is_empty(),
        "crate dependency graph drifted from ALLOWED_EDGES.\n\
         new (not allowed) edges:\n{}\
         vanished (still listed) edges:\n{}\
         how to fix: revert the dependency, or if layering really changed, \
         update ALLOWED_EDGES in upeg-core/tests/crate_boundaries.rs \
         together with this file's module docs.",
        render(&unexpected),
        render(&stale),
    );
}

#[test]
fn every_dependency_points_to_the_same_or_an_inner_layer() {
    for (from, to) in actual_edges() {
        let from_layer = layer_of(&from);
        let to_layer = layer_of(&to);
        assert!(
            to_layer.rank <= from_layer.rank,
            "dependency points outward: {from}({}, rank {}) -> {to}({}, rank {})",
            from_layer.name,
            from_layer.rank,
            to_layer.name,
            to_layer.rank,
        );
    }
}

#[test]
fn dependencies_on_entry_point_crates_are_documented_exceptions_only() {
    for (from, to) in actual_edges() {
        if layer_of(&to).kind != LayerKind::EntryPoint {
            continue;
        }
        let documented = DOCUMENTED_EXCEPTIONS
            .iter()
            .any(|(ex_from, ex_to)| *ex_from == from && *ex_to == to);
        assert!(
            documented,
            "{to} is a crate that owns entry points ({}). For {from} to depend \
             on it, it must be registered with a reason in \
             DOCUMENTED_EXCEPTIONS in upeg-core/tests/crate_boundaries.rs.",
            layer_of(&to).name,
        );
    }
}

#[test]
fn every_workspace_member_belongs_to_exactly_one_layer() {
    let root = workspace_root();
    for member in workspace_members(&root) {
        let hits = LAYERS
            .iter()
            .filter(|layer| layer.crates.contains(&member.as_str()))
            .count();
        assert_eq!(
            hits, 1,
            "{member} must belong to exactly one layer (currently {hits})",
        );
    }
    let listed: usize = LAYERS.iter().map(|layer| layer.crates.len()).sum();
    assert_eq!(
        listed,
        workspace_members(&root).len(),
        "a crate is listed in LAYERS but missing from the workspace",
    );
}

#[test]
fn layer_ranks_are_strictly_ascending_without_duplicates() {
    // rank is the basis of comparison, so if the table itself drifts, every
    // verdict becomes meaningless.
    for pair in LAYERS.windows(2) {
        assert!(
            pair[0].rank < pair[1].rank,
            "LAYERS must be in ascending rank order: {} ({}) vs {} ({})",
            pair[0].name,
            pair[0].rank,
            pair[1].name,
            pair[1].rank,
        );
    }
}
