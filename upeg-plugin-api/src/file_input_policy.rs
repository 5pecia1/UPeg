use serde::{Deserialize, Serialize};

/// Default maximum number of regular-file leaves accepted by a File input.
pub const DEFAULT_PLUGIN_FILE_MAX_COUNT: u32 = 1;

/// Raw File-input policy carried by a plugin manifest.
///
/// This boundary DTO intentionally permits values that Core may reject.
/// `upeg-wasm` validates it while lowering into Core's non-optional policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginFileInputPolicy {
    /// Maximum number of regular-file leaves, recursively.
    #[serde(default = "default_max_count")]
    pub max_count: u32,
    /// Allowed canonical extensions; empty permits every extension.
    #[serde(default)]
    pub extensions: Vec<String>,
    /// Optional maximum bytes for each regular-file leaf.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_file_bytes: Option<u64>,
    /// Optional maximum bytes across all regular-file leaves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_total_bytes: Option<u64>,
}

impl Default for PluginFileInputPolicy {
    fn default() -> Self {
        Self {
            max_count: DEFAULT_PLUGIN_FILE_MAX_COUNT,
            extensions: Vec::new(),
            max_file_bytes: None,
            max_total_bytes: None,
        }
    }
}

const fn default_max_count() -> u32 {
    DEFAULT_PLUGIN_FILE_MAX_COUNT
}
