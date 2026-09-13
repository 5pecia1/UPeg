use std::ffi::OsString;

use super::*;

struct SourceEnvironment(Vec<(&'static str, Option<OsString>)>);

impl SourceEnvironment {
    #[allow(
        unsafe_code,
        reason = "caller holds the pegboard home environment test lock"
    )]
    fn isolated() -> Self {
        let root = upeg_core::paths::config_root().expect("테스트 설정 루트");
        let variables = [
            ("UPEG_TOOLKITS_DIR", root.join("toolkits").into_os_string()),
            (
                "UPEG_MCP_IMPORTS_DIR",
                root.join("mcp-imports").into_os_string(),
            ),
            ("UPEG_WASM_DIR", root.join("wasm").into_os_string()),
            ("UPEG_PROJECT_MANIFEST_PATH", OsString::from("off")),
        ];
        Self(
            variables
                .into_iter()
                .map(|(key, value)| {
                    let previous = std::env::var_os(key);
                    unsafe {
                        std::env::set_var(key, value);
                    }
                    (key, previous)
                })
                .collect(),
        )
    }
}

impl Drop for SourceEnvironment {
    #[allow(
        unsafe_code,
        reason = "caller holds the pegboard home environment test lock"
    )]
    fn drop(&mut self) {
        for (key, previous) in &self.0 {
            match previous {
                Some(value) => unsafe {
                    std::env::set_var(key, value);
                },
                None => unsafe {
                    std::env::remove_var(key);
                },
            }
        }
    }
}

#[test]
fn 첫_미리보기_이전에_바뀐_도구킷은_재시작을_요청한다() {
    crate::test_support::with_seeded_pegboard_home(
        "board-source-before-preview",
        |_| {},
        || {
            let _environment = SourceEnvironment::isolated();
            let config = upeg_sources::RuntimeSourceConfig::from_env();
            let directory = config.toolkits_dir.as_ref().expect("도구킷 경로");
            std::fs::create_dir_all(directory).expect("도구킷 디렉터리");
            let path = directory.join("before-preview.toml");
            std::fs::write(&path, "id = 'before_preview'\n").expect("초기 선언");
            upeg_sources::load_local_runtime_sources(&config);
            std::fs::write(&path, "id = 'after_preview'\n").expect("선언 변경");

            let error = board_connection_preview(&BoardKey::parse("dev").expect("보드"))
                .expect_err("오래된 레지스트리로 연결 설정을 만들면 안 된다");
            assert!(error.to_string().contains("restart"), "{error}");
        },
    );
}

#[test]
fn 시작_이후_추가된_임포트_선언도_첫_미리보기에서_감지한다() {
    crate::test_support::with_seeded_pegboard_home(
        "board-source-added-import",
        |_| {},
        || {
            let _environment = SourceEnvironment::isolated();
            let config = upeg_sources::RuntimeSourceConfig::from_env();
            upeg_sources::load_local_runtime_sources(&config);
            let directory = config.mcp_import_dir.as_ref().expect("임포트 경로");
            std::fs::create_dir_all(directory).expect("임포트 디렉터리");
            std::fs::write(directory.join("added.toml"), "command = 'never-run'\n")
                .expect("임포트 추가");

            let error = board_connection_preview(&BoardKey::parse("dev").expect("보드"))
                .expect_err("추가된 선언도 재시작이 필요하다");
            assert!(error.to_string().contains("restart"), "{error}");
        },
    );
}

#[test]
#[allow(
    unsafe_code,
    reason = "caller holds the pegboard home environment test lock"
)]
fn 시작할_때_없던_프로젝트_매니페스트도_새로_생기면_감지한다() {
    crate::test_support::with_seeded_pegboard_home(
        "board-source-new-project",
        |_| {},
        || {
            let _environment = SourceEnvironment::isolated();
            let path = upeg_core::paths::config_root()
                .expect("설정 루트")
                .join("upeg.toml");
            unsafe {
                std::env::set_var("UPEG_PROJECT_MANIFEST_PATH", &path);
            }
            let config = upeg_sources::RuntimeSourceConfig::from_env();
            assert!(config.project_manifest.is_none());
            upeg_sources::load_local_runtime_sources(&config);
            std::fs::write(&path, "id = 'new_project'\n").expect("프로젝트 매니페스트 생성");

            let error = board_connection_preview(&BoardKey::parse("dev").expect("보드"))
                .expect_err("새 매니페스트를 반영하기 전에 재시작해야 한다");
            assert!(matches!(
                error,
                BoardAgentError::Sources(upeg_sources::LoadedSourcesError::Changed { .. })
            ));
        },
    );
}
