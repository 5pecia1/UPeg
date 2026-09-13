use std::collections::BTreeMap;
use std::path::PathBuf;

use upeg_core::{ArgsPreset, BOARD_COLS, ColSpan, PinColorHex, PinSpan, Placement, RowSpan};

use super::{Store, schema};
use crate::pegboard::{BoardData, PegboardSelection, PegboardState};
use crate::store::export;

fn temp_store_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "upeg-store-{label}-{}-{:?}",
        std::process::id(),
        std::thread::current().id(),
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn store_db_path(dir: &std::path::Path) -> PathBuf {
    dir.join(upeg_core::paths::STORE_FILE)
}

fn board(key: &str, title: &str) -> BoardData {
    BoardData {
        key: key.into(),
        title: title.into(),
        guidance: upeg_core::BoardGuidance::default(),
    }
}

fn one_board_state(placements: Vec<Placement>) -> PegboardState {
    PegboardState {
        boards: vec![board("dev", "Dev")],
        layouts: BTreeMap::from([("dev".to_string(), placements)]),
        selection: PegboardSelection::default(),
    }
}

#[test]
fn user_version_러너는_마이그레이션을_적용하고_재실행에_멱등이다() {
    let store = Store::open_in_memory().expect("in-memory store");
    let version = schema::user_version(store.connection()).expect("user_version 조회");
    assert_eq!(
        version,
        schema::MIGRATIONS.len(),
        "fresh store는 모든 마이그레이션이 적용된 user_version이어야 한다"
    );

    // 파일 기반 재오픈: 이미 적용된 마이그레이션은 건너뛰어야 한다.
    let dir = temp_store_dir("migrate-idempotent");
    let path = store_db_path(&dir);
    drop(Store::open_at(&path).expect("첫 오픈"));
    let reopened =
        Store::open_at(&path).expect("재오픈은 마이그레이션을 다시 실행하지 않아야 한다");
    assert_eq!(
        schema::user_version(reopened.connection()).expect("재오픈 user_version"),
        schema::MIGRATIONS.len()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 스키마의_span_check는_core_상수와_일치한다() {
    // MIGRATIONS는 const 문자열이라 BOARD_COLS를 직접 삽입할 수 없다.
    // 이 테스트가 SQL 리터럴과 Rust 상수를 묶는다.
    let v1 = schema::MIGRATIONS[0];
    assert!(
        v1.contains(&format!("span_cols BETWEEN 1 AND {BOARD_COLS}")),
        "placements.span_cols CHECK는 BOARD_COLS({BOARD_COLS})와 같아야 한다"
    );
    assert!(v1.contains("span_rows >= 1"));
    for table in [
        schema::META_TABLE,
        schema::BOARDS_TABLE,
        schema::PLACEMENTS_TABLE,
        schema::SELECTION_TABLE,
    ] {
        assert!(
            v1.contains(&format!("CREATE TABLE {table} ")),
            "v1 스키마에 {table} 테이블이 있어야 한다"
        );
    }
}

#[test]
fn 스키마는_span_check_위반_행을_거부한다() {
    let store = Store::open_in_memory().expect("in-memory store");
    let insert = format!(
        "INSERT INTO {boards} (key, title, position, updated_at, device_id) \
         VALUES ('dev', 'Dev', 0, 0, 'test')",
        boards = schema::BOARDS_TABLE,
    );
    store.connection().execute(&insert, []).expect("보드 삽입");
    let bad_span = format!(
        "INSERT INTO {placements} \
           (board_key, tool_id, x, y, span_cols, span_rows, updated_at, device_id) \
         VALUES ('dev', 't', 0, 0, {over}, 1, 0, 'test')",
        placements = schema::PLACEMENTS_TABLE,
        over = BOARD_COLS + 1,
    );
    assert!(
        store.connection().execute(&bad_span, []).is_err(),
        "BOARD_COLS를 넘는 span_cols는 CHECK로 거부되어야 한다"
    );
}

#[test]
fn change_rev는_쓰기_전_none이고_쓰기마다_1씩_증가한다() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    assert_eq!(store.change_rev().expect("rev 조회"), None);

    store
        .save_state_global(&one_board_state(vec![Placement::new("a", 0, 0)]))
        .expect("첫 저장");
    assert_eq!(store.change_rev().expect("rev 조회"), Some(1));

    store
        .save_selection(&PegboardSelection::default())
        .expect("selection 저장");
    assert_eq!(store.change_rev().expect("rev 조회"), Some(2));
}

#[test]
fn device_id는_최초_오픈시_생성되고_재오픈에도_유지된다() {
    let dir = temp_store_dir("device-id");
    let path = store_db_path(&dir);
    let first = Store::open_at(&path)
        .expect("첫 오픈")
        .device_id()
        .expect("device_id 생성");
    assert!(
        uuid::Uuid::parse_str(&first).is_ok(),
        "device_id는 UUID여야 한다: {first}"
    );
    let second = Store::open_at(&path)
        .expect("재오픈")
        .device_id()
        .expect("device_id 유지");
    assert_eq!(first, second);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn open_at은_경로별로_격리된_상태를_가진다() {
    let dir_a = temp_store_dir("isolation-a");
    let dir_b = temp_store_dir("isolation-b");
    let mut store_a = Store::open_at(&store_db_path(&dir_a)).expect("store A");
    let store_b = Store::open_at(&store_db_path(&dir_b)).expect("store B");

    store_a
        .save_state_global(&one_board_state(vec![Placement::new("a", 0, 0)]))
        .expect("A 저장");

    assert_eq!(store_a.change_rev().expect("A rev"), Some(1));
    assert_eq!(store_b.change_rev().expect("B rev"), None);
    assert!(
        store_b
            .load_state_global()
            .expect("B 로드")
            .boards
            .is_empty()
    );
    let _ = std::fs::remove_dir_all(&dir_a);
    let _ = std::fs::remove_dir_all(&dir_b);
}

#[test]
fn 언핀된_placement는_tombstone되어_로드에서_걸러진다() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    store
        .save_state_global(&one_board_state(vec![
            Placement::new("keep.tool", 0, 0),
            Placement::new("drop.tool", 1, 0),
        ]))
        .expect("두 placement 저장");

    store
        .save_state_global(&one_board_state(vec![Placement::new("keep.tool", 0, 0)]))
        .expect("하나만 남긴 저장");

    let loaded = store.load_state_global().expect("로드");
    let dev = loaded.layouts.get("dev").expect("dev layout");
    assert_eq!(
        dev.iter().map(|p| p.tool_id.as_str()).collect::<Vec<_>>(),
        vec!["keep.tool"],
        "tombstone된 placement는 로드에 나타나면 안 된다"
    );

    // tombstone은 삭제가 아니다: 행 자체는 남아 있어야 한다(이후 sync 대비).
    let count: i64 = store
        .connection()
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM {placements}",
                placements = schema::PLACEMENTS_TABLE
            ),
            [],
            |row| row.get(0),
        )
        .expect("행 수 조회");
    assert_eq!(count, 2, "tombstone 행은 물리적으로 남아야 한다");
}

