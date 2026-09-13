//! Dynamic Tool identity: `ToolId`, `ToolKey`, `ToolIdentity`, and
//! `ToolIdError`.
//!
//! Tool ids are intentionally dynamic — TOML, WASM, and MCP sources
//! can add ids at runtime, so a closed Rust enum would be the wrong
//! model. These newtypes give boundary code one typed place to enforce
//! a canonical full id before ids are leaked into static registries.

/// Borrowed, validated Tool id.
///
/// Tool ids are intentionally dynamic: TOML, WASM, and MCP sources can add ids
/// at runtime, so a closed Rust enum would be the wrong model. This newtype
/// still gives boundary code one typed place to enforce a canonical full id
/// before ids are leaked into static registries. A bare full id is deliberately
/// not split into Toolkit/local parts because both Toolkit ids and local Tool
/// ids may contain dots; callers that know the Toolkit must use
/// [`ToolIdentity`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ToolId<'a> {
    raw: &'a str,
}

/// Borrowed, validated structured Tool key.
///
/// This is the toolbox identity: Toolkit id and local Tool id are separate
/// fields, so dots inside either part never need separator inference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ToolKey<'a> {
    toolkit: &'a str,
    local: &'a str,
}

/// Borrowed, validated Tool identity within a known Toolkit.
///
/// The Toolkit id is supplied out-of-band instead of inferred from the first
/// dot in [`ToolId::as_str`]. That keeps `github.com.admin.tools.list`
/// unambiguous: Toolkit `github.com`, local Tool `admin.tools.list`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ToolIdentity<'a> {
    id: ToolId<'a>,
    toolkit: &'a str,
    local: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ToolIdError {
    #[error("tool id must be non-empty")]
    EmptyId,
    #[error("toolkit id must be non-empty")]
    EmptyToolkit,
    #[error("tool id must include a Toolkit/local separator `.`")]
    MissingSeparator,
    #[error("tool id must include a non-empty local name")]
    EmptyLocal,
    #[error("tool id `{id}` must be canonical and unpadded")]
    NonCanonicalId { id: String },
    #[error("toolkit id `{toolkit}` must be canonical and unpadded")]
    NonCanonicalToolkit { toolkit: String },
    #[error("tool id `{id}` must start with its owning Toolkit prefix `{toolkit}.`")]
    ToolkitMismatch { id: String, toolkit: String },
}

impl<'a> ToolId<'a> {
    pub fn parse(raw: &'a str) -> Result<Self, ToolIdError> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(ToolIdError::EmptyId);
        }
        if !raw.contains('.') {
            return Err(ToolIdError::MissingSeparator);
        }
        if raw.starts_with('.') {
            return Err(ToolIdError::EmptyToolkit);
        }
        if raw.ends_with('.') {
            return Err(ToolIdError::EmptyLocal);
        }
        Ok(Self { raw })
    }

    pub fn parse_canonical(raw: &'a str) -> Result<Self, ToolIdError> {
        let id = Self::parse(raw)?;
        if id.raw != raw || has_padded_dot_component(id.raw) {
            return Err(ToolIdError::NonCanonicalId {
                id: raw.to_string(),
            });
        }
        Ok(id)
    }

    pub fn parse_in_toolkit(
        raw: &'a str,
        toolkit: &'a str,
    ) -> Result<ToolIdentity<'a>, ToolIdError> {
        if raw.trim().is_empty() {
            return Err(ToolIdError::EmptyId);
        }
        let toolkit = toolkit.trim();
        if toolkit.is_empty() {
            return Err(ToolIdError::EmptyToolkit);
        }
        let id = Self::parse(raw)?;
        id.in_toolkit(toolkit)
    }

    pub fn parse_canonical_in_toolkit(
        raw: &'a str,
        toolkit: &'a str,
    ) -> Result<ToolIdentity<'a>, ToolIdError> {
        if raw.trim().is_empty() {
            return Err(ToolIdError::EmptyId);
        }
        if toolkit.trim().is_empty() {
            return Err(ToolIdError::EmptyToolkit);
        }
        if toolkit != toolkit.trim() {
            return Err(ToolIdError::NonCanonicalToolkit {
                toolkit: toolkit.to_string(),
            });
        }
        let id = Self::parse_canonical(raw)?;
        id.in_toolkit(toolkit)
    }

    pub const fn as_str(self) -> &'a str {
        self.raw
    }

    fn in_toolkit(self, toolkit: &'a str) -> Result<ToolIdentity<'a>, ToolIdError> {
        let Some(rest) = self.raw.strip_prefix(toolkit) else {
            return Err(ToolIdError::ToolkitMismatch {
                id: self.raw.to_string(),
                toolkit: toolkit.to_string(),
            });
        };
        let Some(local) = rest.strip_prefix('.') else {
            if rest.is_empty() {
                return Err(ToolIdError::EmptyLocal);
            }
            return Err(ToolIdError::ToolkitMismatch {
                id: self.raw.to_string(),
                toolkit: toolkit.to_string(),
            });
        };
        if local.is_empty() {
            return Err(ToolIdError::EmptyLocal);
        }
        if local != local.trim() {
            return Err(ToolIdError::NonCanonicalId {
                id: self.raw.to_string(),
            });
        }
        Ok(ToolIdentity {
            id: self,
            toolkit,
            local,
        })
    }
}

