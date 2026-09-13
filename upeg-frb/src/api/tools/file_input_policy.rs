use upeg_core::FileInputPolicy;

/// File-selection limits exposed to Dart without weakening Core invariants.
#[derive(Debug, Clone, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct FileInputPolicyDto {
    pub max_count: u32,
    pub extensions: Vec<String>,
    pub max_file_bytes: Option<u64>,
    pub max_total_bytes: Option<u64>,
}

impl From<&FileInputPolicy> for FileInputPolicyDto {
    fn from(policy: &FileInputPolicy) -> Self {
        Self {
            max_count: policy.max_count(),
            extensions: policy.extensions().map(str::to_string).collect(),
            max_file_bytes: policy.max_file_bytes(),
            max_total_bytes: policy.max_total_bytes(),
        }
    }
}
