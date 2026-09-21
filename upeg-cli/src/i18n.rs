//! TUI translation catalog and locale-aware lookup wrappers.
//!
//! TUI-local string catalog, paired with the Flutter desktop catalog in
//! `flutter_app/lib/src/i18n/` through the shared mechanism in
//! `upeg_core::i18n`. Catalog content stays per-surface so a desktop-only
//! string never leaks into the TUI and vice versa.
//!
//! TUI render functions receive `Locale` explicitly (no global signal).
//! This matches the rest of the TUI's pure-state posture — every
//! rendering decision flows from `State`.

use phf::{Map, phf_map};
use upeg_core::i18n as core;
use upeg_core::prefs::Locale;

/// Look up a translation for an explicitly-provided locale. Wraps
/// [`upeg_core::i18n::t`] with the TUI [`catalog`].
pub fn t(locale: Locale, key: &str) -> &'static str {
    core::t(locale, key, catalog)
}

/// Look up a translation and substitute `{name}` placeholders.
pub fn t_args(locale: Locale, key: &str, args: &[(&str, &str)]) -> String {
    core::t_args(locale, key, args, catalog)
}

/// Catalog adapter — defers to the locale-specific [`phf::Map`].
pub fn catalog(locale: Locale, key: &str) -> Option<&'static str> {
    match locale {
        Locale::En => EN.get(key).copied(),
        Locale::Ko => KO.get(key).copied(),
    }
}

// ─── Catalog tables ────────────────────────────────────────────