impl<'a> ToolKey<'a> {
    pub fn parse(toolkit: &'a str, local: &'a str) -> Result<Self, ToolIdError> {
        let toolkit = toolkit.trim();
        let local = local.trim();
        if toolkit.is_empty() {
            return Err(ToolIdError::EmptyToolkit);
        }
        if local.is_empty() {
            return Err(ToolIdError::EmptyLocal);
        }
        Ok(Self { toolkit, local })
    }

    pub fn parse_canonical(toolkit: &'a str, local: &'a str) -> Result<Self, ToolIdError> {
        if toolkit.trim().is_empty() {
            return Err(ToolIdError::EmptyToolkit);
        }
        if local.trim().is_empty() {
            return Err(ToolIdError::EmptyLocal);
        }
        if toolkit != toolkit.trim() {
            return Err(ToolIdError::NonCanonicalToolkit {
                toolkit: toolkit.to_string(),
            });
        }
        if has_padded_dot_component(toolkit) {
            return Err(ToolIdError::NonCanonicalToolkit {
                toolkit: toolkit.to_string(),
            });
        }
        if local != local.trim() || has_padded_dot_component(local) {
            return Err(ToolIdError::NonCanonicalId {
                id: local.to_string(),
            });
        }
        Ok(Self { toolkit, local })
    }

    pub const fn toolkit(self) -> &'a str {
        self.toolkit
    }

    pub const fn local(self) -> &'a str {
        self.local
    }
}

impl<'a> ToolIdentity<'a> {
    pub const fn as_str(self) -> &'a str {
        self.id.as_str()
    }

    pub const fn key(self) -> ToolKey<'a> {
        ToolKey {
            toolkit: self.toolkit,
            local: self.local,
        }
    }

    pub const fn toolkit(self) -> &'a str {
        self.toolkit
    }

    pub const fn local(self) -> &'a str {
        self.local
    }
}

#[allow(
    clippy::expect_used,
    reason = "ToolMeta fields are produced by the `#[tool]` macro from compile-time-canonical static strings; parse cannot fail at runtime."
)]
pub(crate) fn canonical_tool_id_in_toolkit<'a>(id: &'a str, toolkit: &'a str) -> ToolIdentity<'a> {
    ToolId::parse_canonical_in_toolkit(id, toolkit)
        .expect("tool id must be canonical and match its unpadded toolkit")
}

fn has_padded_dot_component(value: &str) -> bool {
    value.split('.').any(|part| part != part.trim())
}

pub(crate) fn canonical_toolkit_id(id: &str) {
    assert!(!id.trim().is_empty(), "toolkit id must be non-empty");
    assert_eq!(id, id.trim(), "toolkit id must be canonical and unpadded");
    assert!(
        !has_padded_dot_component(id),
        "toolkit id must be canonical and contain unpadded dot components"
    );
}