#[test]
fn 삭제된_보드는_tombstone되어_로드에서_걸러진다() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    store
        .save_state_global(&PegboardState {
            boards: vec![board("dev", "Dev"), board("lab", "Lab")],
            layouts: BTreeMap::from([
                ("dev".to_string(), vec![Placement::new("a", 0, 0)]),
                ("lab".to_string(), vec![Placement::new("b", 0, 0)]),
            ]),
            selection: PegboardSelection::default(),
        })
        .expect("두 보드 저장");

    store
        .save_state_global(&one_board_state(vec![Placement::new("a", 0, 0)]))
        .expect("lab 제거 저장");

    let loaded = store.load_state_global().expect("로드");
    assert_eq!(loaded.boards, vec![board("dev", "Dev")]);
    assert!(
        !loaded.layouts.contains_key("lab"),
        "tombstone된 보드의 layout은 로드되면 안 된다"
    );
}

#[test]
fn 빈_layout은_기본값으로_되살아나지_않고_빈_채로_왕복한다() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    store
        .save_state_global(&one_board_state(vec![Placement::new("a", 0, 0)]))
        .expect("저장");
    store
        .save_state_global(&one_board_state(Vec::new()))
        .expect("모두 언핀 저장");

    let loaded = store.load_state_global().expect("로드");
    assert_eq!(
        loaded.layouts.get("dev").map(Vec::len),
        Some(0),
        "빈 layout은 명시적 빈 항목으로 로드되어야 한다"
    );
}

