//! `csv` toolkit — row-level CSV diffing, CSV→JSON, and column selection.
//!
//! `csv.diff` was promoted from `gui_meta.rs` (previously a GUI-only meta
//! with `pin = Launcher` and no dispatcher — running it failed with
//! "dispatch not implemented for csv.diff"). `csv.to_json` and
//! `csv.select` joined it later, clearing the R6 "≥2 tools per toolkit"
//! consistency rule that `csv` used to carry as a documented single-tool
//! exception (the `#[tool]` macro requires `toolkit` to literally prefix
//! `id`, so a `csv.*` id always forces `toolkit = "csv"`).
//!
//! Rows are parsed with the `csv` crate (not a naive line split) so
//! quoted fields containing commas or embedded newlines are handled
//! correctly — that correctness is the whole reason this tool needs a
//! real CSV parser instead of reusing `text.diff`'s line-based approach.
//! Row alignment reuses the `similar` crate (already a workspace
//! dependency for `text.diff`) via its generic slice-diff algorithm,
//! treating each parsed row (`Vec<String>`) as one comparable unit.

use similar::{Algorithm, DiffOp, capture_diff_slices};
use upeg_core::tool;

/// Max number of sample rows returned in the `samples` array, across all
/// added/removed/changed rows combined. Keeps the JSON payload bounded
/// for large CSVs — the counts remain exact even when samples are
/// truncated.
const CSV_DIFF_SAMPLE_LIMIT: usize = 20;

/// `csv.diff` — row-level diff between two CSV texts.
///
/// Returns a JSON object: `added`/`removed`/`changed` row counts (exact,
/// never truncated) plus a `samples` array of up to
/// [`CSV_DIFF_SAMPLE_LIMIT`] example rows, each tagged
/// `{"kind": "added" | "removed" | "changed", ...}`.
#[tool(
    id = "csv.diff",
    display_label = "CSV diff",
    toolkit = "csv",
    description = "Diff two CSV texts row by row, reporting added/removed/changed counts plus sample rows.",
    inputs = [
        required left: String = "First CSV text",
        required right: String = "Second CSV text",
    ],
    outputs = [
        result: Json = "Added/removed/changed row counts plus up to 20 sample rows",
    ],
    pin = Launcher,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn csv_diff(left: &str, right: &str) -> Result<String, &'static str> {
    let left_rows = parse_csv_rows(left).map_err(|_| "left input is not valid csv")?;
    let right_rows = parse_csv_rows(right).map_err(|_| "right input is not valid csv")?;

    let mut tally = DiffTally::default();
    for op in capture_diff_slices(Algorithm::Myers, &left_rows, &right_rows) {
        tally.apply(op, &left_rows, &right_rows);
    }

    Ok(serde_json::json!({
        "added": tally.added,
        "removed": tally.removed,
        "changed": tally.changed,
        "samples": tally.samples,
    })
    .to_string())
}

/// Row-diff accumulator: exact counts plus a capped sample list.
#[derive(Default)]
struct DiffTally {
    added: usize,
    removed: usize,
    changed: usize,
    samples: Vec<serde_json::Value>,
}

impl DiffTally {
    fn apply(&mut self, op: DiffOp, left_rows: &[Vec<String>], right_rows: &[Vec<String>]) {
        match op {
            DiffOp::Equal { .. } => {}
            DiffOp::Delete {
                old_index, old_len, ..
            } => {
                self.removed += old_len;
                for row in &left_rows[old_index..old_index + old_len] {
                    self.push_sample(serde_json::json!({"kind": "removed", "row": row}));
                }
            }
            DiffOp::Insert {
                new_index, new_len, ..
            } => {
                self.added += new_len;
                for row in &right_rows[new_index..new_index + new_len] {
                    self.push_sample(serde_json::json!({"kind": "added", "row": row}));
                }
            }
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => self.apply_replace(
                old_index, old_len, new_index, new_len, left_rows, right_rows,
            ),
        }
    }

