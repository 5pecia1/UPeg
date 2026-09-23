//! External process readiness and setup guidance copy.

use phf::{Map, phf_map};

pub(super) static EN: Map<&'static str, &'static str> = phf_map! {
    "readiness.missing_executable.label" => "command not installed",
    "readiness.missing_executable.hint" => "Install the required command, then check again.",
    "readiness.missing_working_directory.label" => "working directory unavailable",
    "readiness.missing_working_directory.hint" => "Fix the configured working directory, then check again.",
    "readiness.unchecked_credential_path.label" => "command path checked at run time",
    "readiness.unchecked_credential_path.hint" => "A credential supplies PATH, so availability is checked when the tool runs.",
    "readiness.checking" => "checking requirements…",
    "readiness.ready_on_platform" => "ready on {platform}",
    "readiness.setup_required" => "setup required",
    "readiness.setup_title" => "Set up {tool}",
    "readiness.recheck" => "Check again",
    "readiness.copy_command" => "copy command",
    "readiness.open_guide" => "open installation guide",
    "readiness.guide_unavailable" => "could not open the installation guide",
    "readiness.run_time_check" => "check at run time",
    "readiness.host_unavailable" => "could not check the paired host",
    "readiness.host_malformed" => "the paired host returned malformed readiness data",
    "readiness.host_unpaired" => "pair a host in Settings to check this command",
    "readiness.remote_catalog_limit" => "Remote host tools are available for readiness checks here; adding them to this browser's board is not yet supported.",
};

pub(super) static KO: Map<&'static str, &'static str> = phf_map! {
    "readiness.missing_executable.label" => "명령이 설치되지 않음",
    "readiness.missing_executable.hint" => "필요한 명령을 설치한 뒤 다시 확인하세요.",
    "readiness.missing_working_directory.label" => "작업 디렉터리를 사용할 수 없음",
    "readiness.missing_working_directory.hint" => "설정된 작업 디렉터리를 수정한 뒤 다시 확인하세요.",
    "readiness.unchecked_credential_path.label" => "실행 시 명령 경로 확인",
    "readiness.unchecked_credential_path.hint" => "credential이 PATH를 제공하므로 도구 실행 시 가용성을 확인합니다.",
    "readiness.checking" => "요구 사항 확인 중…",
    "readiness.ready_on_platform" => "{platform}에서 준비됨",
    "readiness.setup_required" => "설정 필요",
    "readiness.setup_title" => "{tool} 설정",
    "readiness.recheck" => "다시 확인",
    "readiness.copy_command" => "명령 복사",
    "readiness.open_guide" => "설치 안내 열기",
    "readiness.guide_unavailable" => "설치 안내를 열 수 없습니다",
    "readiness.run_time_check" => "실행 시 확인",
    "readiness.host_unavailable" => "연결된 호스트를 확인할 수 없습니다",
    "readiness.host_malformed" => "연결된 호스트의 준비 상태 응답 형식이 올바르지 않습니다",
    "readiness.host_unpaired" => "이 명령을 확인하려면 설정에서 호스트를 연결하세요",
    "readiness.remote_catalog_limit" => "원격 호스트 도구는 여기서 준비 상태를 확인할 수 있습니다. 이 브라우저의 보드에 추가하는 기능은 아직 지원하지 않습니다.",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readiness_copy_has_the_same_keys_in_both_languages() {
        assert_eq!(EN.len(), KO.len());
        for key in EN.keys() {
            assert!(KO.contains_key(key), "{key}");
        }
    }
}
