use super::*;

#[test]
fn 보드_안내는_제이슨_왕복에서_유지된다() {
    let json = r##"[{"key":"dev","title":"Dev","guidance":{"description":"개발 작업","instructions":"# 순서\n\n    cargo test\n"}}]"##;
    let boards = export::boards_from_json(json).expect("보드 읽기");
    let exported: serde_json::Value =
        serde_json::from_str(&export::boards_to_json(&boards).expect("보드 내보내기"))
            .expect("제이슨 읽기");

    assert_eq!(exported[0]["guidance"]["description"], "개발 작업");
    assert_eq!(
        exported[0]["guidance"]["instructions"],
        "# 순서\n\n    cargo test\n"
    );
}

#[test]
fn 생략한_보드_안내는_빈_문자열로_읽힌다() {
    let boards = export::boards_from_json(r#"[{"key":"dev","title":"Dev"}]"#).expect("보드 읽기");
    let exported: serde_json::Value =
        serde_json::from_str(&export::boards_to_json(&boards).expect("보드 내보내기"))
            .expect("제이슨 읽기");

    assert_eq!(exported[0]["guidance"]["description"], "");
    assert_eq!(exported[0]["guidance"]["instructions"], "");
}

#[test]
fn 보드_안내의_생략한_필드는_빈_문자열로_읽힌다() {
    for (json, description, instructions) in [
        (
            r#"[{"key":"dev","title":"Dev","guidance":{"description":"설명만"}}]"#,
            "설명만",
            "",
        ),
        (
            r#"[{"key":"dev","title":"Dev","guidance":{"instructions":"지침만"}}]"#,
            "",
            "지침만",
        ),
    ] {
        let boards = export::boards_from_json(json).expect("보드 읽기");
        assert_eq!(boards[0].guidance.description, description);
        assert_eq!(boards[0].guidance.instructions, instructions);
    }
}

#[test]
fn 개인_보드_안내는_저장과_재로드에서_유지된다() {
    let root = std::env::temp_dir().join(format!("upeg-guidance-roundtrip-{}", std::process::id()));
    let path = state_path_from_root(&root);
    let _ = std::fs::remove_dir_all(&root);
    let visibility = BoardVisibility::global_only();
    let mut state = default_state();
    state.boards = export::boards_from_json(
        r##"[{"key":"dev","title":"Dev","guidance":{"description":"개발 작업","instructions":"# 순서\n\n    cargo test\n"}}]"##,
    ).expect("보드 읽기");
    save_state_to_path_in(&path, &state, &visibility).expect("저장");
    let loaded = load_state_from_path_in(&path, &visibility).expect("재로드");
    let exported: serde_json::Value =
        serde_json::from_str(&export::boards_to_json(&loaded.boards).expect("내보내기"))
            .expect("제이슨 읽기");

    assert_eq!(exported[0]["guidance"]["description"], "개발 작업");
    assert_eq!(
        exported[0]["guidance"]["instructions"],
        "# 순서\n\n    cargo test\n"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn 보드_안내만_바꾸어도_기존_행과_내보내기에_반영된다() {
    let root = std::env::temp_dir().join(format!("upeg-guidance-edit-{}", std::process::id()));
    let path = state_path_from_root(&root);
    let _ = std::fs::remove_dir_all(&root);
    let visibility = BoardVisibility::global_only();
    let state = default_state();
    save_state_to_path_in(&path, &state, &visibility).expect("기본 상태 저장");
    set_board_guidance_to_path_in(
        &path,
        "personal",
        BoardGuidance {
            description: "일상 작업".into(),
            instructions: "# 매일\n\n    정리\n".into(),
        },
        &visibility,
    )
    .expect("안내만 저장");

    let reloaded = load_state_from_path_in(&path, &visibility).expect("재로드");
    let imported =
        export::boards_from_json(&export::boards_to_json(&reloaded.boards).expect("내보내기"))
            .expect("가져오기");
    let guidance = &imported
        .iter()
        .find(|board| board.key == "personal")
        .expect("개인 보드")
        .guidance;
    assert_eq!(guidance.description, "일상 작업");
    assert_eq!(guidance.instructions, "# 매일\n\n    정리\n");
    assert_eq!(reloaded.layouts, state.layouts);
    set_board_guidance_to_path_in(&path, "personal", BoardGuidance::default(), &visibility)
        .expect("안내 제거");
    let cleared = load_state_from_path_in(&path, &visibility).expect("안내 제거 후 재로드");
    assert_eq!(
        board_guidance_in(&cleared, "personal"),
        Some(&BoardGuidance::default())
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn 보드_안내_수정은_지정한_개인_보드에만_적용된다() {
    let mut state = default_state();
    let layouts = state.layouts.clone();
    set_board_guidance_in(
        &mut state,
        "personal",
        BoardGuidance {
            description: "개인 작업".into(),
            instructions: "# 시작\n\n    확인\n".into(),
        },
        &BoardVisibility::global_only(),
    )
    .expect("안내 변경");

    let guidance = board_guidance_in(&state, "personal").expect("개인 보드");
    assert_eq!(guidance.description, "개인 작업");
    assert_eq!(guidance.instructions, "# 시작\n\n    확인\n");
    assert_eq!(
        board_guidance_in(&state, "dev"),
        Some(&BoardGuidance::default())
    );
    assert_eq!(state.layouts, layouts);
}

#[test]
fn 없는_보드의_안내_수정은_상태를_바꾸지_않는다() {
    let mut state = default_state();
    let before = state.clone();
    let error = set_board_guidance_in(
        &mut state,
        "missing",
        BoardGuidance::default(),
        &BoardVisibility::global_only(),
    )
    .expect_err("알 수 없는 보드");

    assert!(matches!(error, BoardGuidanceEditError::UnknownBoard { board } if board == "missing"));
    assert_eq!(state, before);
}

#[test]
fn 안내_수정은_아직_등록되지_않은_핀과_프리셋을_보존한다() {
    let root = std::env::temp_dir().join(format!("upeg-guidance-deferred-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let path = state_path_from_root(&root);
    let visibility = BoardVisibility::global_only();
    let mut store = crate::store::Store::open_at(&path).expect("스토어");
    let mut state = default_state();
    state.layouts.insert(
        "personal".into(),
        vec![
            Placement::new("not_loaded.echo", 0, 0).with_args_preset(Some(
                upeg_core::ArgsPreset::parse(r#"{"message":"유지"}"#).expect("프리셋"),
            )),
        ],
    );
    store
        .save_state(&state, &visibility)
        .expect("지연 도구 핀 저장");

    set_board_guidance_to_path_in(
        &path,
        "personal",
        BoardGuidance {
            description: "새 설명".into(),
            instructions: "새 지침".into(),
        },
        &visibility,
    )
    .expect("안내 저장");

    let reloaded = store.load_state(&visibility).expect("원본 재로드");
    assert_eq!(reloaded.layouts, state.layouts);
    assert_eq!(
        board_guidance_in(&reloaded, "personal")
            .expect("안내")
            .description,
        "새 설명"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn 첫_안내_저장은_모든_기본_보드와_핀을_유지한다() {
    let root =
        std::env::temp_dir().join(format!("upeg-guidance-first-edit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let path = state_path_from_root(&root);
    let visibility = BoardVisibility::global_only();
    let expected = default_state();

    set_board_guidance_to_path_in(
        &path,
        "personal",
        BoardGuidance {
            description: "첫 설명".into(),
            instructions: String::new(),
        },
        &visibility,
    )
    .expect("첫 안내 저장");

    let state = crate::store::Store::open_at(&path)
        .expect("스토어")
        .load_state(&visibility)
        .expect("저장 상태");
    assert_eq!(state.boards.len(), expected.boards.len());
    assert_eq!(state.layouts, expected.layouts);
    assert_eq!(
        board_guidance_in(&state, "personal")
            .expect("안내")
            .description,
        "첫 설명"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn 안내_저장은_다른_연결이_바꾼_최신_배치에_영향을_주지_않는다() {
    let root =
        std::env::temp_dir().join(format!("upeg-guidance-other-writer-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let path = state_path_from_root(&root);
    let visibility = BoardVisibility::global_only();
    let mut editor = crate::store::Store::open_at(&path).expect("안내 편집 연결");
    editor
        .save_state(&default_state(), &visibility)
        .expect("기본 저장");
    let _previous = editor.load_state(&visibility).expect("기존 화면 상태");
    let mut other = crate::store::Store::open_at(&path).expect("다른 연결");
    let changed = Placement::new("not_loaded.other_writer", 5, 7);
    other
        .upsert_placement("personal", &changed)
        .expect("다른 연결의 변경");

    assert!(
        editor
            .update_board_guidance(
                "personal",
                &BoardGuidance {
                    description: "새 안내".into(),
                    instructions: String::new(),
                },
                &visibility
            )
            .expect("안내만 수정")
    );

    let state = other.load_state(&visibility).expect("최신 상태");
    assert!(state.layouts["personal"].contains(&changed));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn 없는_보드의_안내를_저장해도_저장소_리비전은_바뀌지_않는다() {
    let root = std::env::temp_dir().join(format!(
        "upeg-guidance-missing-write-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let path = state_path_from_root(&root);
    let store = crate::store::Store::open_at(&path).expect("스토어");
    let before = store.change_rev().expect("기존 리비전");

    let error = set_board_guidance_to_path_in(
        &path,
        "missing",
        BoardGuidance::default(),
        &BoardVisibility::global_only(),
    )
    .expect_err("없는 보드");

    assert!(matches!(error, BoardGuidanceEditError::UnknownBoard { .. }));
    assert_eq!(store.change_rev().expect("변경 후 리비전"), before);
    let _ = std::fs::remove_dir_all(root);
}
