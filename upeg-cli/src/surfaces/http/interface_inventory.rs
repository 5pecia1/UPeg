//! HTTP surface interface-inventory declarations.
//!
//! Extracted from `surfaces/http/mod.rs` to keep that file under the
//! workspace 1000-line file-size budget — this is pure metadata (route
//! id/method/path declarations lowered into `InterfaceEntry`), not
//! router/middleware behavior, so it splits cleanly with no shared state.

use upeg_core::{
    Surface,
    interface_inventory::{
        Compatibility, ContractIo, ContractIoKind, ContractLocator, ContractShape, DocRef,
        InterfaceEntry, InterfaceKind, OwnerRef, SourceRef, SurfaceSet, TestMapping,
    },
};

// Interface-inventory metadata (feature/test). Lives alongside the data-
// plane state in `mod.rs` conceptually, so `interface inventory check` can
// enumerate the HTTP surface contracts without parsing axum routes.
const HTTP_SURFACE_VERSION: &str = "v1";
const HTTP_OWNER_PATH: &str = "upeg-cli/Cargo.toml";
const HTTP_DOCS_PATH: &str = "README.md";
const HTTP_TEST_PATH: &str = "upeg-cli/src/inventory/tests.rs";
/// The test inside [`HTTP_TEST_PATH`] that actually asserts these
/// entries — pinned by the inventory honesty check.
const HTTP_TEST_NAME: &str = "interface_inventory_covers_cli_http_and_mcp";

struct HttpRouteDeclaration {
    id: &'static str,
    method: &'static str,
    path: &'static str,
}

const HTTP_ROUTES: &[HttpRouteDeclaration] = &[
    HttpRouteDeclaration {
        id: "http.v1.tools.list",
        method: "GET",
        path: "/v1/tools",
    },
    HttpRouteDeclaration {
        id: "http.v1.tools.readiness",
        method: "GET",
        path: "/v1/tools/{id}/readiness",
    },
    HttpRouteDeclaration {
        id: "http.v1.tools.call",
        method: "POST",
        path: "/v1/tools/{id}",
    },
    HttpRouteDeclaration {
        id: "http.v1.tools.call.stream",
        method: "POST",
        path: "/v1/tools/{id}/stream",
    },
];

pub(crate) fn interface_inventory_entries() -> Vec<InterfaceEntry> {
    HTTP_ROUTES
        .iter()
        .map(HttpRouteDeclaration::interface_entry)
        .collect()
}

impl HttpRouteDeclaration {
    fn interface_entry(&self) -> InterfaceEntry {
        InterfaceEntry {
            id: self.id.to_string(),
            surfaces: SurfaceSet::single(Surface::Http),
            kind: InterfaceKind::HttpRoute,
            contract: ContractShape::new(
                ContractLocator::http(self.method, self.path),
                ContractIo::declared(
                    ContractIoKind::HttpRequest,
                    None,
                    None,
                    "HTTP request contract",
                ),
                ContractIo::not_declared(
                    "HTTP route responses use route-specific schemas; tool dispatch routes return canonical ToolResult envelopes",
                ),
            ),
            version: HTTP_SURFACE_VERSION.to_string(),
            compatibility: Compatibility::Stable,
            owner: OwnerRef {
                path: Some(HTTP_OWNER_PATH.to_string()),
                url: None,
            },
            docs: DocRef {
                path: Some(HTTP_DOCS_PATH.to_string()),
                url: None,
            },
            source: SourceRef {
                path: Some("upeg-cli/src/surfaces/http/mod.rs".to_string()),
                url: None,
            },
            tests: TestMapping::covered(HTTP_TEST_PATH, Some(HTTP_TEST_NAME.to_string())),
        }
    }
}