    /// A `Replace` op means the aligned rows differ; when the old/new
    /// spans have different lengths, the extra rows on the longer side
    /// are also structurally added/removed (not just "changed").
    #[allow(
        clippy::too_many_arguments,
        reason = "internal helper split out of `apply` purely to keep match arms short"
    )]
    fn apply_replace(
        &mut self,
        old_index: usize,
        old_len: usize,
        new_index: usize,
        new_len: usize,
        left_rows: &[Vec<String>],
        right_rows: &[Vec<String>],
    ) {
        let paired = old_len.min(new_len);
        self.changed += paired;
        for i in 0..paired {
            self.push_sample(serde_json::json!({
                "kind": "changed",
                "left": &left_rows[old_index + i],
                "right": &right_rows[new_index + i],
            }));
        }
        if old_len > paired {
            self.removed += old_len - paired;
            for row in &left_rows[old_index + paired..old_index + old_len] {
                self.push_sample(serde_json::json!({"kind": "removed", "row": row}));
            }
        } else if new_len > paired {
            self.added += new_len - paired;
            for row in &right_rows[new_index + paired..new_index + new_len] {
                self.push_sample(serde_json::json!({"kind": "added", "row": row}));
            }
        }
    }

    fn push_sample(&mut self, value: serde_json::Value) {
        if self.samples.len() < CSV_DIFF_SAMPLE_LIMIT {
            self.samples.push(value);
        }
    }
}

/// `csv.to_json` — parse CSV text (header row required) into a JSON array
/// of objects, one per data row, keyed by the header row's column names.
///
/// Unlike `csv.diff`'s headerless row-level comparison, this tool treats
/// the first row as a schema — that's the whole point of "CSV → JSON":
/// column names become object keys. All values stay strings (CSV carries
/// no type information); a consumer that needs numbers/booleans converts
/// downstream.
#[tool(
    id = "csv.to_json",
    display_label = "CSV → JSON",
    toolkit = "csv",
    description = "Parse CSV text (header row required) into a JSON array of objects.",
    inputs = [
        required input: String = "CSV text with a header row",
    ],
    outputs = [
        result: Json = "One JSON object per data row, keyed by header column names",
    ],
    pin = Launcher,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn csv_to_json(input: &str) -> Result<String, &'static str> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(input.as_bytes());
    let headers = reader
        .headers()
        .map_err(|_| "invalid csv header row")?
        .clone();

    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|_| "invalid csv row")?;
        let mut obj = serde_json::Map::with_capacity(headers.len());
        for (key, value) in headers.iter().zip(record.iter()) {
            obj.insert(
                key.to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
        rows.push(serde_json::Value::Object(obj));
    }
    serde_json::to_string(&rows).map_err(|_| "failed to serialize JSON")
}

/// `csv.select` — parse CSV text (header row required) and emit a new CSV
/// containing only the requested `columns`, reordered to match the
/// requested order. Unknown column names are rejected up front (before
/// any output is written) rather than silently dropped.
#[tool(
    id = "csv.select",
    display_label = "CSV select",
    toolkit = "csv",
    description = "Select a subset of columns (by header name) from CSV text, in the given order.",
    inputs = [
        required input: String = "CSV text with a header row",
        required columns: String = "Comma-separated column names to keep, e.g. \"a,c\"",
    ],
    pin = Launcher,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn csv_select(input: &str, columns: &str) -> Result<String, String> {
    let wanted: Vec<&str> = columns
        .split(',')
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .collect();
    if wanted.is_empty() {
        return Err("columns must not be empty".to_string());
    }

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(input.as_bytes());
    let headers = reader
        .headers()
        .map_err(|e| format!("invalid csv header row: {e}"))?
        .clone();

    let mut indices = Vec::with_capacity(wanted.len());
    for name in &wanted {
        let index = headers
            .iter()
            .position(|header| header == *name)
            .ok_or_else(|| format!("unknown column `{name}`"))?;
        indices.push(index);
    }

    let mut writer = csv::WriterBuilder::new().from_writer(Vec::new());
    writer
        .write_record(&wanted)
        .map_err(|e| format!("failed to write csv header: {e}"))?;
    for record in reader.records() {
        let record = record.map_err(|e| format!("invalid csv row: {e}"))?;
        let selected: Vec<&str> = indices
            .iter()
            .map(|&i| record.get(i).unwrap_or(""))
            .collect();
        writer
            .write_record(&selected)
            .map_err(|e| format!("failed to write csv row: {e}"))?;
    }
    let bytes = writer
        .into_inner()
        .map_err(|e| format!("failed to finalize csv output: {e}"))?;
    String::from_utf8(bytes).map_err(|_| "csv output is not valid UTF-8".to_string())
}

