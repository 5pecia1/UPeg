use super::*;
use upeg_core::{BOARD_COLS, PinColorHex, Surface};
use upeg_runtime::pegboard::{placement_rect, placement_size, rects_overlap};

#[test]
fn 기본_상태는_공유_보드와_레이아웃을_가진다() {
    let state = default_state();
    let keys: Vec<&str> = state
        .boards
        .iter()
        .map(|board| board.key.as_str())
        .collect();
    assert_eq!(keys, vec!["dev", "trading", "personal"]);
    assert!(state.layouts.contains_key("dev"));
    assert!(state.layouts.contains_key("trading"));
    assert!(state.layouts.contains_key("personal"));
    assert!(
        state
            .layouts
            .get("trading")
            .is_some_and(|placements| placements.iter().any(|p| p.tool_id == "eth.gas")),
        "공유 기본 레이아웃은 GUI 전용 trading 메타데이터를 포함해야 한다"
    );
}

#[test]
fn 저장_로드_왕복은_알_수_없는_항목을_정리한다() {
    let root = std::env::temp_dir().join(format!("upeg-pegboard-{}", std::process::id()));
    let path = state_path_from_root(&root);
    let _ = std::fs::remove_dir_all(&root);
    let mut layouts = BTreeMap::new();
    layouts.insert(
        "dev".into(),
        vec![
            Placement::new("num.hex_to_decimal", 0, 0),
            Placement::new("missing.tool", 1, 0),
        ],
    );
    layouts.insert(
        "obsolete".into(),
        vec![Placement::new("num.hex_to_decimal", 0, 0)],
    );
    let state = PegboardState {
        boards: vec![BoardData {
            key: "dev".into(),
            title: "Dev".into(),
            guidance: upeg_core::BoardGuidance::default(),
        }],
        layouts,
        selection: PegboardSelection {
            board_key: Some(" dev ".into()),
            tag: " pure ".into(),
        },
    };

    save_state_to_path(&path, &state).expect("저장 성공");
    let loaded = load_state_from_path(&path).expect("로드 성공");

    assert_eq!(loaded.boards.len(), 1);
    assert_eq!(
        loaded.selection,
        PegboardSelection {
            board_key: Some("dev".into()),
            tag: "pure".into(),
        },
    );
    let dev = loaded.layouts.get("dev").cloned().unwrap_or_default();
    assert_eq!(dev.len(), 1);
    assert_eq!(dev[0].tool_id, "num.hex_to_decimal");
    assert!(!loaded.layouts.contains_key("obsolete"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn selection은_없는_board와_tag를_all로_정규화한다() {
    let state = PegboardState {
        boards: vec![BoardData {
            key: "dev".into(),
            title: "Dev".into(),
            guidance: upeg_core::BoardGuidance::default(),
        }],
        layouts: BTreeMap::from([(
            "dev".into(),
            vec![Placement::new("num.hex_to_decimal", 0, 0)],
        )]),
        selection: PegboardSelection {
            board_key: Some("missing".into()),
            tag: "__missing__".into(),
        },
    };

    let sanitized = sanitize_state(state);

    assert_eq!(sanitized.selection.board_key, None);
    assert_eq!(sanitized.selection.tag, ALL_TAG);
}

#[test]
fn state_change_rev는_저장할_때마다_증가하고_저장_전에는_none이다() {
    let root = std::env::temp_dir().join(format!("upeg-pegboard-rev-{}", std::process::id()));
    let path = state_path_from_root(&root);
    let _ = std::fs::remove_dir_all(&root);
    let state = PegboardState {
        boards: vec![BoardData {
            key: "dev".into(),
            title: "Dev".into(),
            guidance: upeg_core::BoardGuidance::default(),
        }],
        layouts: BTreeMap::from([(
            "dev".into(),
            vec![Placement::new("num.hex_to_decimal", 0, 0)],
        )]),
        selection: PegboardSelection::default(),
    };

    assert_eq!(
        state_change_rev_from_path(&path),
        None,
        "저장 전(파일 없음)에는 rev가 없어야 하고, 조회가 store를 만들어서도 안 된다"
    );
    assert!(!path.exists(), "rev 조회는 store 파일을 만들면 안 된다");

    save_state_to_path(&path, &state).expect("저장 성공");
    let first = state_change_rev_from_path(&path).expect("첫 저장 뒤 rev");

    save_state_to_path(&path, &state).expect("재저장 성공");
    let second = state_change_rev_from_path(&path).expect("재저장 뒤 rev");

    assert!(
        second > first,
        "쓰기마다 rev가 커져야 한다: {first} -> {second}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn pegboard_state_경로는_core_paths에_위임한다() {
    let source = include_str!("../pegboard.rs");
    let delegated = ["upeg_core", "paths", "store_path_in(root)"].join("::");
    assert!(
        source.contains(&delegated),
        "Pegboard state path must use the shared core path builder"
    );
}

#[test]
fn 배치_이동은_충돌한_항목을_앞으로_민다() {
    let mut state = PegboardState {
        boards: vec![BoardData {
            key: "dev".into(),
            title: "Dev".into(),
            guidance: upeg_core::BoardGuidance::default(),
        }],
        layouts: BTreeMap::from([(
            "dev".into(),
            vec![
                Placement::new("num.hex_to_decimal", 0, 0),
                Placement::new("id.uuid_v7", 1, 0),
            ],
        )]),
        selection: PegboardSelection::default(),
    };
    // id.uuid_v7을 num.hex_to_decimal 위로 옮긴다. 이동한 도구가 대상 셀을
    // 차지하고, 충돌한 도구는 앞으로 밀린다.
    assert!(move_placement(&mut state, "dev", "id.uuid_v7", 0, 0));
    let dev = state.layouts.get("dev").unwrap();
    let moved = dev.iter().find(|p| p.tool_id == "id.uuid_v7").unwrap();
    let pushed = dev
        .iter()
        .find(|p| p.tool_id == "num.hex_to_decimal")
        .unwrap();
    let (moved_w, _) = placement_size("id.uuid_v7");
    assert_eq!((moved.x, moved.y), (0, 0));
    assert_eq!((pushed.x, pushed.y), (moved_w, 0));
    assert!(
        dev.iter().enumerate().all(|(index, left)| {
            dev.iter()
                .skip(index + 1)
                .all(|right| !rects_overlap(placement_rect(left), placement_rect(right)))
        }),
        "밀어내기 이동은 겹치는 배치를 남기면 안 된다"
    );
}

#[test]
fn 모든_곳에서_도구를_제거하면_모든_보드가_비워진다() {
    let mut state = PegboardState {
        boards: vec![
            BoardData {
                key: "a".into(),
                title: "A".into(),
                guidance: upeg_core::BoardGuidance::default(),
            },
            BoardData {
                key: "b".into(),
                title: "B".into(),
                guidance: upeg_core::BoardGuidance::default(),
            },
        ],
        layouts: BTreeMap::from([
            ("a".into(), vec![Placement::new("num.hex_to_decimal", 0, 0)]),
            ("b".into(), vec![Placement::new("num.hex_to_decimal", 0, 0)]),
        ]),
        selection: PegboardSelection::default(),
    };
    assert!(remove_tool_everywhere(&mut state, "num.hex_to_decimal"));
    assert!(state.layouts.get("a").unwrap().is_empty());
    assert!(state.layouts.get("b").unwrap().is_empty());
}

#[test]
fn 배치_정리는_충돌을_첫_빈칸으로_조정한다() {
    // 같은 (x, y)에 있는 두 U1 위젯은 손상된 저장값이나 새로 겹치게 된
    // 매니페스트 크기 변경을 흉내 낸다. sanitize는 첫 번째 배치를 저장된
    // 좌표에 유지하고, 충돌한 항목을 다음 행 우선 빈칸으로 옮겨야 한다.
    let mut layouts = BTreeMap::new();
    layouts.insert(
        "dev".into(),
        vec![
            Placement::new("num.hex_to_decimal", 0, 0),
            Placement::new("id.uuid_v7", 0, 0), // collides
        ],
    );
    let state = PegboardState {
        boards: vec![BoardData {
            key: "dev".into(),
            title: "Dev".into(),
            guidance: upeg_core::BoardGuidance::default(),
        }],
        layouts,
        selection: PegboardSelection::default(),
    };
    let sanitized = sanitize_state(state);
    let dev = sanitized.layouts.get("dev").cloned().unwrap_or_default();
    assert_eq!(dev.len(), 2);
    let first = dev
        .iter()
        .find(|p| p.tool_id == "num.hex_to_decimal")
        .unwrap();
    let second = dev.iter().find(|p| p.tool_id == "id.uuid_v7").unwrap();
    assert_eq!(
        (first.x, first.y),
        (0, 0),
        "첫 번째 배치는 저장된 좌표를 유지해야 한다"
    );
    assert_ne!((second.x, second.y), (0, 0), "충돌은 조정되어야 한다");
    // 조정 후 두 배치는 겹치면 안 된다.
    assert!(!rects_overlap(
        placement_rect(first),
        placement_rect(second)
    ));
}

#[test]
fn 배치_정리는_6열_밖_항목을_고정_보드_안으로_옮긴다() {
    const BEYOND_BOARD_OFFSET: u16 = 4;

    // 저장된 x가 BOARD_COLS를 넘으면 sanitize는 가로 확장 대신
    // 고정 6열 안의 첫 유효 위치로 재배치해야 한다.
    let invalid_x = BOARD_COLS.saturating_add(BEYOND_BOARD_OFFSET);
    let mut layouts = BTreeMap::new();
    layouts.insert(
        "dev".into(),
        vec![Placement::new("num.hex_to_decimal", invalid_x, 0)],
    );
    let state = PegboardState {
        boards: vec![BoardData {
            key: "dev".into(),
            title: "Dev".into(),
            guidance: upeg_core::BoardGuidance::default(),
        }],
        layouts,
        selection: PegboardSelection::default(),
    };
    let sanitized = sanitize_state(state);
    let dev = sanitized.layouts.get("dev").unwrap();
    assert_eq!(dev.len(), 1);
    let p = &dev[0];
    let (w, _) = placement_size(&p.tool_id);
    assert_eq!(
        (p.x, p.y),
        (0, 0),
        "잘못 저장된 x는 고정 보드 안의 첫 유효 위치로 보정되어야 한다"
    );
    assert!(p.x.saturating_add(w) <= BOARD_COLS);
}

#[test]
fn 보드와_태그별_배치는_yx_정렬된_쌍을_반환한다() {
    // Vec 저장 순서를 일부러 (y, x) 순서와 다르게 두어 정렬이 빠지면
    // 테스트가 실패하게 한다.
    let state = PegboardState {
        boards: vec![BoardData {
            key: "dev".into(),
            title: "Dev".into(),
            guidance: upeg_core::BoardGuidance::default(),
        }],
        layouts: BTreeMap::from([(
            "dev".into(),
            vec![
                Placement::new("num.hex_to_decimal", 3, 1), // (y=1, x=3)
                Placement::new("id.uuid_v7", 0, 0),         // (y=0, x=0)
            ],
        )]),
        selection: PegboardSelection::default(),
    };
    let pairs = placements_for_board_and_tag_in(&state, Some("dev"), None);
    assert_eq!(pairs.len(), 2);
    assert_eq!(
        pairs[0].0.tool_id, "id.uuid_v7",
        "(y=0, x=0)이 먼저 정렬되어야 한다"
    );
    assert_eq!(pairs[1].0.tool_id, "num.hex_to_decimal");
    // 각 쌍은 tool_id가 일치하는 (Placement, ToolMeta)여야 한다.
    for (placement, tool) in &pairs {
        assert_eq!(placement.tool_id, tool.id);
    }
}

#[test]
fn 보드가_지정되지_않으면_보드와_태그별_배치는_합집합을_반환한다() {
    let state = PegboardState {
        boards: vec![
            BoardData {
                key: "a".into(),
                title: "A".into(),
                guidance: upeg_core::BoardGuidance::default(),
            },
            BoardData {
                key: "b".into(),
                title: "B".into(),
                guidance: upeg_core::BoardGuidance::default(),
            },
        ],
        layouts: BTreeMap::from([
            ("a".into(), vec![Placement::new("num.hex_to_decimal", 0, 0)]),
            (
                "b".into(),
                vec![
                    Placement::new("num.hex_to_decimal", 5, 5), // 중복 id는 제거된다.
                    Placement::new("id.uuid_v7", 0, 0),
                ],
            ),
        ]),
        selection: PegboardSelection::default(),
    };
    let pairs = placements_for_board_and_tag_in(&state, None, None);
    let ids: Vec<&str> = pairs.iter().map(|(_, t)| t.id).collect();
    // 중복 제거는 첫 항목인 board a의 num.hex_to_decimal를 유지한다.
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&"num.hex_to_decimal"));
    assert!(ids.contains(&"id.uuid_v7"));
}

#[test]
fn 보드와_태그별_배치는_태그로_필터링된다() {
    let state = PegboardState {
        boards: vec![BoardData {
            key: "dev".into(),
            title: "Dev".into(),
            guidance: upeg_core::BoardGuidance::default(),
        }],
        layouts: BTreeMap::from([(
            "dev".into(),
            vec![Placement::new("num.hex_to_decimal", 0, 0)],
        )]),
        selection: PegboardSelection::default(),
    };
    // "pure"는 num.hex_to_decimal와 일치해야 한다(레지스트리 사실).
    let with_tag = placements_for_board_and_tag_in(&state, Some("dev"), Some("pure"));
    assert!(
        with_tag.iter().all(|(_, t)| t.has_tag("pure")),
        "필터링된 집합에는 해당 태그가 있는 도구만 있어야 한다"
    );
    // 가짜 태그는 빈 결과를 만든다.
    let none = placements_for_board_and_tag_in(&state, Some("dev"), Some("__no-such-tag"));
    assert!(none.is_empty());
}

/// `board_entries_on_surface_in`은 CLI/HTTP/MCP가 공유하는 보드 열거
/// 진입점이다(app/board_scope 리팩터 전에는 세 표면이 각자
/// 재구현했고, CLI 쪽 사본만 `is_on_surface` 필터가 빠져 있었다).
/// `num.hex_to_decimal`은 기본 ALL_SURFACES, `memo.scratch`는
/// GUI_SURFACES(Desktop/Pwa/Ext)뿐이라 Cli/Mcp/Http에는 없다 — 정확히
/// 그 버그가 재현되던 조합이다.
#[test]
fn 표면_필터링된_보드_배치는_표면에_없는_핀을_제외한다() {
    let state = PegboardState {
        boards: vec![BoardData {
            key: "dev".into(),
            title: "Dev".into(),
            guidance: upeg_core::BoardGuidance::default(),
        }],
        layouts: BTreeMap::from([(
            "dev".into(),
            vec![
                Placement::new("num.hex_to_decimal", 0, 0),
                Placement::new("memo.scratch", 1, 0),
            ],
        )]),
        selection: PegboardSelection::default(),
    };

    for headless_surface in [Surface::Cli, Surface::Mcp, Surface::Http] {
        let ids: Vec<&str> = board_entries_on_surface_in(&state, "dev", None, headless_surface)
            .iter()
            .map(|(_, tool)| tool.id)
            .collect();
        assert_eq!(
            ids,
            vec!["num.hex_to_decimal"],
            "{headless_surface:?} 표면에는 GUI 전용 memo.scratch가 없어야 한다"
        );
    }

    let desktop_ids: Vec<&str> = board_entries_on_surface_in(&state, "dev", None, Surface::Desktop)
        .iter()
        .map(|(_, tool)| tool.id)
        .collect();
    assert!(
        desktop_ids.contains(&"memo.scratch") && desktop_ids.contains(&"num.hex_to_decimal"),
        "Desktop 표면은 두 핀 모두 봐야 한다: {desktop_ids:?}"
    );
}

#[test]
fn 표면_필터링된_보드_배치는_필터링_전_결과의_부분집합이다() {
    let state = default_state();
    let unfiltered = placements_for_board_and_tag_in(&state, Some("dev"), None);
    let filtered = board_entries_on_surface_in(&state, "dev", None, Surface::Cli);

    assert!(filtered.len() <= unfiltered.len());
    for (placement, tool) in &filtered {
        assert!(
            tool.is_on_surface(Surface::Cli),
            "필터링된 결과의 모든 도구는 그 표면에 있어야 한다"
        );
        assert!(
            unfiltered
                .iter()
                .any(|(p, t)| p.tool_id == placement.tool_id && t.id == tool.id),
            "필터링된 결과는 필터링 전 결과의 부분집합이어야 한다"
        );
    }
}

/// `board_placement_on_surface_in`(단일 도구 게이트)은
/// `board_entries_on_surface_in`(목록)과 같은 판정을 내려야 한다 —
/// "목록에는 없는데 호출은 된다"거나 그 반대가 되면 안 된다.
#[test]
fn 단일_배치_표면_게이트는_목록_함수와_판정이_일치한다() {
    let state = PegboardState {
        boards: vec![BoardData {
            key: "dev".into(),
            title: "Dev".into(),
            guidance: upeg_core::BoardGuidance::default(),
        }],
        layouts: BTreeMap::from([(
            "dev".into(),
            vec![
                Placement::new("num.hex_to_decimal", 0, 0),
                Placement::new("memo.scratch", 1, 0),
            ],
        )]),
        selection: PegboardSelection::default(),
    };

    assert!(
        board_placement_on_surface_in(&state, "dev", "num.hex_to_decimal", Surface::Cli).is_some(),
        "CLI 표면 도구는 단일 게이트도 통과해야 한다"
    );
    assert!(
        board_placement_on_surface_in(&state, "dev", "memo.scratch", Surface::Cli).is_none(),
        "GUI 전용 도구는 CLI 단일 게이트를 통과하면 안 된다(목록에서도 빠졌다)"
    );
    assert!(
        board_placement_on_surface_in(&state, "dev", "memo.scratch", Surface::Desktop).is_some(),
        "Desktop 표면에서는 memo.scratch도 통과해야 한다"
    );
    assert!(
        board_placement_on_surface_in(&state, "dev", "no.such.tool", Surface::Cli).is_none(),
        "핀되지 않은 도구는 표면과 무관하게 없어야 한다"
    );
}

#[test]
fn 보드와_태그_쿼리는_레이아웃_상태_형태를_사용한다() {
    // 구조적 불변식(ALL_TAG가 먼저 오고, has_tag/is_on_board 사실이 보존됨)은
    // 공유 기본 상태(default_state)를 명시적으로 넘기는 `_in` 변형으로
    // 검증한다. `load_state()`를 직접 거치면 개발자 로컬
    // `~/.upeg/pegboard-state.v2.json`의 내용(예: 이름이 바뀌기 전의 낡은
    // tool id)에 따라 결과가 달라져 CI에서는 통과하고 로컬에서는 실패하는
    // 환경 의존 테스트가 된다. `placements_for_board_and_tag_in`의 문서에
    // 적힌 이유(“병렬 테스트가 UPEG_HOME을 두고 경쟁하지 않도록”)와 같은
    // 이유로 여기서도 명시적 상태를 사용한다.
    let state = default_state();
    let tags = tag_options_for_board_in(&state, Some("dev"));
    assert_eq!(tags.first().map(String::as_str), Some(ALL_TAG));
    let tools = tools_for_board_and_tag_in(&state, Some("dev"), Some("pure"));
    assert!(
        tools
            .iter()
            .all(|tool| tool.has_tag("pure") && tool.is_on_board("dev")),
        "기본 dev + pure 쿼리는 두 레지스트리 사실을 모두 보존해야 한다"
    );

    // 그래도 디스크 경로 배선(공개 API가 실제로 `load_state()`를 거치는
    // 것) 자체는 여전히 실행해서 회귀를 잡아야 한다. 다만 디스크에 무엇이
    // 있든 항상 참이어야 하는 비교만 한다: 공개 API의 결과는 "같은 시점에
    // 읽은 load_state() 결과를 명시적으로 넘긴 `_in` 변형"의 결과와 항상
    // 구조적으로 일치해야 한다. 이 비교는 로컬 상태 파일의 내용(심지어
    // 오래되었거나 손상된 내용)에 좌우되지 않으므로 환경 의존성이 없다.
    let disk_state = load_state();
    assert_eq!(
        tag_options_for_board(Some("dev")),
        tag_options_for_board_in(&disk_state, Some("dev")),
        "공개 tag_options_for_board는 load_state()를 거치는 것 외에 _in 변형과 달라서는 안 된다"
    );
    let public_ids: Vec<&str> = tools_for_board_and_tag(Some("dev"), Some("pure"))
        .iter()
        .map(|tool| tool.id)
        .collect();
    let in_ids: Vec<&str> = tools_for_board_and_tag_in(&disk_state, Some("dev"), Some("pure"))
        .iter()
        .map(|tool| tool.id)
        .collect();
    assert_eq!(
        public_ids, in_ids,
        "공개 tools_for_board_and_tag는 load_state()를 거치는 것 외에 _in 변형과 달라서는 안 된다"
    );
}

// ─── 변경 헬퍼 ──────────────────────────────────────────
//
// 표면 간 편집 통합 1단계. Desktop과 TUI는 모든 상태 변경을 이 헬퍼로
// 실행해야 한다. 그래야 (a) 저장 형태가 항상 같은 방식으로 정리되고,
// (b) 표면별 코드의 갈라진 두 사본 대신 이후 버그를 고칠 지점이 하나가 된다.

fn fresh_state() -> PegboardState {
    default_state()
}

fn layout_ids_by_position(state: &PegboardState, board: &str) -> Vec<String> {
    let mut placements = state.layouts.get(board).cloned().unwrap_or_default();
    placements.sort_by_key(|p| (p.y, p.x));
    placements.into_iter().map(|p| p.tool_id).collect()
}

#[test]
fn slugify는_임의의_제목을_정규화한다() {
    assert_eq!(slugify(""), "board");
    assert_eq!(slugify("   "), "board");
    assert_eq!(slugify("Hello World"), "hello-world");
    assert_eq!(slugify("UPPER_case"), "upper-case");
    assert_eq!(slugify("special!!!chars"), "special-chars");
    assert_eq!(slugify("trailing---"), "trailing");
    assert_eq!(slugify("already-slug"), "already-slug");
}

#[test]
fn 보드_추가는_slugify하고_중복을_제거한다() {
    let mut state = fresh_state();
    let key = add_board(&mut state, "My Board!").expect("추가되어야 한다");
    assert_eq!(key, "my-board");
    assert!(state.boards.iter().any(|b| b.key == "my-board"));
    assert!(state.layouts.contains_key("my-board"));

    let key2 = add_board(&mut state, "My Board").expect("추가되어야 한다");
    assert_eq!(key2, "my-board-2");
}

#[test]
fn 보드_추가는_빈_제목을_거부한다() {
    let mut state = fresh_state();
    let before = state.boards.len();
    assert!(add_board(&mut state, "").is_none());
    assert!(add_board(&mut state, "   ").is_none());
    assert_eq!(state.boards.len(), before);
}

#[test]
fn 보드_제거는_레이아웃_항목도_제거한다() {
    let mut state = fresh_state();
    assert!(remove_board(&mut state, "dev"));
    assert!(!state.boards.iter().any(|b| b.key == "dev"));
    assert!(!state.layouts.contains_key("dev"));
    // 멱등성: 다시 제거하면 아무 일도 하지 않는다.
    assert!(!remove_board(&mut state, "dev"));
}

#[test]
fn 보드_이름변경은_제목을_바꾸고_키를_유지한다() {
    let mut state = fresh_state();
    assert!(rename_board(&mut state, "dev", "Development"));
    let dev = state.boards.iter().find(|b| b.key == "dev").unwrap();
    assert_eq!(dev.title, "Development");
    assert_eq!(dev.key, "dev");
}

#[test]
fn 보드_이름변경은_빈_제목이나_없는_보드를_거부한다() {
    let mut state = fresh_state();
    assert!(!rename_board(&mut state, "dev", ""));
    assert!(!rename_board(&mut state, "dev", "  "));
    assert!(!rename_board(&mut state, "nonexistent", "X"));
}

#[test]
fn 도구_고정은_멱등이며_알_수_없는_도구를_거부한다() {
    let mut state = fresh_state();
    let tool_id = "num.hex_to_decimal";
    // 고정 전이를 선명하게 관찰하도록 기본값을 제거한다.
    state
        .layouts
        .entry("dev".into())
        .or_default()
        .retain(|p| p.tool_id != tool_id);
    assert_eq!(pin_tool(&mut state, "dev", tool_id), PinAction::Pinned);
    assert_eq!(pin_tool(&mut state, "dev", tool_id), PinAction::NoOp);
    assert_eq!(
        pin_tool(&mut state, "dev", "no.such.tool.exists"),
        PinAction::NoOp
    );
    assert_eq!(pin_tool(&mut state, "no-board", tool_id), PinAction::NoOp);
}

#[test]
fn 도구_고정_해제는_결과를_보고한다() {
    let mut state = fresh_state();
    let tool_id = "num.hex_to_decimal";
    state
        .layouts
        .entry("dev".into())
        .or_default()
        .push(Placement::new(tool_id, 0, 0));
    assert_eq!(unpin_tool(&mut state, "dev", tool_id), PinAction::Unpinned);
    assert_eq!(unpin_tool(&mut state, "dev", tool_id), PinAction::NoOp);
    assert_eq!(unpin_tool(&mut state, "no-board", tool_id), PinAction::NoOp);
}

#[test]
fn pin_color_문자열_경계는_정규화하고_배치를_유지한다() {
    const RAW_COLOR: &str = "#12abef";
    const NORMALIZED_COLOR: &str = "#12ABEF";

    let mut state = fresh_state();
    let tool_id = "num.hex_to_decimal";
    state
        .layouts
        .insert("dev".into(), vec![Placement::new(tool_id, 0, 0)]);

    assert!(set_pin_color_hex(&mut state, "dev", tool_id, Some(RAW_COLOR)).expect("색상 설정"));
    let placement = state
        .layouts
        .get("dev")
        .and_then(|placements| {
            placements
                .iter()
                .find(|placement| placement.tool_id == tool_id)
        })
        .expect("배치 유지");
    assert_eq!(
        placement.color.as_ref().map(PinColorHex::as_str),
        Some(NORMALIZED_COLOR)
    );
    assert_eq!((placement.x, placement.y), (0, 0));

    assert!(set_pin_color_hex(&mut state, "dev", tool_id, None).expect("색상 초기화"));
    let placement = state
        .layouts
        .get("dev")
        .and_then(|placements| {
            placements
                .iter()
                .find(|placement| placement.tool_id == tool_id)
        })
        .expect("초기화 뒤에도 배치 유지");
    assert_eq!(placement.color, None);
}

#[test]
fn pin_color_저장_로드는_타입_색상을_왕복한다() {
    const RAW_COLOR: &str = "#aabbcc";
    const NORMALIZED_COLOR: &str = "#AABBCC";

    let root = std::env::temp_dir().join(format!("upeg-pin-color-{}", std::process::id()));
    let path = state_path_from_root(&root);
    let _ = std::fs::remove_dir_all(&root);
    let state = PegboardState {
        boards: vec![BoardData {
            key: "dev".into(),
            title: "Dev".into(),
            guidance: upeg_core::BoardGuidance::default(),
        }],
        layouts: BTreeMap::from([(
            "dev".into(),
            vec![
                Placement::new("num.hex_to_decimal", 0, 0)
                    .with_color(Some(PinColorHex::parse(RAW_COLOR).expect("테스트 색상"))),
            ],
        )]),
        selection: PegboardSelection::default(),
    };

    save_state_to_path(&path, &state).expect("색상 저장");
    let loaded = load_state_from_path(&path).expect("색상 로드");
    let placement = loaded
        .layouts
        .get("dev")
        .and_then(|placements| placements.first())
        .expect("저장된 배치");

    assert_eq!(
        placement.color.as_ref().map(PinColorHex::as_str),
        Some(NORMALIZED_COLOR)
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn 고정_토글은_상태를_뒤집는다() {
    let mut state = fresh_state();
    let tool_id = "num.hex_to_decimal";
    state
        .layouts
        .entry("dev".into())
        .or_default()
        .retain(|p| p.tool_id != tool_id);
    assert_eq!(toggle_pin(&mut state, "dev", tool_id), PinAction::Pinned);
    assert_eq!(toggle_pin(&mut state, "dev", tool_id), PinAction::Unpinned);
    assert_eq!(toggle_pin(&mut state, "dev", tool_id), PinAction::Pinned);
    assert_eq!(
        toggle_pin(&mut state, "dev", "no.such.tool"),
        PinAction::NoOp
    );
}

#[test]
fn 레이아웃_항목_이동은_범위_안에서_움직인다() {
    let mut state = fresh_state();
    let a = "convert.base64_encode";
    let b = "convert.base64_decode";
    let c = "convert.base32_encode";
    state.layouts.insert(
        "dev".into(),
        vec![
            Placement::new(a, 0, 0),
            Placement::new(b, 1, 0),
            Placement::new(c, 2, 0),
        ],
    );
    assert!(move_layout_entry(&mut state, "dev", b, Direction::Prev));
    assert_eq!(layout_ids_by_position(&state, "dev"), vec![b, a, c]);
    assert!(move_layout_entry(&mut state, "dev", b, Direction::Next));
    assert_eq!(layout_ids_by_position(&state, "dev"), vec![a, b, c]);
    // 시작 밖으로 나가면 아무 일도 하지 않는다.
    assert!(!move_layout_entry(&mut state, "dev", a, Direction::Prev));
    // 끝 밖으로 나가면 아무 일도 하지 않는다.
    assert!(!move_layout_entry(&mut state, "dev", c, Direction::Next));
    // 알 수 없는 도구/보드는 아무 일도 하지 않는다.
    assert!(!move_layout_entry(
        &mut state,
        "dev",
        "ghost",
        Direction::Prev
    ));
    assert!(!move_layout_entry(
        &mut state,
        "no-board",
        a,
        Direction::Next
    ));
}

#[test]
fn 레이아웃_쌍_교환은_교환하거나_아무_일도_하지_않는다() {
    let mut state = fresh_state();
    let a = "convert.base64_encode";
    let b = "convert.base64_decode";
    let c = "convert.base32_encode";
    state.layouts.insert(
        "dev".into(),
        vec![
            Placement::new(a, 0, 0),
            Placement::new(b, 1, 0),
            Placement::new(c, 2, 0),
        ],
    );
    assert!(swap_layout_pair(&mut state, "dev", a, c));
    assert_eq!(layout_ids_by_position(&state, "dev"), vec![c, b, a]);
    // 같은 id는 아무 일도 하지 않는다.
    assert!(!swap_layout_pair(&mut state, "dev", b, b));
    // 알 수 없는 id 쌍은 아무 일도 하지 않는다.
    assert!(!swap_layout_pair(&mut state, "dev", "ghost", a));
    assert!(!swap_layout_pair(&mut state, "no-board", a, c));
}

// 표면 간 왕복: TUI 변경 표면으로 적용한 편집은 데스크톱 UI가 읽는 것과
// 같은 디스크 형태에 도착한다. 테스트가 프로세스 env를 바꾸지 않아도 되도록
// save_state_to_path / load_state_from_path를 사용한다.
#[test]
fn tui_방식_편집은_desktop_로드_경로와_왕복한다() {
    let root = std::env::temp_dir().join(format!("upeg-pegboard-roundtrip-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let path = state_path_from_root(&root);

    // TUI 세션: 기본값에서 시작해 보드를 추가하고, 알려진 레지스트리 도구를
    // 고정한 뒤 한 칸 뒤로 재정렬한다.
    let mut tui_state = default_state();
    let new_key = add_board(&mut tui_state, "Project Alpha").expect("add_board 성공");
    assert_eq!(new_key, "project-alpha");
    let tool_id = "num.hex_to_decimal";
    assert_eq!(
        pin_tool(&mut tui_state, &new_key, tool_id),
        PinAction::Pinned
    );
    // move_layout_entry가 교환할 이웃을 갖도록 두 번째 도구를 고정한다.
    let second_id = "convert.base64_encode";
    assert_eq!(
        pin_tool(&mut tui_state, &new_key, second_id),
        PinAction::Pinned
    );
    assert!(move_layout_entry(
        &mut tui_state,
        &new_key,
        second_id,
        Direction::Prev,
    ));
    save_state_to_path(&path, &tui_state).expect("TUI save_state 성공");

    // Desktop 세션: 새 로드는 TUI가 쓴 모든 것을 읽어야 한다.
    let desktop_state = load_state_from_path(&path).expect("desktop load_state 성공");
    assert!(
        desktop_state.boards.iter().any(|b| b.key == new_key),
        "desktop은 TUI가 추가한 보드를 봐야 한다"
    );
    assert_eq!(
        layout_ids_by_position(&desktop_state, &new_key),
        vec![second_id.to_string(), tool_id.to_string()],
        "desktop은 TUI가 커밋한 것과 같은 고정 순서를 봐야 한다"
    );

    // Desktop 쪽 편집도 같은 방식으로 TUI에 돌아온다.
    let mut desktop_state = desktop_state;
    assert!(rename_board(&mut desktop_state, &new_key, "Project Beta",));
    assert_eq!(
        toggle_pin(&mut desktop_state, &new_key, tool_id),
        PinAction::Unpinned,
        "desktop 고정 해제가 등록되어야 한다"
    );
    save_state_to_path(&path, &desktop_state).expect("desktop save_state 성공");

    let tui_state_again = load_state_from_path(&path).expect("TUI 재로드 성공");
    let renamed = tui_state_again
        .boards
        .iter()
        .find(|b| b.key == new_key)
        .expect("보드 키는 이름 변경 후에도 유지되어야 한다");
    assert_eq!(renamed.title, "Project Beta");
    assert_eq!(
        layout_ids_by_position(&tui_state_again, &new_key),
        vec![second_id.to_string()],
        "TUI는 desktop 고정 해제를 봐야 한다"
    );

    let _ = std::fs::remove_dir_all(&root);
}