static EN: Map<&'static str, &'static str> = phf_map! {
    // Header bar (view.rs `render_header`).
    "tui.header.title"              => " upeg TUI ",
    "tui.header.frame_title"        => " Universal Pegboard ",
    "tui.header.hint.navigate"      => "←↓↑→/hjkl navigate  ",
    // Enter/F1/Space always run/confirm; inspecting a tool's manifest
    // moved to its own key so the two intents stay distinguishable.
    "tui.header.hint.open"          => "o inspect  ",
    "tui.header.hint.run"           => "↵/Space/F1 run  ",
    "tui.header.hint.filters"       => "b/t/1-9 filters  / search  ",
    "tui.header.hint.quit"          => "q quit",

    // Filter bars.
    "tui.boards.title"              => " Boards ",
    "tui.tags.title"                => " Tags ",

    // Surface gating + run forms.
    "tui.detail.not_on_surface"     => "not on TUI",
    "tui.detail.body"               => "id            {id}\ntoolkit       {toolkit}\ntags          {tags}\npin   {pin}\ninvoker       {invoker}\nsurfaces      {surfaces}\nboards {boards}\n\n{description}",
    "tui.run.header"                => "Run: {tool_id}\n\n",

    // Header hint variant that includes the `s` settings entry.
    "tui.header.hint.settings"      => "s settings  ",
    // Board-management + per-pin hint chips. Modeless: always live, so
    // the single hint bar advertises them next to navigation. Each chip
    // pairs the key with the verb it performs.
    "tui.header.edit.hint.new"      => "n new  ",
    "tui.header.edit.hint.rename"   => "R rename  ",
    "tui.header.edit.hint.delete"   => "D delete  ",
    "tui.header.edit.hint.add"      => "a add  ",
    "tui.header.edit.hint.pin"      => "p pin  ",
    "tui.header.edit.hint.color"    => "c color  ",
    "tui.header.edit.hint.move"     => "m move  [ ] swap  ",

    // Phase 6 — BoardEditor + ConfirmDeleteBoard + ToolPicker.
    "tui.board.editor.title"           => " Edit board · ↵/F1 commit · Ctrl+U clear · esc cancel ",
    "tui.board.editor.add_prompt"      => "New board title: ",
    "tui.board.editor.rename_prompt"   => "Rename {title} → ",
    "tui.board.editor.empty_hint"      => "(type a title and press ↵)",
    "tui.board.delete.confirm.title"   => " Delete board ",
    "tui.board.delete.confirm.prompt"  => "Delete board '{title}'? Layout drops with it.",
    "tui.board.delete.confirm.options" => "[y/F1] delete  ·  [n] keep",
    "tui.quit.confirm.title"           => " Quit upeg ",
    "tui.quit.confirm.prompt"          => "Quit upeg? Unsaved in-flight input is lost.",
    "tui.quit.confirm.options"         => "[y/F1] quit  ·  [n] stay",
    // Chain approval barrier (upeg_runtime::tool_approval_policy).
    "tui.approval.confirm.title"       => " Approval required ",
    "tui.approval.confirm.prompt"      => "This chain has a step that requires approval. Run '{tool_id}'?",
    "tui.approval.confirm.options"     => "[↵/y/F1] approve  ·  [esc/n] cancel",
    "tui.approval.denied_for_surface"  => "this chain only honors approvals from {surfaces}; the TUI cannot approve it. Run it from one of those surfaces.",
    "tui.toolpicker.title"             => " Add tool · ↵/F1 pin/unpin · Ctrl+U clear · esc back ",
    "tui.toolsearch.title"             => " Search tools · ↵/F1 open · Ctrl+U clear · esc back ",
    "tui.toolpicker.query"             => "Filter: ",
    "tui.toolpicker.empty"             => "(no matches)",
    "tui.toolpicker.pinned_marker"     => "★ ",
    "tui.pin_color.editor.title"       => " Pin color · ↵/F1 apply · r reset · Ctrl+U clear · esc/q cancel ",
    "tui.pin_color.editor.tool"        => "Pin: ",
    "tui.pin_color.editor.current"     => "Color: ",
    "tui.pin_color.editor.palette"     => "Palette:",
    "tui.pin_color.editor.hint"        => "1-9 palette · type #RRGGBB · Backspace edit",

    // Settings View (Stage E).
    "tui.settings.title"            => " Settings · ←/→ change · esc back ",
    "tui.settings.field.locale"     => "Language",
    "tui.settings.field.theme"      => "Theme",
    "tui.settings.field.accent"     => "Accent",
    "tui.settings.locale.en"        => "English",
    "tui.settings.locale.ko"        => "한국어",
    "tui.settings.theme.dark"       => "Dark",
    "tui.settings.theme.light"      => "Light",
    "tui.settings.accent.green"     => "Green",
    "tui.settings.accent.amber"     => "Amber",
    "tui.settings.accent.cyan"      => "Cyan",
    "tui.settings.accent.pink"      => "Pink",
    "tui.settings.hint"             => "↑↓ field · ←→ value · esc back",

    // Pegboard grid + right-pane titles and empty-state placeholders.
    "tui.grid.title"                => " Pegboard grid · {count} ",
    "tui.grid.moving_title"         => " Pegboard grid · moving {tool} · ↵ commit · esc cancel · {count} ",
    "tui.grid.resize_title"         => " Pegboard grid · resizing {tool} {cols}x{rows} · 0 reset · ↵ commit · esc cancel · {count} ",
    "tui.grid.empty"                => "(toolbox empty)",
    "tui.right_pane.list"           => " Tool · ↵ run · o inspect · Space run · / search ",
    "tui.right_pane.detail"         => " Manifest · F1/↵ run · F2 copy id · q back ",
    "tui.right_pane.form"           => " Form · ↵ run · esc back ",
    "tui.right_pane.result"         => " Result · ↵/F1 rerun · F2 copy · esc/q close ",
    "tui.right_pane.running"        => " Running · esc cancel · esc esc quit ",
    "tui.running.status"            => "running…",
    "tui.running.cancelling"        => "cancelling…",
    "tui.running.empty"             => "(no output yet)",
    "tui.empty.list"                => "(toolbox empty — link upeg-tools or load TOML)",
    "tui.empty.detail"              => "(no tool selected)",

    // Manifest body fragments (technical field labels like `id`,
    // `toolkit` stay English on purpose — they're the same identifiers
    // CLI/MCP/JSON surfaces ship, and a Korean rename would break
    // cross-surface docs). Only the user-prose pieces translate.
    "tui.manifest.no_boards"        => "(none)",
    "tui.manifest.no_description"   => "(no description)",
    "tui.manifest.inputs_none"      => "\n\ninputs        (none)",
    "tui.manifest.inputs_header"    => "\n\ninputs        {count} field(s)",
    "tui.manifest.embed_url"        => "\n\nembed_url     {url}",
    "tui.manifest.bindings_header"  => "\n\ncontrolled_embed.bindings ({count})",
    "tui.manifest.run_hint"         => "[↵/F1] run",

    // Errors surfaced into the Result view.
    "tui.error.tool_not_found"      => "tool not found",
    "tui.error.not_on_surface"      => "tool is pinned in the shared pegboard, but is not available on TUI (surfaces: {surfaces})",

    // A second Run while one is still in flight. The TUI dispatches one
    // tool at a time, so the honest answer is to name what is running
    // rather than open a pane that would fill with the other run's
    // output.
    "tui.run.already_running"       => "already running {tool_id} — Esc cancels it",

    // F2 clipboard copy feedback (Result/Detail). Best-effort — no
    // clipboard crate exists in the workspace, so failure just means
    // no supported platform paste/copy program was found.
    "tui.clipboard.copied"          => "copied to clipboard",
    "tui.clipboard.failed"          => "clipboard unavailable on this host",

    // Footer suffixes appended to the runtime NetworkStatus label.
    "tui.footer.attached"           => " · attached: {endpoint}",
    "tui.footer.standalone"         => " · standalone",
};

