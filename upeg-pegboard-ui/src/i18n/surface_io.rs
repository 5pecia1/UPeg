//! Board status, file selection, host-attach, and backup copy.

use phf::{Map, phf_map};

pub(super) static EN: Map<&'static str, &'static str> = phf_map! {
    "empty.pegboard.no_pins" => "no pins on this board yet",
    "a11y.inline.running" => "Running tool",
    "inline.dispatch_failed" => "Unable to run this tool.",
    "inline.run" => "Run",

    // File input field (file_input_field.dart) + typed selection
    // failures (file_selection_assembler.dart — `FileSelectionFailure`
    // carries the code, `messageKey` maps it to one of these).
    "modal.generic.file_path_suffix" => "{label} (file path)",
    "modal.generic.file_pick"       => "Choose file",
    "modal.generic.file_clear"      => "clear selection",
    "modal.file.empty_prompt"              => "Select files or\ndrag them here.",
    "modal.file.selected_count"            => "{count} file(s) selected",
    "modal.file.pick_failed"               => "Could not select files.",
    "modal.file.error.invalid_policy"      => "The file selection policy is invalid.",
    "modal.file.error.empty_selection"     => "No files were selected.",
    "modal.file.error.directory_selected"  => "Folders cannot be selected: {file}",
    "modal.file.error.extension_not_allowed" => "File type is not allowed: {file}",
    "modal.file.error.too_many_files"      => "You can select at most {max} files.",
    "modal.file.error.too_many_nodes"      => "The file structure exceeds the {max}-node limit.",
    "modal.file.error.metadata_too_large"  => "File names and MIME info are too large.",
    "modal.file.error.file_too_large"      => "File exceeds the size limit: {file}",
    "modal.file.error.total_too_large"     => "The total file size exceeds the limit.",
    "modal.file.error.read_failed"         => "Could not read the file: {file}",

    // Host-attach section (host_attach_section.dart).
    "settings.section.host_attach"  => "host attach",
    "host_attach.base_url_label"    => "Host URL",
    "host_attach.token_label"       => "Host token",
    "host_attach.check_button"      => "Check connection",
    "host_attach.checking"          => "checking…",
    "host_attach.connected"          => "connected",
    "host_attach.unreachable"       => "connection failed · check that the daemon is running",

    // Host-attach failure notice on pins (host_attach_notice_body.dart).
    "host_attach.notice.unauthorized_label" => "authentication failed",
    "host_attach.notice.unauthorized_hint"  => "check the host token in Settings",
    "host_attach.notice.unreachable_label"  => "host connection failed",
    "host_attach.notice.unreachable_hint"   => "check that the daemon is running",
    "host_attach.notice.tool_error_label"   => "run error",
    // Client-side tool-error codes (attach_client.dart) map to these
    // localized hints; daemon-authored errors pass their message through.
    "host_attach.notice.response_too_large_hint" => "the host's answer was too large to read",
    "host_attach.notice.malformed_response_hint" => "the host's answer was malformed",
    "host_attach.notice.invalid_file_hint"       => "the host returned a file this app could not read",

    // Backup export / import section (backup_section.dart).
    "settings.section.backup"       => "backup",
    "settings.backup.export"        => "export",
    "settings.backup.import"        => "import",
    "settings.backup.export_failed" => "export failed: {msg}",
    "settings.backup.import_failed" => "import failed: {msg}",
    "settings.backup.export_dialog_title" => "Save backup",
    "settings.backup.import_dialog_title" => "Open backup",
    "settings.backup.exported"            => "Saved backup to {path}",
    "settings.backup.imported"            => "Restored {boards} boards, {layouts} layouts, {memos} memos",

    // Backup pipeline error messages (backup.rs). Internal technical
    // strings ("Blob creation failed", "FileReader unavailable…") stay
    // English on purpose — they're diagnostic context the user only
    // ever sees behind the localised wrapper. The native-stub strings
    // and the import-wrapper here are the user-facing top lines.
    "backup.unavailable.export"     => "Backup export is not available on desktop yet",
    "backup.unavailable.import"     => "Backup import is not available on desktop yet",
};

