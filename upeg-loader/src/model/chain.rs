//! Small chain declaration helpers shared by parsing and validation.

pub(crate) fn chain_step_key(position: usize, id: Option<&str>) -> String {
    id.map(str::trim)
        .filter(|id| !id.is_empty())
        .map_or_else(|| format!("step{}", position + 1), str::to_string)
}
