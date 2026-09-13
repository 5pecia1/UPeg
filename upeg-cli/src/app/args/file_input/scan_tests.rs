use std::cell::Cell;

use upeg_core::InputName;

use super::bounded_directory_entries;
use crate::CliError;

#[test]
fn 과다_디렉터리_스캔은_허용_개수보다_하나_더_읽으면_즉시_중단한다() {
    let yielded = Cell::new(0_u32);
    let entries = std::iter::from_fn(|| {
        let current = yielded.get();
        yielded.set(current + 1);
        Some(Ok::<_, CliError>(current))
    });
    let field_name = InputName::new("images").expect("테스트 입력 이름은 유효해야 한다");

    let result = bounded_directory_entries(&field_name, 2, entries);

    assert!(result.is_err());
    assert_eq!(yielded.get(), 3);
}