pub(super) static KO: Map<&'static str, &'static str> = phf_map! {
    "empty.pegboard.no_pins" => "이 보드에는 아직 핀이 없습니다",
    "a11y.inline.running" => "도구 실행 중",
    "inline.dispatch_failed" => "이 도구를 실행할 수 없습니다.",
    "inline.run" => "실행",

    // File input field + typed selection failures — see the EN block comment.
    "modal.generic.file_path_suffix" => "{label} (파일 경로)",
    "modal.generic.file_pick"       => "파일 선택",
    "modal.generic.file_clear"      => "선택 지우기",
    "modal.file.empty_prompt"              => "파일을 선택하거나\n여기로 끌어 놓으세요.",
    "modal.file.selected_count"            => "{count}개 파일 선택됨",
    "modal.file.pick_failed"               => "파일을 선택하지 못했습니다.",
    "modal.file.error.invalid_policy"      => "파일 선택 정책이 올바르지 않습니다.",
    "modal.file.error.empty_selection"     => "선택된 파일이 없습니다.",
    "modal.file.error.directory_selected"  => "폴더는 선택할 수 없습니다: {file}",
    "modal.file.error.extension_not_allowed" => "허용되지 않는 파일 형식입니다: {file}",
    "modal.file.error.too_many_files"      => "파일은 최대 {max}개까지 선택할 수 있습니다.",
    "modal.file.error.too_many_nodes"      => "파일 구조가 최대 {max}개 노드를 초과했습니다.",
    "modal.file.error.metadata_too_large"  => "파일 이름과 MIME 정보가 너무 큽니다.",
    "modal.file.error.file_too_large"      => "파일 크기 제한을 초과했습니다: {file}",
    "modal.file.error.total_too_large"     => "전체 파일 크기 제한을 초과했습니다.",
    "modal.file.error.read_failed"         => "파일을 읽지 못했습니다: {file}",

    "settings.section.host_attach"  => "호스트 연결",
    "host_attach.base_url_label"    => "호스트 주소",
    "host_attach.token_label"       => "호스트 토큰",
    "host_attach.check_button"      => "연결 확인",
    "host_attach.checking"          => "확인 중…",
    "host_attach.connected"          => "연결됨",
    "host_attach.unreachable"       => "연결 실패 · 데몬이 실행 중인지 확인하세요",

    "host_attach.notice.unauthorized_label" => "인증 실패",
    "host_attach.notice.unauthorized_hint"  => "설정에서 host 토큰을 확인하세요",
    "host_attach.notice.unreachable_label"  => "호스트 연결 실패",
    "host_attach.notice.unreachable_hint"   => "데몬이 실행 중인지 확인하세요",
    "host_attach.notice.tool_error_label"   => "실행 오류",
    "host_attach.notice.response_too_large_hint" => "호스트 응답이 너무 커서 읽을 수 없습니다",
    "host_attach.notice.malformed_response_hint" => "호스트 응답 형식이 올바르지 않습니다",
    "host_attach.notice.invalid_file_hint"       => "호스트가 읽을 수 없는 파일을 반환했습니다",

    // Backup export / import section — see the EN block comment.
    "settings.section.backup"       => "백업",
    "settings.backup.export"        => "내보내기",
    "settings.backup.import"        => "가져오기",
    "settings.backup.export_failed" => "내보내기 실패: {msg}",
    "settings.backup.import_failed" => "가져오기 실패: {msg}",
    "settings.backup.export_dialog_title" => "백업 저장",
    "settings.backup.import_dialog_title" => "백업 열기",
    "settings.backup.exported"            => "백업을 {path}에 저장했습니다",
    "settings.backup.imported"            => "보드 {boards}개, 레이아웃 {layouts}개, 메모 {memos}개를 복원했습니다",

    "backup.unavailable.export"     => "데스크탑에서는 백업 내보내기를 아직 지원하지 않습니다",
    "backup.unavailable.import"     => "데스크탑에서는 백업 가져오기를 아직 지원하지 않습니다",
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::catalog;
    use std::collections::BTreeSet;
    use upeg_core::prefs::Locale;

    #[test]
    fn catalogs_have_exact_key_parity() {
        let en: BTreeSet<_> = EN.keys().collect();
        let ko: BTreeSet<_> = KO.keys().collect();
        assert_eq!(en, ko);
    }

    #[test]
    fn status_copy_resolves_in_both_locales() {
        for (key, english, korean) in [
            (
                "empty.pegboard.no_pins",
                "no pins on this board yet",
                "이 보드에는 아직 핀이 없습니다",
            ),
            ("a11y.inline.running", "Running tool", "도구 실행 중"),
            (
                "inline.dispatch_failed",
                "Unable to run this tool.",
                "이 도구를 실행할 수 없습니다.",
            ),
            ("inline.run", "Run", "실행"),
        ] {
            assert_eq!(catalog(Locale::En, key), Some(english), "{key}");
            assert_eq!(catalog(Locale::Ko, key), Some(korean), "{key}");
        }
    }
}