#[test]
fn span과_args_preset과_색상은_store를_통해_왕복한다() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    let placement = Placement::new("styled.tool", 2, 1)
        .with_color(Some(PinColorHex::parse("#12abef").expect("테스트 색상")))
        .with_span(Some(PinSpan::new(
            ColSpan::new(3).expect("테스트 cols"),
            RowSpan::new(2).expect("테스트 rows"),
        )))
        .with_args_preset(Some(
            ArgsPreset::parse(r#"{"city":"Seoul","days":3}"#).expect("테스트 preset"),
        ));
    store
        .save_state_global(&one_board_state(vec![placement.clone()]))
        .expect("저장");

    let loaded = store.load_state_global().expect("로드");
    assert_eq!(
        loaded.layouts.get("dev").map(Vec::as_slice),
        Some(std::slice::from_ref(&placement)),
        "color/span/args_preset이 손실 없이 왕복해야 한다"
    );
}

#[test]
fn selection은_단일_행으로_왕복한다() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    let mut state = one_board_state(vec![Placement::new("a", 0, 0)]);
    state.selection = PegboardSelection {
        board_key: Some("dev".into()),
        tag: "pure".into(),
    };
    store.save_state_global(&state).expect("저장");

    assert_eq!(
        store.load_state_global().expect("로드").selection,
        state.selection
    );

    store
        .save_selection(&PegboardSelection::default())
        .expect("selection 교체");
    assert_eq!(
        store.load_state_global().expect("재로드").selection,
        PegboardSelection::default()
    );
}

#[test]
fn 행단위_upsert와_tombstone은_load_state에_반영된다() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    store
        .save_state_global(&one_board_state(Vec::new()))
        .expect("보드 준비");

    store
        .upsert_placement("dev", &Placement::new("row.tool", 4, 2))
        .expect("행 단위 upsert");
    let loaded = store.load_state_global().expect("로드");
    assert_eq!(
        loaded.layouts.get("dev").and_then(|l| l.first()),
        Some(&Placement::new("row.tool", 4, 2))
    );

    store
        .tombstone_placement("dev", "row.tool")
        .expect("행 단위 tombstone");
    assert_eq!(
        store
            .load_state_global()
            .expect("재로드")
            .layouts
            .get("dev")
            .map(Vec::len),
        Some(0)
    );
}

