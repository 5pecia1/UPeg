use std::cell::Cell;

use upeg_core::InputName;

use super::bounded_directory_entries;
use crate::CliError;

#[test]
fn an_overfull_directory_scan_stops_right_after_one_past_the_limit() {
    let yielded = Cell::new(0_u32);
    let entries = std::iter::from_fn(|| {
        let current = yielded.get();
        yielded.set(current + 1);
        Some(Ok::<_, CliError>(current))
    });
    let field_name = InputName::new("images").expect("test input name must be valid");

    let result = bounded_directory_entries(&field_name, 2, entries);

    assert!(result.is_err());
    assert_eq!(yielded.get(), 3);
}
