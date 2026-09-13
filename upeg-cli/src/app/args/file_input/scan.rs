use upeg_core::InputName;

use crate::CliError;

pub(super) fn bounded_directory_entries<T>(
    field_name: &InputName,
    max_count: u32,
    entries: impl Iterator<Item = Result<T, CliError>>,
) -> Result<Vec<T>, CliError> {
    let max_count_usize = usize::try_from(max_count).map_err(|_| {
        CliError::tool_failed(format!(
            "file input `{}` max_count={max_count} exceeds this platform's capacity",
            field_name.as_str()
        ))
    })?;
    let mut bounded = Vec::new();
    for entry in entries {
        let entry = entry?;
        if bounded.len() == max_count_usize {
            let actual = u64::from(max_count) + 1;
            return Err(CliError::tool_failed(format!(
                "file input `{}` has at least {actual} files, over max_count={max_count}",
                field_name.as_str()
            )));
        }
        bounded.push(entry);
    }
    Ok(bounded)
}