#[test]
fn wal_모드에서_두_커넥션이_병행으로_써도_모든_쓰기가_남는다() {
    const WRITES_PER_CONNECTION: u64 = 5;

    let dir = temp_store_dir("wal-concurrency");
    let path = store_db_path(&dir);
    // 스키마/디바이스 준비를 먼저 끝내 두 스레드가 곧장 쓰기 경쟁만 하게 한다.
    drop(Store::open_at(&path).expect("스키마 준비"));

    let writer = |tool_prefix: &'static str, path: PathBuf| {
        std::thread::spawn(move || {
            let mut store = Store::open_at(&path).expect("스레드 store 오픈");
            for index in 0..WRITES_PER_CONNECTION {
                store
                    .upsert_placement_bootstrap(tool_prefix, index)
                    .expect("병행 쓰기 성공");
            }
        })
    };
    let a = writer("alpha", path.clone());
    let b = writer("beta", path.clone());
    a.join().expect("alpha 스레드");
    b.join().expect("beta 스레드");

    let store = Store::open_at(&path).expect("검증용 오픈");
    assert_eq!(
        store.change_rev().expect("rev"),
        Some(WRITES_PER_CONNECTION * 2),
        "busy_timeout 아래에서 어떤 쓰기도 유실되면 안 된다"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

impl Store {
    /// WAL 동시성 테스트 전용: 보드 upsert + placement upsert를 한
    /// 트랜잭션으로 묶어 실제 쓰기 경합을 만든다.
    fn upsert_placement_bootstrap(
        &mut self,
        prefix: &'static str,
        index: u64,
    ) -> Result<(), super::StoreError> {
        let device_id = self.device_id()?;
        let now = super::epoch_millis_now();
        self.write_tx(|tx| {
            let board_sql = format!(
                "INSERT OR IGNORE INTO {boards} (key, title, position, updated_at, device_id) \
                 VALUES (?1, ?1, 0, ?2, ?3)",
                boards = schema::BOARDS_TABLE,
            );
            tx.execute(&board_sql, rusqlite::params![prefix, now, device_id])?;
            let placement_sql = format!(
                "INSERT OR REPLACE INTO {placements} \
                   (board_key, tool_id, x, y, updated_at, device_id) \
                 VALUES (?1, ?2, 0, 0, ?3, ?4)",
                placements = schema::PLACEMENTS_TABLE,
            );
            tx.execute(
                &placement_sql,
                rusqlite::params![prefix, format!("{prefix}.{index}"), now, device_id],
            )?;
            Ok(())
        })
    }
}

fn log_record(tool_id: &str, started_at_ms: u64) -> super::ExecutionLogRecord {
    super::ExecutionLogRecord {
        schema_version: 1,
        started_at_ms,
        tool_id: tool_id.to_string(),
        toolkit: String::new(),
        invoker: "function".to_string(),
        surface: "cli".to_string(),
        board: None,
        trigger: None,
        principal: None,
        status: "ok".to_string(),
        duration_ms: 1,
    }
}

#[test]
fn v4_스키마는_execution_log에_principal_열을_덧붙인다() {
    // append-only 이력이므로 테이블을 다시 만들지 않고 열만 추가한다 —
    // 이전 행은 principal이 NULL로 남아야 하고, 지어내면 안 된다.
    let v4 = schema::MIGRATIONS[3];
    assert!(
        v4.contains(&format!(
            "ALTER TABLE {} ADD COLUMN principal TEXT",
            schema::EXECUTION_LOG_TABLE
        )),
        "v4는 principal 열을 ALTER TABLE로 추가해야 한다: {v4}"
    );
}

#[test]
fn principal은_기록하고_읽는_동안_보존된다() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    let mut record = log_record("principal.tool", 10);
    record.principal = Some("agent".to_string());
    store.append_execution_record(&record).expect("append");

    let read = store
        .read_execution_records(&super::ExecutionLogFilter::default())
        .expect("read");

    assert_eq!(read, vec![record]);
}

#[test]
fn principal이_없는_기록은_null로_남는다() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    let record = log_record("principal.absent", 10);
    store.append_execution_record(&record).expect("append");

    let read = store
        .read_execution_records(&super::ExecutionLogFilter::default())
        .expect("read");

    assert_eq!(read[0].principal, None);
}

#[test]
fn v2_스키마는_memos와_execution_log_테이블을_만든다() {
    let v2 = schema::MIGRATIONS[1];
    for table in [schema::MEMOS_TABLE, schema::EXECUTION_LOG_TABLE] {
        assert!(
            v2.contains(&format!("CREATE TABLE {table} ")),
            "v2 스키마에 {table} 테이블이 있어야 한다"
        );
    }
    assert!(
        v2.contains(&format!(
            "ON {} (started_at_ms)",
            schema::EXECUTION_LOG_TABLE
        )),
        "v2에 started_at_ms 인덱스가 있어야 한다"
    );
    assert!(
        v2.contains(&format!(
            "ON {} (tool_id, started_at_ms)",
            schema::EXECUTION_LOG_TABLE
        )),
        "v2에 (tool_id, started_at_ms) 인덱스가 있어야 한다"
    );
}

