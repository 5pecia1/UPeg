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

//! Workspace layering gate — `docs/architecture/crate-boundaries.md`.
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
        crates: &["upeg-tools"],
    },
    Layer {
        name: "adapters",
        rank: 5,
        kind: LayerKind::Library,
        crates: &["upeg-loader", "upeg-wasm", "upeg-sources"],
    },
    Layer {
        name: "application+surface",
        rank: 6,
        kind: LayerKind::EntryPoint,
        crates: &["upeg-cli"],
    },
    Layer {
        name: "ui-state",
        rank: 7,
        kind: LayerKind::Library,
        crates: &["upeg-pegboard-ui"],
    },
    Layer {
        name: "bridge",
        rank: 8,
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
    ("upeg-cli", "upeg-tools"),
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
    ("upeg-frb", "upeg-tools"),
    ("upeg-loader", "upeg-core"),
    ("upeg-loader", "upeg-runtime"),
    ("upeg-loader", "upeg-wasm"),
    ("upeg-macros", "upeg-tool-grammar"),
    ("upeg-pegboard-ui", "upeg-core"),
    ("upeg-pegboard-ui", "upeg-runtime"),
    ("upeg-pegboard-ui", "upeg-sources"),
    ("upeg-pegboard-ui", "upeg-tools"),
    ("upeg-plugin-macros", "upeg-tool-grammar"),
    ("upeg-runtime", "upeg-core"),
    ("upeg-sources", "upeg-core"),
    ("upeg-sources", "upeg-loader"),
    ("upeg-sources", "upeg-runtime"),
    ("upeg-sources", "upeg-tools"),
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

/// 별칭 의존이 그래프에서 통째로 사라지던 구멍의 재현.
///
/// 멤버가 `alias = { workspace = true }`로 상속하면 실제 패키지 이름은
/// 루트 `[workspace.dependencies]`에만 있다. 그걸 읽지 않으면 노드
/// 이름이 `alias`가 되어 `upeg-` 접두사 필터에서 탈락하고, 계층 위반이
/// 있어도 정확 일치 비교가 통과해 버린다.
#[test]
fn 루트에서_이름을_바꾼_의존도_실제_패키지로_해석된다() {
    let root: toml::Table = r#"
[workspace.dependencies]
domain = { package = "upeg-core", version = "0.1" }
serde = "1"
"#
    .parse()
    .expect("합성 루트 매니페스트");
    let renames = workspace_renames(&root);

    let member: toml::Table = "
[dependencies]
domain = { workspace = true }
serde = { workspace = true }
"
    .parse()
    .expect("합성 멤버 매니페스트");

    let mut out = BTreeSet::new();
    collect_from_tables(&member, &renames, &mut out);

    assert_eq!(
        out,
        BTreeSet::from(["upeg-core".to_string()]),
        "별칭은 실제 패키지 이름으로 해석되고, upeg 밖 의존은 그대로 무시된다"
    );
}

#[test]
fn 멤버가_직접_적은_package_별칭이_루트_별칭보다_우선한다() {
    let root: toml::Table = r#"
[workspace.dependencies]
domain = { package = "upeg-core", version = "0.1" }
"#
    .parse()
    .expect("합성 루트 매니페스트");
    let renames = workspace_renames(&root);

    let member: toml::Table = r#"
[dependencies]
domain = { package = "upeg-runtime", version = "0.1" }
"#
    .parse()
    .expect("합성 멤버 매니페스트");

    let mut out = BTreeSet::new();
    collect_from_tables(&member, &renames, &mut out);

    assert_eq!(out, BTreeSet::from(["upeg-runtime".to_string()]));
}

#[test]
fn 상속하지_않는_같은_이름_의존에는_루트_별칭을_적용하지_않는다() {
    let root: toml::Table = r#"
[workspace.dependencies]
domain = { package = "upeg-core", version = "0.1" }
"#
    .parse()
    .expect("합성 루트 매니페스트");
    let renames = workspace_renames(&root);

    // 같은 키지만 `workspace = true`가 아니다 — 진짜로 `domain`이라는
    // 크레이트를 쓰는 것이므로 upeg 그래프의 노드가 아니다.
    let member: toml::Table = r#"
[dependencies]
domain = "1.0"
"#
    .parse()
    .expect("합성 멤버 매니페스트");

    let mut out = BTreeSet::new();
    collect_from_tables(&member, &renames, &mut out);

    assert!(out.is_empty(), "상속하지 않은 동명 의존: {out:?}");
}

#[test]
fn 크레이트_의존_그래프는_허용_표와_정확히_일치한다() {
    let actual = actual_edges();
    let allowed = allowed_edge_set();

    let unexpected: BTreeSet<_> = actual.difference(&allowed).cloned().collect();
    let stale: BTreeSet<_> = allowed.difference(&actual).cloned().collect();

    assert!(
        unexpected.is_empty() && stale.is_empty(),
        "crate dependency graph drifted from ALLOWED_EDGES.\n\
         새로 생긴(허용되지 않은) 엣지:\n{}\
         사라진(표에만 남은) 엣지:\n{}\
         고치는 방법: 의존을 되돌리거나, 계층이 정말 바뀌었다면 \
         upeg-core/tests/crate_boundaries.rs 의 ALLOWED_EDGES 와 \
         docs/architecture/crate-boundaries.md 를 함께 갱신한다.",
        render(&unexpected),
        render(&stale),
    );
}

#[test]
fn 모든_의존은_같거나_더_안쪽_계층을_향한다() {
    for (from, to) in actual_edges() {
        let from_layer = layer_of(&from);
        let to_layer = layer_of(&to);
        assert!(
            to_layer.rank <= from_layer.rank,
            "의존이 바깥으로 향한다: {from}({}, rank {}) -> {to}({}, rank {})",
            from_layer.name,
            from_layer.rank,
            to_layer.name,
            to_layer.rank,
        );
    }
}

#[test]
fn 진입점_크레이트를_향한_의존은_문서화된_예외뿐이다() {
    for (from, to) in actual_edges() {
        if layer_of(&to).kind != LayerKind::EntryPoint {
            continue;
        }
        let documented = DOCUMENTED_EXCEPTIONS
            .iter()
            .any(|(ex_from, ex_to)| *ex_from == from && *ex_to == to);
        assert!(
            documented,
            "{to} 는 진입점을 소유한 크레이트다 ({}). {from} 이 이를 의존하려면 \
             upeg-core/tests/crate_boundaries.rs 의 DOCUMENTED_EXCEPTIONS 에 \
             이유와 함께 등록되어야 한다.",
            layer_of(&to).name,
        );
    }
}

#[test]
fn 모든_워크스페이스_멤버는_정확히_한_계층에_속한다() {
    let root = workspace_root();
    for member in workspace_members(&root) {
        let hits = LAYERS
            .iter()
            .filter(|layer| layer.crates.contains(&member.as_str()))
            .count();
        assert_eq!(
            hits, 1,
            "{member} 는 정확히 한 계층에 속해야 한다 (현재 {hits}개)",
        );
    }
    let listed: usize = LAYERS.iter().map(|layer| layer.crates.len()).sum();
    assert_eq!(
        listed,
        workspace_members(&root).len(),
        "LAYERS 에만 있고 워크스페이스에는 없는 크레이트가 있다",
    );
}

#[test]
fn 계층_순위는_중복_없이_오름차순이다() {
    // rank 는 비교의 기준이므로 표 자체가 흐트러지면 모든 판정이 무의미해진다.
    for pair in LAYERS.windows(2) {
        assert!(
            pair[0].rank < pair[1].rank,
            "LAYERS 는 rank 오름차순이어야 한다: {} ({}) vs {} ({})",
            pair[0].name,
            pair[0].rank,
            pair[1].name,
            pair[1].rank,
        );
    }
}