/// Parse CSV text into rows of string fields. `has_headers(false)`
/// treats every row uniformly — `csv.diff` reports structural row
/// differences, it doesn't assume a header schema. `flexible` stays at
/// its strict default (`false`): every row must share the first row's
/// field count, so a row with a differing column count is a genuine
/// CSV validity error rather than silently diffed — this is the
/// concrete case a real CSV parser catches that a naive line-based
/// diff (like `text.diff`) never would.
fn parse_csv_rows(text: &str) -> Result<Vec<Vec<String>>, csv::Error> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(text.as_bytes());
    let mut rows = Vec::new();
    for record in reader.records() {
        rows.push(record?.iter().map(str::to_string).collect());
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diff_json(left: &str, right: &str) -> serde_json::Value {
        let raw = csv_diff(left, right).unwrap();
        serde_json::from_str(&raw).unwrap()
    }

    #[test]
    fn csv_차이는_동일한_입력에_대해_변경_없음을_보고한다() {
        let csv_text = "a,b,c\n1,2,3\n";
        let v = diff_json(csv_text, csv_text);
        assert_eq!(v["added"], 0);
        assert_eq!(v["removed"], 0);
        assert_eq!(v["changed"], 0);
        assert!(v["samples"].as_array().unwrap().is_empty());
    }

    #[test]
    fn csv_차이는_추가된_행을_센다() {
        let v = diff_json("a,b\n1,2\n", "a,b\n1,2\n3,4\n");
        assert_eq!(v["added"], 1);
        assert_eq!(v["removed"], 0);
        assert_eq!(v["changed"], 0);
        let samples = v["samples"].as_array().unwrap();
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0]["kind"], "added");
        assert_eq!(samples[0]["row"], serde_json::json!(["3", "4"]));
    }

    #[test]
    fn csv_차이는_제거된_행을_센다() {
        let v = diff_json("a,b\n1,2\n3,4\n", "a,b\n1,2\n");
        assert_eq!(v["added"], 0);
        assert_eq!(v["removed"], 1);
        assert_eq!(v["changed"], 0);
        let samples = v["samples"].as_array().unwrap();
        assert_eq!(samples[0]["kind"], "removed");
        assert_eq!(samples[0]["row"], serde_json::json!(["3", "4"]));
    }

    #[test]
    fn csv_차이는_변경된_행을_센다() {
        let v = diff_json("a,b\n1,2\n", "a,b\n1,9\n");
        assert_eq!(v["added"], 0);
        assert_eq!(v["removed"], 0);
        assert_eq!(v["changed"], 1);
        let samples = v["samples"].as_array().unwrap();
        assert_eq!(samples[0]["kind"], "changed");
        assert_eq!(samples[0]["left"], serde_json::json!(["1", "2"]));
        assert_eq!(samples[0]["right"], serde_json::json!(["1", "9"]));
    }

    #[test]
    fn csv_차이는_따옴표로_묶인_쉼표와_줄바꿈을_열로서_올바르게_처리한다() {
        // A naive line-based diff would split this row's embedded comma
        // and newline as if they were structural — the whole reason
        // `csv.diff` uses a real CSV parser instead of `text.diff`.
        let left = "a,b\n\"hello, world\",\"multi\nline\"\n";
        let right = "a,b\n\"hello, world\",\"multi\nline\"\n";
        let v = diff_json(left, right);
        assert_eq!(v["added"], 0);
        assert_eq!(v["removed"], 0);
        assert_eq!(v["changed"], 0);
    }

    #[test]
    fn csv_차이는_한쪽_내부에서_불일치하는_열_개수를_잘못된_csv로_거부한다() {
        // Strict (non-`flexible`) parsing: every row within one CSV must
        // share the first row's field count. A row with a differing
        // column count is a genuine validity error, not silently diffed —
        // this is exactly the kind of structural check a naive
        // line-based diff (`text.diff`) would never catch.
        assert!(
            csv_diff("a,b\n1,2,3\n", "a,b\n1,2\n").is_err(),
            "ragged left input must be rejected"
        );
        assert!(
            csv_diff("a,b\n1,2\n", "a,b\n1,2,3\n").is_err(),
            "ragged right input must be rejected"
        );
    }

    #[test]
    fn csv_차이는_샘플_개수를_상한으로_제한하지만_카운트는_정확하다() {
        use std::fmt::Write as _;

        let left = String::new();
        let mut right = String::new();
        for i in 0..(CSV_DIFF_SAMPLE_LIMIT * 2) {
            writeln!(right, "row{i}").expect("writing to a String never fails");
        }
        let v = diff_json(&left, &right);
        assert_eq!(v["added"], CSV_DIFF_SAMPLE_LIMIT * 2);
        assert_eq!(
            v["samples"].as_array().unwrap().len(),
            CSV_DIFF_SAMPLE_LIMIT
        );
    }

    // ─── csv.to_json ────────────────────────────────────────────

    #[test]
    fn csv_json_변환은_헤더를_키로_사용한다() {
        let out = csv_to_json("a,b\n1,2\n3,4\n").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(
            v,
            serde_json::json!([
                {"a": "1", "b": "2"},
                {"a": "3", "b": "4"},
            ])
        );
    }

    #[test]
    fn csv_json_변환은_데이터_행이_없으면_빈_배열을_반환한다() {
        let out = csv_to_json("a,b\n").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v, serde_json::json!([]));
    }

    #[test]
    fn csv_json_변환은_따옴표로_묶인_쉼표를_열로서_올바르게_처리한다() {
        let out = csv_to_json("a,b\n\"hello, world\",2\n").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v, serde_json::json!([{"a": "hello, world", "b": "2"}]));
    }

    #[test]
    fn csv_json_변환은_열_개수가_어긋난_행을_거부한다() {
        assert!(csv_to_json("a,b\n1,2,3\n").is_err());
    }

    // ─── csv.select ─────────────────────────────────────────────

    #[test]
    fn csv_선택은_요청한_열만_요청한_순서로_남긴다() {
        let out = csv_select("a,b,c\n1,2,3\n4,5,6\n", "c,a").unwrap();
        assert_eq!(out, "c,a\n3,1\n6,4\n");
    }

    #[test]
    fn csv_선택은_열_이름_주변_공백을_잘라낸다() {
        let out = csv_select("a,b\n1,2\n", " a , b ").unwrap();
        assert_eq!(out, "a,b\n1,2\n");
    }

    #[test]
    fn csv_선택은_알수없는_열을_소문자_무점_오류로_거부한다() {
        match csv_select("a,b\n1,2\n", "a,z") {
            Err(msg) => {
                assert_eq!(msg, "unknown column `z`");
                assert!(!msg.ends_with('.'));
            }
            Ok(_) => panic!("expected unknown-column error"),
        }
    }

    #[test]
    fn csv_선택은_빈_열_목록을_거부한다() {
        assert!(csv_select("a,b\n1,2\n", "").is_err());
        assert!(csv_select("a,b\n1,2\n", "  ").is_err());
    }

    #[test]
    fn csv_선택은_단일_열로_축소할_수_있다() {
        let out = csv_select("a,b,c\n1,2,3\n", "b").unwrap();
        assert_eq!(out, "b\n2\n");
    }
}
