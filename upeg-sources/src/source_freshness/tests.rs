use super::*;

fn fixture(label: &str) -> (PathBuf, RuntimeSourceConfig) {
    let root = std::env::temp_dir().join(format!(
        "upeg-source-snapshot-{label}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let config = RuntimeSourceConfig {
        toolkits_dir: Some(root.join("toolkits")),
        project_manifest: None,
        wasm_dir: None,
        mcp_import_dir: Some(root.join("imports")),
    };
    (root, config)
}

#[test]
fn 소스_스냅샷은_파일_변경과_추가와_삭제를_감지한다() {
    let (root, config) = fixture("mutations");
    let directory = config.toolkits_dir.as_ref().expect("도구킷 경로");
    std::fs::create_dir_all(directory).expect("디렉터리");
    let path = directory.join("tool.toml");
    std::fs::write(&path, "id = 'initial'\n").expect("선언");
    record_sources_for_root(&root, &config);
    assert!(validate_sources_for_root(&root, &config).is_ok());
    std::fs::write(directory.join("notes.md"), "안내만 변경").expect("관련 없는 파일");
    assert!(validate_sources_for_root(&root, &config).is_ok());

    for content in [Some("id = 'changed'\n"), None, Some("id = 'added'\n")] {
        match content {
            Some(content) => std::fs::write(&path, content).expect("선언 변경"),
            None => std::fs::remove_file(&path).expect("선언 삭제"),
        }
        assert!(matches!(
            validate_sources_for_root(&root, &config),
            Err(LoadedSourcesError::Changed { .. })
        ));
        record_sources_for_root(&root, &config);
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn 소스_스냅샷은_설정_루트끼리_간섭하지_않는다() {
    let (first_root, first) = fixture("first-root");
    let (second_root, second) = fixture("second-root");
    record_sources_for_root(&first_root, &first);
    assert!(validate_sources_for_root(&second_root, &second).is_ok());
    record_sources_for_root(&second_root, &second);
    let directory = first.toolkits_dir.as_ref().expect("도구킷 경로");
    std::fs::create_dir_all(directory).expect("디렉터리");
    std::fs::write(directory.join("new.toml"), "id = 'new'\n").expect("새 선언");

    assert!(validate_sources_for_root(&first_root, &first).is_err());
    assert!(validate_sources_for_root(&second_root, &second).is_ok());
    let _ = std::fs::remove_dir_all(first_root);
}

#[test]
fn 지연_임포트_로드는_시작_시점_스냅샷을_덮어쓰지_않는다() {
    let (root, config) = fixture("deferred-import");
    record_sources_for_root(&root, &config);
    let directory = config.mcp_import_dir.as_ref().expect("임포트 경로");
    std::fs::create_dir_all(directory).expect("디렉터리");
    std::fs::write(directory.join("invalid.toml"), "invalid = [").expect("유효하지 않은 선언");
    crate::load_mcp_imports_for_host(&config);

    assert!(matches!(
        validate_sources_for_root(&root, &config),
        Err(LoadedSourcesError::Changed { .. })
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn 소스_디렉터리_설정이_바뀌면_재시작해야_한다() {
    let (root, config) = fixture("changed-path");
    record_sources_for_root(&root, &config);
    let mut changed = config;
    changed.toolkits_dir = Some(root.join("other-toolkits"));

    assert!(matches!(
        validate_sources_for_root(&root, &changed),
        Err(LoadedSourcesError::Changed { .. })
    ));
}

#[test]
fn 읽을_수_없는_소스는_준비된_설정으로_취급하지_않는다() {
    let (root, config) = fixture("unreadable-source");
    let path = config
        .toolkits_dir
        .as_ref()
        .expect("도구킷 경로")
        .join("directory.toml");
    std::fs::create_dir_all(&path).expect("선언 대신 디렉터리");
    record_sources_for_root(&root, &config);
    assert!(matches!(
        validate_sources_for_root(&root, &config),
        Err(LoadedSourcesError::Unavailable { .. })
    ));
    std::fs::remove_dir_all(&path).expect("읽기 오류 제거");
    assert!(matches!(
        validate_sources_for_root(&root, &config),
        Err(LoadedSourcesError::Unavailable { .. })
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(feature = "wasm-plugin")]
#[test]
fn 와즘_플러그인_변경도_소스_변경으로_감지한다() {
    let (root, mut config) = fixture("wasm-source");
    let directory = root.join("wasm");
    std::fs::create_dir_all(&directory).expect("와즘 디렉터리");
    let path = directory.join("tool.wasm");
    std::fs::write(&path, b"before").expect("기존 바이너리");
    config.wasm_dir = Some(directory);
    record_sources_for_root(&root, &config);
    std::fs::write(&path, b"after").expect("바이너리 변경");

    assert!(matches!(
        validate_sources_for_root(&root, &config),
        Err(LoadedSourcesError::Changed { .. })
    ));
    let _ = std::fs::remove_dir_all(root);
}