#[test]
fn 메모_tombstone은_행을_남기고_로드에서_걸러진다() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    let mut memos = BTreeMap::from([
        ("keep".to_string(), "body".to_string()),
        ("drop".to_string(), "gone".to_string()),
    ]);
    store.save_memos(&memos).expect("두 메모 저장");

    memos.remove("drop");
    store.save_memos(&memos).expect("하나 제거 저장");

    assert_eq!(store.load_memos().expect("로드"), memos);
    let count: i64 = store
        .connection()
        .query_row(
            &format!("SELECT COUNT(*) FROM {}", schema::MEMOS_TABLE),
            [],
            |row| row.get(0),
        )
        .expect("행 수 조회");
    assert_eq!(count, 2, "tombstone 행은 물리적으로 남아야 한다");
}

#[test]
fn 메모_동일_내용_재저장은_updated_at을_움직이지_않는다() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    let memos = BTreeMap::from([("scratch".to_string(), "hello".to_string())]);
    store.save_memos(&memos).expect("첫 저장");
    let first: i64 = store
        .connection()
        .query_row(
            &format!(
                "SELECT updated_at FROM {} WHERE key = 'scratch'",
                schema::MEMOS_TABLE
            ),
            [],
            |row| row.get(0),
        )
        .expect("updated_at 조회");

    std::thread::sleep(std::time::Duration::from_millis(5));
    store.save_memos(&memos).expect("동일 내용 재저장");
    let second: i64 = store
        .connection()
        .query_row(
            &format!(
                "SELECT updated_at FROM {} WHERE key = 'scratch'",
                schema::MEMOS_TABLE
            ),
            [],
            |row| row.get(0),
        )
        .expect("updated_at 재조회");
    assert_eq!(
        first, second,
        "변경 없는 행의 updated_at은 움직이면 안 된다"
    );
}

#[test]
fn 실행_로그는_필터와_제한으로_읽고_append_순서를_유지한다() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    store
        .append_execution_record(&log_record("a.one", 1))
        .expect("append 1");
    let mut http_record = log_record("b.two", 2);
    http_record.surface = "http".to_string();
    http_record.trigger = Some("hotkey".to_string());
    http_record.status = "tool_error".to_string();
    store
        .append_execution_record(&http_record)
        .expect("append 2");
    store
        .append_execution_record(&log_record("c.three", 3))
        .expect("append 3");

    let filtered = store
        .read_execution_records(&super::ExecutionLogFilter {
            surface: Some("http".into()),
            trigger: Some("hotkey".into()),
            status: Some("tool_error".into()),
            since_ms: Some(2),
            ..super::ExecutionLogFilter::default()
        })
        .expect("필터 읽기");
    assert_eq!(filtered, vec![http_record]);

    let limited = store
        .read_execution_records(&super::ExecutionLogFilter {
            limit: Some(2),
            ..super::ExecutionLogFilter::default()
        })
        .expect("제한 읽기");
    assert_eq!(
        limited
            .iter()
            .map(|record| record.tool_id.as_str())
            .collect::<Vec<_>>(),
        vec!["b.two", "c.three"],
        "limit은 최신 행을 남기되 append 순서로 반환해야 한다"
    );
}

#[test]
fn 실행_로그_recent_signal은_newest_first_unique다() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    for (tool_id, ts) in [
        ("alpha.one", 10),
        ("beta.two", 30),
        ("alpha.one", 50),
        ("gamma.three", 40),
    ] {
        store
            .append_execution_record(&log_record(tool_id, ts))
            .expect("append");
    }

    let signals = store.recent_execution_search_signals().expect("signals");
    assert!(signals.pinned.is_empty());
    assert_eq!(
        signals
            .recent
            .iter()
            .map(|signal| (signal.tool_id.as_str(), signal.rank))
            .collect::<Vec<_>>(),
        vec![("alpha.one", 0), ("gamma.three", 1), ("beta.two", 2)]
    );
}