static KO: Map<&'static str, &'static str> = phf_map! {
    "tui.header.title"              => " upeg TUI ",
    "tui.header.frame_title"        => " Universal Pegboard ",
    "tui.header.hint.navigate"      => "←↓↑→/hjkl 이동  ",
    "tui.header.hint.open"          => "o 상세  ",
    "tui.header.hint.run"           => "↵/Space/F1 실행  ",
    "tui.header.hint.filters"       => "b/t/1-9 필터  / 검색  ",
    "tui.header.hint.quit"          => "q 종료",

    "tui.boards.title"              => " 보드 ",
    "tui.tags.title"                => " 태그 ",

    "tui.detail.not_on_surface"     => "TUI에서 지원하지 않음",
    "tui.detail.body"               => "id            {id}\ntoolkit       {toolkit}\ntags          {tags}\npin   {pin}\ninvoker       {invoker}\nsurfaces      {surfaces}\nboards {boards}\n\n{description}",
    "tui.run.header"                => "실행: {tool_id}\n\n",

    "tui.header.hint.settings"      => "s 설정  ",
    "tui.header.edit.hint.new"      => "n 새 보드  ",
    "tui.header.edit.hint.rename"   => "R 이름변경  ",
    "tui.header.edit.hint.delete"   => "D 삭제  ",
    "tui.header.edit.hint.add"      => "a 추가  ",
    "tui.header.edit.hint.pin"      => "p 핀  ",
    "tui.header.edit.hint.color"    => "c 색상  ",
    "tui.header.edit.hint.move"     => "m 이동  [ ] 교환  ",

    "tui.board.editor.title"           => " 보드 편집 · ↵/F1 저장 · Ctrl+U 지우기 · esc 취소 ",
    "tui.board.editor.add_prompt"      => "새 보드 이름: ",
    "tui.board.editor.rename_prompt"   => "{title} → ",
    "tui.board.editor.empty_hint"      => "(이름을 입력하고 ↵)",
    "tui.board.delete.confirm.title"   => " 보드 삭제 ",
    "tui.board.delete.confirm.prompt"  => "보드 '{title}'을 삭제할까요? 레이아웃도 함께 사라집니다.",
    "tui.board.delete.confirm.options" => "[y/F1] 삭제  ·  [n] 유지",
    "tui.quit.confirm.title"           => " upeg 종료 ",
    "tui.quit.confirm.prompt"          => "upeg를 종료할까요? 입력 중이던 내용은 사라집니다.",
    "tui.quit.confirm.options"         => "[y/F1] 종료  ·  [n] 유지",
    // Chain approval barrier (upeg_runtime::tool_approval_policy).
    "tui.approval.confirm.title"       => " 승인 필요 ",
    "tui.approval.confirm.prompt"      => "이 chain에는 승인이 필요한 step이 있습니다. '{tool_id}'을 실행할까요?",
    "tui.approval.confirm.options"     => "[↵/y/F1] 승인  ·  [esc/n] 취소",
    "tui.approval.denied_for_surface"  => "이 chain은 {surfaces} 표면의 승인만 인정합니다. TUI에서는 승인할 수 없으니 해당 표면에서 실행하세요.",
    "tui.toolpicker.title"             => " 도구 추가 · ↵/F1 핀/해제 · Ctrl+U 지우기 · esc 뒤로 ",
    "tui.toolsearch.title"             => " 도구 검색 · ↵/F1 열기 · Ctrl+U 지우기 · esc 뒤로 ",
    "tui.toolpicker.query"             => "필터: ",
    "tui.toolpicker.empty"             => "(일치 없음)",
    "tui.toolpicker.pinned_marker"     => "★ ",
    "tui.pin_color.editor.title"       => " 핀 색상 · ↵/F1 적용 · r 초기화 · Ctrl+U 지우기 · esc/q 취소 ",
    "tui.pin_color.editor.tool"        => "핀: ",
    "tui.pin_color.editor.current"     => "색상: ",
    "tui.pin_color.editor.palette"     => "팔레트:",
    "tui.pin_color.editor.hint"        => "1-9 팔레트 · #RRGGBB 입력 · Backspace 수정",

    "tui.settings.title"            => " 설정 · ←/→ 변경 · esc 뒤로 ",
    "tui.settings.field.locale"     => "언어",
    "tui.settings.field.theme"      => "테마",
    "tui.settings.field.accent"     => "강조색",
    "tui.settings.locale.en"        => "English",
    "tui.settings.locale.ko"        => "한국어",
    "tui.settings.theme.dark"       => "다크",
    "tui.settings.theme.light"      => "라이트",
    "tui.settings.accent.green"     => "그린",
    "tui.settings.accent.amber"     => "앰버",
    "tui.settings.accent.cyan"      => "시안",
    "tui.settings.accent.pink"      => "핑크",
    "tui.settings.hint"             => "↑↓ 항목 · ←→ 값 · esc 뒤로",

    "tui.grid.title"                => " 페그보드 · {count} ",
    "tui.grid.moving_title"         => " 페그보드 · {tool} 이동 중 · ↵ 저장 · esc 취소 · {count} ",
    "tui.grid.resize_title"         => " 페그보드 · {tool} 크기 조절 {cols}x{rows} · 0 초기화 · ↵ 저장 · esc 취소 · {count} ",
    "tui.grid.empty"                => "(툴박스 비어 있음)",
    "tui.right_pane.list"           => " 도구 · ↵ 실행 · o 상세 · Space 실행 · / 검색 ",
    "tui.right_pane.detail"         => " 매니페스트 · F1/↵ 실행 · F2 id 복사 · q 뒤로 ",
    "tui.right_pane.form"           => " 입력 · ↵ 실행 · esc 뒤로 ",
    "tui.right_pane.result"         => " 결과 · ↵/F1 재실행 · F2 복사 · esc/q 닫기 ",
    "tui.right_pane.running"        => " 실행 중 · esc 취소 · esc esc 종료 ",
    "tui.running.status"            => "실행 중…",
    "tui.running.cancelling"        => "취소 중…",
    "tui.running.empty"             => "(아직 출력 없음)",
    "tui.empty.list"                => "(툴박스 비어 있음 — upeg-tools 연결 또는 TOML 로드)",
    "tui.empty.detail"              => "(선택된 도구 없음)",

    "tui.manifest.no_boards"        => "(없음)",
    "tui.manifest.no_description"   => "(설명 없음)",
    "tui.manifest.inputs_none"      => "\n\ninputs        (없음)",
    "tui.manifest.inputs_header"    => "\n\ninputs        {count}개 필드",
    "tui.manifest.embed_url"        => "\n\nembed_url     {url}",
    "tui.manifest.bindings_header"  => "\n\ncontrolled_embed.bindings ({count})",
    "tui.manifest.run_hint"         => "[↵/F1] 실행",

    "tui.error.tool_not_found"      => "도구를 찾을 수 없음",
    "tui.error.not_on_surface"      => "도구가 공유 페그보드에 핀되어 있지만 TUI에서는 사용 불가 (지원 표면: {surfaces})",

    "tui.run.already_running"       => "{tool_id} 실행 중 — Esc로 취소할 수 있습니다",

    "tui.clipboard.copied"          => "클립보드에 복사됨",
    "tui.clipboard.failed"          => "이 호스트에서는 클립보드를 사용할 수 없음",

    "tui.footer.attached"           => " · 연결됨: {endpoint}",
    "tui.footer.standalone"         => " · 독립 실행",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_english_key_has_a_korean_translation() {
        let missing: Vec<&str> = EN
            .keys()
            .copied()
            .filter(|k| KO.get(*k).is_none())
            .collect();
        assert!(
            missing.is_empty(),
            "TUI Ko catalog is missing translations for: {missing:?}",
        );
    }

    #[test]
    fn no_orphan_korean_keys() {
        let orphans: Vec<&str> = KO
            .keys()
            .copied()
            .filter(|k| EN.get(*k).is_none())
            .collect();
        assert!(
            orphans.is_empty(),
            "TUI Ko catalog has orphan keys (no matching En entry): {orphans:?}",
        );
    }

    #[test]
    fn korean_locale_returns_korean_catalog() {
        assert_eq!(catalog(Locale::Ko, "tui.boards.title"), Some(" 보드 "));
    }

    #[test]
    fn english_locale_returns_english_catalog() {
        assert_eq!(catalog(Locale::En, "tui.boards.title"), Some(" Boards "));
    }
}