#[test]
fn 실행_로그_동시_타임스탬프는_나중_삽입이_먼저_온다() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    store
        .append_execution_record(&log_record("first.tool", 7))
        .expect("append 1");
    store
        .append_execution_record(&log_record("second.tool", 7))
        .expect("append 2");

    let signals = store.recent_execution_search_signals().expect("signals");
    assert_eq!(
        signals
            .recent
            .iter()
            .map(|signal| signal.tool_id.as_str())
            .collect::<Vec<_>>(),
        vec!["second.tool", "first.tool"],
        "같은 started_at_ms면 삽입 순서가 늦은 쪽이 최신이다"
    );
}

#[test]
fn 두_커넥션이_병행으로_실행_로그를_append해도_유실되지_않는다() {
    const APPENDS_PER_CONNECTION: usize = 5;

    let dir = temp_store_dir("log-append-concurrency");
    let path = store_db_path(&dir);
    // 스키마/디바이스 준비를 먼저 끝내 두 스레드가 곧장 쓰기 경쟁만 하게 한다.
    drop(Store::open_at(&path).expect("스키마 준비"));

    let writer = |tool_prefix: &'static str, path: PathBuf| {
        std::thread::spawn(move || {
            // CLI dispatch와 HTTP 데몬처럼 별도 커넥션에서 병행 append.
            let mut store = Store::open_at(&path).expect("스레드 store 오픈");
            for index in 0..APPENDS_PER_CONNECTION {
                let started = u64::try_from(index).unwrap_or_default();
                store
                    .append_execution_record(&log_record(
                        &format!("{tool_prefix}.{index}"),
                        started,
                    ))
                    .expect("busy_timeout 아래에서 append는 성공해야 한다");
            }
        })
    };
    let a = writer("alpha", path.clone());
    let b = writer("beta", path.clone());
    a.join().expect("alpha 스레드");
    b.join().expect("beta 스레드");

    let store = Store::open_at(&path).expect("검증용 오픈");
    let records = store
        .read_execution_records(&super::ExecutionLogFilter::default())
        .expect("전체 읽기");
    assert_eq!(
        records.len(),
        APPENDS_PER_CONNECTION * 2,
        "병행 append에서 어떤 기록도 유실되면 안 된다"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 정규_내보내기는_보드_안내와_배치_형태를_보존한다() {
    // 안내의 기본값, BTreeMap 키 정렬, None 필드 생략을 보존한다.
    let boards = vec![board("dev", "Dev"), board("trading", "Trading")];
    assert_eq!(
        export::boards_to_json(&boards).expect("boards 직렬화"),
        r#"[{"key":"dev","title":"Dev","guidance":{"description":"","instructions":""}},{"key":"trading","title":"Trading","guidance":{"description":"","instructions":""}}]"#,
    );

    let layouts = BTreeMap::from([
        ("zeta".to_string(), vec![Placement::new("plain.tool", 1, 0)]),
        (
            "alpha".to_string(),
            vec![
                Placement::new("styled.tool", 0, 0)
                    .with_color(Some(PinColorHex::parse("#AABBCC").expect("테스트 색상")))
                    .with_span(Some(PinSpan::new(
                        ColSpan::new(2).expect("테스트 cols"),
                        RowSpan::new(1).expect("테스트 rows"),
                    )))
                    .with_args_preset(Some(
                        ArgsPreset::parse(r#"{"n":1}"#).expect("테스트 preset"),
                    )),
            ],
        ),
    ]);
    let json = export::layouts_to_json(&layouts).expect("layouts 직렬화");
    assert_eq!(
        json,
        concat!(
            r##"{"alpha":[{"tool_id":"styled.tool","x":0,"y":0,"color":"#AABBCC","##,
            r#""span":{"cols":2,"rows":1},"args_preset":{"n":1}}],"#,
            r#""zeta":[{"tool_id":"plain.tool","x":1,"y":0}]}"#,
        ),
        "BTreeMap 정렬 + None 생략의 canonical 형태가 유지되어야 한다"
    );

    // import 방향도 동일 형태를 되돌린다.
    assert_eq!(
        export::layouts_from_json(&json).expect("layouts 역직렬화"),
        layouts
    );
    assert_eq!(
        export::boards_from_json(&export::boards_to_json(&boards).expect("직렬화"))
            .expect("boards 역직렬화"),
        boards
    );
}

// ─── last_outcomes (schema v3) ─────────────────────────────────────────

use crate::store::{NewLastOutcome, OUTPUTS_JSON_MAX_BYTES, cap_outputs_json};

fn outcome(tool_id: &str, ok: bool) -> NewLastOutcome {
    NewLastOutcome {
        tool_id: tool_id.into(),
        ok,
        primary_output_id: Some("value".into()),
        outputs_json: r#"[{"id":"value","kind":"string","value":{"String":{"value":"42"}}}]"#
            .into(),
        error_code: if ok { None } else { Some("E_FAIL".into()) },
        error_message: if ok { None } else { Some("boom".into()) },
    }
}

#[test]
fn v3_스키마는_last_outcomes_테이블을_placement_키로_만든다() {
    let v3 = schema::MIGRATIONS[2];
    assert!(
        v3.contains(&format!("CREATE TABLE {} ", schema::LAST_OUTCOMES_TABLE)),
        "v3 스키마에 last_outcomes 테이블이 있어야 한다"
    );
    assert!(
        v3.contains("PRIMARY KEY (board_key, tool_id)"),
        "last_outcomes는 placement 단위 (board_key, tool_id) 키여야 한다"
    );
}

#[test]
fn 크기_상한_이내의_outputs_json은_절단_없이_통과한다() {
    let json = r#"[{"id":"value","v":1}]"#;
    let capped = cap_outputs_json(json, Some("value"));
    assert_eq!(capped.json, json);
    assert!(!capped.truncated);
}

#[test]
fn 크기_상한_초과시_primary_output만_남기고_truncated를_표시한다() {
    let filler = "x".repeat(OUTPUTS_JSON_MAX_BYTES);
    let json = format!(r#"[{{"id":"value","v":"ok"}},{{"id":"noise","v":"{filler}"}}]"#);
    let capped = cap_outputs_json(&json, Some("value"));
    assert_eq!(capped.json, r#"[{"id":"value","v":"ok"}]"#);
    assert!(capped.truncated);

    // primary id가 없으면 첫 엔트리가 프리뷰가 된다.
    let no_primary = cap_outputs_json(&json, None);
    assert_eq!(no_primary.json, r#"[{"id":"value","v":"ok"}]"#);
    assert!(no_primary.truncated);
}

#[test]
fn primary_output조차_상한을_넘으면_빈_배열로_절단한다() {
    let filler = "x".repeat(OUTPUTS_JSON_MAX_BYTES + 1);
    let json = format!(r#"[{{"id":"value","v":"{filler}"}}]"#);
    let capped = cap_outputs_json(&json, Some("value"));
    assert_eq!(capped.json, "[]");
    assert!(capped.truncated);

    // 배열이 아닌(손상된) 페이로드도 안전하게 빈 배열로 떨어진다.
    let not_array = format!(r#""{filler}""#);
    let corrupt = cap_outputs_json(&not_array, Some("value"));
    assert_eq!(corrupt.json, "[]");
    assert!(corrupt.truncated);
}

#[test]
fn last_outcome은_record_load로_왕복하고_같은_키에_덮어쓴다() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    store
        .record_last_outcome("dev", &outcome("num.hex", true))
        .expect("성공 기록");
    store
        .record_last_outcome("dev", &outcome("net.ping", false))
        .expect("실패 기록");

    let loaded = store.load_last_outcomes("dev").expect("로드");
    assert_eq!(loaded.len(), 2, "tool_id 정렬로 두 행이 보여야 한다");
    let ping = &loaded[0];
    assert_eq!(ping.tool_id, "net.ping");
    assert!(!ping.ok);
    assert_eq!(ping.error_code.as_deref(), Some("E_FAIL"));
    assert_eq!(ping.error_message.as_deref(), Some("boom"));
    let hex = &loaded[1];
    assert_eq!(hex.tool_id, "num.hex");
    assert!(hex.ok);
    assert!(!hex.truncated);
    assert!(hex.updated_at_ms > 0);
    assert_eq!(hex.outputs_json, outcome("num.hex", true).outputs_json);

    // 같은 (board, tool) 키에 다시 기록하면 덮어쓴다.
    store
        .record_last_outcome("dev", &outcome("num.hex", false))
        .expect("덮어쓰기");
    let reloaded = store.load_last_outcomes("dev").expect("재로드");
    assert_eq!(reloaded.len(), 2);
    assert!(!reloaded[1].ok, "최신 기록이 이전 기록을 대체해야 한다");
}

#[test]
fn last_outcome은_board_키로_격리된다() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    store
        .record_last_outcome("dev", &outcome("num.hex", true))
        .expect("dev 기록");
    store
        .record_last_outcome("trading", &outcome("num.hex", false))
        .expect("trading 기록");

    assert!(store.load_last_outcomes("dev").expect("dev 로드")[0].ok);
    assert!(!store.load_last_outcomes("trading").expect("trading 로드")[0].ok);
    assert!(
        store
            .load_last_outcomes("personal")
            .expect("빈 보드")
            .is_empty()
    );
}

#[test]
fn 상한_초과_기록은_truncated_플래그와_함께_저장된다() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    let filler = "x".repeat(OUTPUTS_JSON_MAX_BYTES);
    let big = NewLastOutcome {
        outputs_json: format!(r#"[{{"id":"value","v":"ok"}},{{"id":"noise","v":"{filler}"}}]"#),
        ..outcome("big.tool", true)
    };
    store.record_last_outcome("dev", &big).expect("기록");

    let loaded = store.load_last_outcomes("dev").expect("로드");
    assert!(loaded[0].truncated);
    assert_eq!(loaded[0].outputs_json, r#"[{"id":"value","v":"ok"}]"#);
}

#[test]
fn clear_last_outcome은_해당_행만_삭제한다() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    store
        .record_last_outcome("dev", &outcome("num.hex", true))
        .expect("기록 1");
    store
        .record_last_outcome("dev", &outcome("net.ping", true))
        .expect("기록 2");

    store.clear_last_outcome("dev", "num.hex").expect("clear");
    let remaining = store.load_last_outcomes("dev").expect("로드");
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].tool_id, "net.ping");
}

#[test]
fn 핀_unpin은_tombstone과_함께_last_outcome도_정리한다() {
    let mut store = Store::open_in_memory().expect("in-memory store");
    store
        .save_state_global(&one_board_state(vec![
            Placement::new("num.hex", 0, 0),
            Placement::new("net.ping", 1, 0),
        ]))
        .expect("보드 준비");
    store
        .record_last_outcome("dev", &outcome("num.hex", true))
        .expect("기록 1");
    store
        .record_last_outcome("dev", &outcome("net.ping", true))
        .expect("기록 2");

    // 행 단위 tombstone 경로.
    store
        .tombstone_placement("dev", "num.hex")
        .expect("행 단위 unpin");
    let after_row = store.load_last_outcomes("dev").expect("로드");
    assert_eq!(
        after_row
            .iter()
            .map(|o| o.tool_id.as_str())
            .collect::<Vec<_>>(),
        vec!["net.ping"],
        "tombstone된 핀의 outcome은 같은 트랜잭션에서 삭제되어야 한다"
    );

    // save_state 전체 저장(diff→tombstone) 경로도 동일하게 정리한다.
    store
        .save_state_global(&one_board_state(Vec::new()))
        .expect("전체 unpin");
    assert!(
        store.load_last_outcomes("dev").expect("재로드").is_empty(),
        "save_state 경로의 unpin도 outcome을 정리해야 한다"
    );
}
