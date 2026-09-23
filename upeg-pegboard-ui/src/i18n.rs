//! Flutter / pegboard translation catalog.
//!
//! Catalog content lives here, not in `upeg-core` — the TUI has its own
//! catalog with its own keys, and we want desktop / popup / settings
//! strings to never leak into the TUI catalog.
//! The lookup mechanism + fallback chain is shared (see
//! [`upeg_core::i18n`]).
//!
//! Storage shape: a per-locale `phf::Map<&'static str, &'static str>`
//! for compile-time perfect-hash lookup. Adding a key means an entry in
//! both [`EN`] and [`KO`]; the test
//! `every_english_key_has_a_korean_translation` pins that contract.
//!
//! Active-locale resolution is the **caller's** concern — the surface
//! holding `Tweaks` (Riverpod on the Flutter side) passes the chosen
//! `Locale` into [`t`] / [`t_args`]. This crate is Signal-free on purpose.

mod board_guidance;
mod media;
mod readiness;
mod surface_io;

use phf::{Map, phf_map};
use upeg_core::ToolMeta;
use upeg_core::i18n as core;
use upeg_core::prefs::Locale;

/// Look up a translation for the explicit `locale`.
///
/// Wraps [`upeg_core::i18n::t`] with this surface's [`catalog`] adapter
/// so call sites just write `t("settings.theme", locale)`.
pub fn t(key: &str, locale: Locale) -> &'static str {
    core::t(locale, key, catalog)
}

/// Look up a translation and substitute `{name}` placeholders from
/// `args`.
pub fn t_args(key: &str, args: &[(&str, &str)], locale: Locale) -> String {
    core::t_args(locale, key, args, catalog)
}

/// Catalog lookup adapter — dispatches to the locale-specific
/// [`phf::Map`] and returns the matching translation, or `None` to fall
/// through to the `upeg_core::i18n::t` English / key-as-marker fallback
/// chain.
pub fn catalog(locale: Locale, key: &str) -> Option<&'static str> {
    match locale {
        Locale::En => EN
            .get(key)
            .or_else(|| surface_io::EN.get(key))
            .or_else(|| board_guidance::EN.get(key))
            .or_else(|| media::EN.get(key))
            .or_else(|| readiness::EN.get(key))
            .copied(),
        Locale::Ko => KO
            .get(key)
            .or_else(|| surface_io::KO.get(key))
            .or_else(|| board_guidance::KO.get(key))
            .or_else(|| media::KO.get(key))
            .or_else(|| readiness::KO.get(key))
            .copied(),
    }
}

/// Tool-metadata translation with static-default fallback.
///
/// Each `ToolMeta` ships English `display_label` / `description` as
/// part of its registration. The render layer asks this helper for the
/// active-locale text and falls through to the static metadata if no
/// translation exists. The key shape is `tool.<id>.label` /
/// `tool.<id>.description`.
pub fn tool_display_label(tool: &ToolMeta, locale: Locale) -> &'static str {
    lookup_tool_meta(tool.id, "label", tool.display_label, locale)
}

pub fn tool_description(tool: &ToolMeta, locale: Locale) -> &'static str {
    lookup_tool_meta(tool.id, "description", tool.description, locale)
}

fn lookup_tool_meta(
    tool_id: &str,
    field: &str,
    fallback: &'static str,
    locale: Locale,
) -> &'static str {
    // Build the catalog key in a stack-local String — the lookup hands
    // back the `&'static str` from the phf map, not a reference into
    // the temporary key.
    let key = format!("tool.{tool_id}.{field}");
    catalog(locale, &key)
        .or_else(|| catalog(Locale::En, &key))
        .unwrap_or(fallback)
}

// ─── Catalog tables ────────────────────────────────────────────
//
// Keep keys in dotted-namespace form: `<surface_or_screen>.<role>[.detail]`.
// Both maps must list the same keys; the parity tests pin that.

static EN: Map<&'static str, &'static str> = phf_map! {
    // Empty-board onboarding card (surfaces/desktop_empty.rs).
    "empty.board.title"            => "Board \"{board_title}\" is empty",
    "empty.tag.title"              => "No tools pinned to tag {tag}",
    "empty.add_hint.before"        => "Use the ",
    "empty.add_hint.between"       => " button or ",
    "empty.add_hint.after"         => " to pin a tool.",
    "empty.suggestion_header"      => "Suggested starters",
    "empty.find_tool_button"       => "pin a tool",

    // Expanded embed modal footnote (expanded_modal/embed.rs).
    "embed.note.daemon_only"       => "Only invoked once the Daemon WebView has explicitly started (PRD §5.4). ",
    "embed.note.selector_rebind"   => "Users re-bind selectors when the Embed mapping breaks (PRD §4.3).",

    // Desktop tab bar + edit-mode chrome (surfaces/desktop/mod.rs).
    "desktop.tab.rename_prompt"     => "Rename board",
    "desktop.tab.remove_confirm"    => "Remove board \"{title}\"?",
    "desktop.tab.rename_title"      => "rename this board",
    "desktop.tab.remove_title"      => "remove this board",
    "desktop.tab.new_placeholder"   => "Board name",
    "desktop.tab.add_confirm_title" => "add board",
    "desktop.tab.add_cancel_title"  => "cancel",
    "desktop.tab.rename_placeholder"   => "New board name",
    "desktop.tab.rename_confirm_title" => "rename",
    "desktop.tab.rename_cancel_title"  => "cancel rename",
    "desktop.tab.remove_confirm_title" => "Delete board?",
    "desktop.tab.remove_confirm_ok"    => "Delete board",
    "desktop.tab.remove_cancel"        => "Cancel",
    "desktop.tab.add_board"         => "+ board",
    "desktop.tab.add_tool"          => "+ pin",
    "desktop.tab.add_tool_title"    => "find and pin a tool to this board",
    "desktop.tab.search"            => "search ",
    "desktop.tab.settings"          => "settings",
    "desktop.tab.done"              => "✓ done",
    "desktop.tab.edit"              => "✎ edit",
    "desktop.tab.pin_color"         => "color",
    "desktop.tab.pin_color_disabled" => "select a pin to edit color",
    "desktop.tags.header"           => "tags",
    "desktop.edit.banner"           => "EDIT MODE · m move · hjkl/arrows focus · [ ] swap · p pin · D delete",
    "desktop.edit.moving_banner"    => "MOVING {tool} · hjkl/arrows move · Enter commit · Esc cancel",
    "desktop.empty.add_tool"        => "+ pin",
    "desktop.status.board_prefix"   => "board ",
    "desktop.status.pinned_suffix"  => " · {pinned}/{total} pinned",

    // Command palette (palette.rs).
    "palette.placeholder"           => "search tools, paste, or type a command…",
    "palette.empty_toolbox"         => "toolbox empty.",
    "palette.no_match"              => "no tools match \"{needle}\"",
    "palette.row.pin"               => "+ pin",
    "palette.row.pinned"            => "pinned",
    "palette.footer.navigate"       => "navigate",
    "palette.footer.open"           => "open",
    "palette.footer.open_pin"       => "open + pin",
    "palette.footer.summary"        => "{count} tools · {total} boards",

    // Settings modal (features/tweaks/settings.rs) and toggle states
    // (features/tweaks/mod.rs).
    "settings.title"                => "settings",
    "settings.close"                => "close",
    "settings.section.theme"        => "theme",
    "settings.section.layout"       => "layout",
    "settings.section.language"     => "language",
    "settings.radio.mode"           => "mode",
    "settings.radio.accent"         => "accent",
    "settings.radio.locale"         => "locale",
    "settings.theme.dark"           => "dark",
    "settings.theme.light"          => "light",
    "settings.accent.green"         => "green",
    "settings.accent.amber"         => "amber",
    "settings.accent.cyan"          => "cyan",
    "settings.accent.pink"          => "pink",
    "settings.toggle.peg_holes"     => "peg holes",
    "settings.toggle.on"            => "on",
    "settings.toggle.off"           => "off",
    "settings.section.host"                => "host",
    "settings.toggle.local_http_host"      => "local HTTP host",
    "settings.toggle.local_http_host_help" => "serve the REST/MCP host from this app; restart to apply",

    // Chrome-extension / popup view (surfaces/popup.rs).
    "popup.tag"                     => "upeg · popup",
    "popup.open_desktop_title"      => "open the full pegboard in Desktop with this Board",
    "popup.open_desktop_button"     => "Open Desktop ⤢",
    "popup.boards_aria"             => "Boards",
    "popup.board_tab_title"         => "Board {label}",
    "popup.search_placeholder"      => "search tools…",
    "popup.no_match"                => "no tools match \"{needle}\"",
    "popup.footer.summary"          => "{total} tools shown",

    // Pegboard pin toolkit badges (only). Badges are uppercase to match the
    // existing visual treatment; translations follow that convention
    // per locale (Korean uses non-uppercased nouns since the script
    // doesn't have a case form). See flutter_app/lib/src/widgets/pin.dart.
    "pin.toolkit.convert"        => "CONVERT",
    "pin.toolkit.generate"       => "GENERATE",
    "pin.toolkit.memo"           => "MEMO",
    "pin.toolkit.live"           => "LIVE",
    "pin.toolkit.live_eth"       => "LIVE · ETH",
    "pin.toolkit.embed_saas"     => "EMBED · SaaS",
    "pin.toolkit.action"         => "ACTION",

    // Pin context-menu entries — shared between the visible popup menu
    // and the assistive-technology CustomSemanticsAction labels
    // (flutter_app/lib/src/widgets/pin.dart).
    "pin.menu.open"              => "open",
    "pin.menu.edit_color"        => "edit color",
    "pin.menu.reset_size"        => "reset size",
    "pin.menu.unpin"             => "unpin",

    // Pin resize (SE-corner handle + keyboard resize mode).
    "pin.resize.handle_tooltip"  => "resize pin",
    "pin.resize.banner"          => "resizing {tool} · {cols}x{rows} · Enter commit · Esc cancel · 0 reset",

    // Pin accessibility semantics (flutter_app/lib/src/widgets/pin.dart).
    // `a11y.pin.label` is the Semantics label ({name} = tool label or
    // id, {kind} = PinKind badge text); the `result_*` / `running` /
    // `stale` keys compose the Semantics value announced by screen
    // readers when an inline run lands.
    "a11y.pin.label"             => "{name} · {kind} pin",
    "a11y.pin.label_plain"       => "{name} pin",
    "a11y.pin.hint_run"          => "tap to run",
    "a11y.pin.running"           => "running",
    "a11y.pin.stale"             => "stale result",
    "a11y.pin.result_ok"         => "OK · {preview}",
    "a11y.pin.result_ok_empty"   => "OK",
    "a11y.pin.result_error"      => "ERROR · {preview}",
    "a11y.pin.result_error_empty" => "ERROR",
    "a11y.pin.restored"          => "restored result",

    // Restored last-run timestamp badge (pin.dart): shown when a pin
    // renders an outcome hydrated from the persisted last-outcome store
    // (schema v3) instead of a fresh dispatch in this session. The
    // relative bucket ("just now" / minutes / hours / days) is picked by
    // the Dart-side formatter (i18n/relative_time.dart).
    "pin.last_run.just_now"      => "last run · just now",
    "pin.last_run.minutes_ago"   => "last run · {minutes}m ago",
    "pin.last_run.hours_ago"     => "last run · {hours}h ago",
    "pin.last_run.days_ago"      => "last run · {days}d ago",

    // "Provider not configured" honest state + CLI fix-it hint
    // (flutter_app/lib/src/widgets/provider_not_configured_body.dart).
    // The concrete `upeg credential add <name>` command line is
    // assembled on the Dart side — CLI syntax is not translatable copy.
    "pin.provider_not_configured.label"        => "needs setup",
    "pin.provider_not_configured.hint"         => "provider not configured",
    "pin.provider_not_configured.cli_hint"     => "Set up a credential in the terminal:",
    "pin.provider_not_configured.copy_tooltip" => "copy command",

    // Expanded modal — shared action labels with F-key keyboard hints
    // (expanded_modal/bespoke_forms.rs + generic_form.rs + embed.rs).
    "modal.action.run"              => "Run [F1]",
    "modal.action.regen"            => "Regen [F1]",
    "modal.action.copy"             => "Copy [F2]",
    "modal.action.pin"              => "Pin [F3]",
    "modal.action.pinned"           => "Pinned ✓ [F3]",
    "modal.action.reload"           => "Reload [F1]",
    "modal.action.remap_selectors"  => "Remap selectors [F2]",
    "modal.action.run_selectors"    => "Run selectors [F4]",
    "modal.header.close"            => "esc · close",
    "modal.tag.input_hex"           => "INPUT · hex",
    "modal.tag.output_u128"         => "OUTPUT · u128",
    "modal.tag.output_uuid_v7"      => "OUTPUT · uuid v7",
    "modal.tag.input"               => "INPUT",
    "modal.tag.output"              => "OUTPUT",
    "modal.pill.live_output"        => "Live output",
    "modal.pill.history"            => "history",
    "modal.pill.permissions_clipboard" => "permissions: clipboard",
    "modal.pill.permissions_none"   => "permissions: —",
    "modal.pill.uuid_v7"            => "v7 · time-ordered",
    "modal.generic.no_inputs"       => "(no inputs — this tool runs without arguments)",
    "modal.generic.press_to_run"    => "(press [F1] Run to dispatch this tool)",
    "modal.generic.required"        => "REQUIRED",
    "modal.generic.error_prefix"    => "error: {msg}",
    "modal.presentation.search"     => "Search results",
    "modal.presentation.empty"      => "No matching rows",
    "modal.presentation.board_changed_not_refreshed" => "The board changed; the list was not refreshed.",
    "modal.presentation.write_refresh_failed" => "The write succeeded, but the list refresh failed.",
    "modal.presentation.board_changed_run_again" => "The board changed; run the list again.",

    // Generic-form field validation errors
    // (expanded_modal/form_validation.dart). `FieldValidationError`
    // carries only the key + args; the widget resolves the localized
    // text through `t()`.
    "modal.validation.required"        => "required",
    "modal.validation.invalid_value"   => "invalid value",
    "modal.validation.not_a_number"    => "not a number",
    "modal.validation.not_an_integer"  => "not an integer",
    "modal.validation.invalid_format"  => "invalid format",
    "modal.validation.not_a_url"       => "not a url",
    "modal.validation.min"             => "min {value}",
    "modal.validation.max"             => "max {value}",

    // Structured-output URL action (expanded_modal/structured_output.dart).
    "modal.output.open_url"         => "open",
    "modal.footer.source_prefix"    => "source: ",
    "modal.footer.source_body"      => "static · #[upeg::tool] · invoker={invoker}",
    "modal.footer.surfaces"         => "surfaces: {label}",
    "modal.embed.tag"               => "EMBED · WebView (sandboxed)",
    "modal.embed.bindings_header"   => "SELECTOR BINDINGS · {count}",
    "modal.embed.no_url"            => "No URL configured for this Embed tool",
    "modal.embed.set_via"           => "Set one via ",
    "modal.embed.set_or_add"        => " (Rust startup), or add ",
    "modal.embed.set_to_toml"       => " to its TOML manifest.",
    "modal.embed.url_arrow"         => "→ {url}",
    "modal.embed.iframe_title"      => "{tool_id} sandboxed embed",

    // Shared strings used by more than one Flutter surface.
    "common.unknown_tool"           => "unknown tool: {tool_id}",
    "common.search_failed"          => "search failed: {msg}",

    // Bottom status bar (flutter_app/lib/src/widgets/status_bar.dart).
    "desktop.status.pinned_count"   => " · {count} pinned",
    "desktop.status.paused"         => "paused",
    "desktop.status.imports"        => "imports {count}",
    "desktop.status.imports_loading" => "imports loading…",

    // Board tab bar dialogs/menus (flutter_app/lib/src/widgets/board_tabs.dart).
    "desktop.tab.load_failed"          => "failed to load boards: {msg}",
    "desktop.tab.menu_rename"          => "Rename…",
    "desktop.tab.menu_delete"          => "Delete",
    "desktop.tab.create_prompt"        => "New board",
    "desktop.tab.create_confirm"       => "Create",
    "desktop.tab.rename_prompt_titled" => "Rename \"{title}\"",
    "desktop.tab.remove_confirm_body"  => "This drops the board and its pinned tools.",

    // Empty-pegboard onboarding card (flutter_app/lib/src/widgets/empty_board.dart).
    "empty.pegboard.title"          => "pegboard is empty",
    "empty.pegboard.hint"           => "This board is empty — pin a tool from the palette.",

    // Popup landing page (flutter_app/lib/src/pages/popup_page.dart).
    "popup.open_desktop"            => "open desktop",
    "popup.catalogue_failed"        => "catalogue failed: {msg}",
    "popup.pinned_section"          => "PINNED · {board}",
    "popup.result.ok"               => "OK",
    "popup.result.error"            => "ERROR",
    "popup.result.empty_output"     => "(no output)",
    "popup.result.copy_tooltip"     => "copy result [F2]",

    // Embed surfaces (embed_page.dart, webview_panel.dart,
    // controlled_embed/{surface,tile,debug_modal}.dart).
    "embed.url_unavailable"         => "embed url not available for {tool_id}",
    "embed.open_externally"         => "open externally",
    "embed.load_failed_hint"        => "This page didn't load. Open it in an external browser.",
    "controlled_embed.inline_run_unsupported" => "inline Run isn't supported on this platform — open in an external browser",
    "controlled_embed.press_run"    => "press Run to fetch outputs",
    "controlled_embed.no_outputs"   => "(no outputs)",
    "controlled_embed.debug_button" => "Debug",
    "controlled_embed.session_unavailable" => "Browser session unavailable: {error}",

    // Expanded modal shell + outcome block
    // (flutter_app/lib/src/pages/expanded_modal_page.dart,
    // dispatch_result_dialog.dart).
    "modal.no_output"               => "(no output)",
    "modal.outcome.ok"              => "ok",
    "modal.outcome.error"           => "error",
    "modal.outcome.details"         => "details",
    "modal.output.primary"          => "primary",
    "modal.pin.no_active_board"     => "no active board to pin into",
    "modal.pin.failed"              => "pin failed: {msg}",
    "modal.pin.success"             => "pinned {tool_id} to {board}",
    "modal.footer.toolkit_prefix"   => "toolkit: ",
    "modal.footer.invoker_prefix"   => "invoker: ",
    "modal.action.run_short"        => "Run",
    "modal.action.cancel_run"       => "Cancel run",

    // Approval barrier confirmation
    // (flutter_app/lib/src/widgets/approval_confirm_dialog.dart). The
    // `denied_*` copy is for a Chain whose manifest does not list this
    // surface among its approvers — it explains instead of offering a
    // button that would change nothing.
    "modal.approval.title"          => "Approval required",
    "modal.approval.body"           => "{tool} has a step that waits for a person to approve it. Approve and run it now?",
    "modal.approval.approve"        => "Approve & run",
    "modal.approval.cancel"         => "Cancel",
    "modal.approval.denied_body"    => "{tool} only accepts approval from: {surfaces}. This desktop app is not one of them, so the run would stop at the barrier.",
    "modal.approval.dismiss"        => "Got it",

    // Bespoke forms (hex_to_dec_form.dart, uuid_v7_form.dart).
    "modal.hex.hint"                => "e.g. ff or 0xCAFE",
    "modal.hex.empty"               => "(empty)",
    "modal.hex.invalid"             => "not a valid hex value",
    "modal.hex.shortcut_hints"      => "[F1] run · [F2] copy",

    // Pin color dialog (flutter_app/lib/src/widgets/pin_color_dialog.dart).
    "pin_color.title"               => "Pin Color",
    "pin_color.invalid_hex"         => "Invalid HEX color",
    "pin_color.save"                => "Save",
    "pin_color.reset"               => "Reset",

    // Settings load failure (tweaks_form.dart).
    "settings.load_failed"          => "failed to load tweaks: {msg}",

    // "Unsupported on this surface" honest state
    // (surface_unsupported_body.dart).
    "surface.unsupported.label"       => "not supported on this surface",
    "surface.unsupported.attach_hint" => "can run via host attach — connect a host in Settings",
    "surface.unsupported.hint.no_process_spawn"  => "needs a subprocess — desktop only",
    "surface.unsupported.hint.no_loader_runtime" => "needs the native runtime — desktop only",
    "surface.unsupported.hint.no_wasm_host"      => "needs the wasm plugin host — desktop only",
    "surface.unsupported.hint.native_only_tool"  => "native-only tool — desktop only",

    // Full provider-not-configured message (lib/src/tool_roles.dart).
    "pin.provider_not_configured.message" => "needs setup (provider not configured)",

    // Keyboard cheatsheet (widgets/cheatsheet_overlay.dart). Structure
    // comes from `upeg_core::binding_catalog()`; these keys are its
    // `keys.scope.*` / `keys.cmd.*` labels. Wording follows
    // docs/ui-ux-surface-contract.md. The coverage test
    // `all_cheatsheet_label_keys_exist_in_both_locales` pins
    // completeness against the catalog.
    "keys.title"                   => "Keyboard shortcuts",
    "keys.footer.close"            => "close",
    "keys.requires_focus"          => "focused pin",

    "keys.scope.board"             => "Board",
    "keys.scope.detail"            => "Detail / modal",
    "keys.scope.form"              => "Form",
    "keys.scope.settings"          => "Settings",
    "keys.scope.board_editor"      => "Board editor",
    "keys.scope.confirm"           => "Confirm dialogs",
    "keys.scope.tool_picker"       => "Tool picker / palette",
    "keys.scope.filter_bar"        => "Filter bar",
    "keys.scope.right_pane"        => "Right pane",
    "keys.scope.moving"            => "Move pin",
    "keys.scope.resize"            => "Resize pin",

    "keys.cmd.focus_move"          => "Move board focus",
    "keys.cmd.focus_cycle"         => "Move focus",
    "keys.cmd.run"                 => "Run",
    "keys.cmd.open"                => "Open / Inspect",
    "keys.cmd.search"              => "Search",
    "keys.cmd.copy"                => "Copy current output",
    "keys.cmd.close"               => "Back / Close",
    "keys.cmd.quit"                => "Quit (confirm dialog)",
    "keys.cmd.settings"            => "Settings",
    "keys.cmd.cheatsheet"          => "Keyboard cheatsheet",
    "keys.cmd.cycle_board_filter"  => "Cycle board filter",
    "keys.cmd.cycle_tag_filter"    => "Cycle tag filter",
    "keys.cmd.clear_board_filter"  => "Clear board filter",
    "keys.cmd.switch_board"        => "Switch to board slot",
    "keys.cmd.new_board"           => "New board",
    "keys.cmd.rename_board"        => "Rename board",
    "keys.cmd.delete_board"        => "Delete board",
    "keys.cmd.open_tool_picker"    => "Open add-tool picker",
    "keys.cmd.toggle_pin"          => "Toggle pin",
    "keys.cmd.edit_pin_color"      => "Edit pin color",
    "keys.cmd.start_move"          => "Move pin (coordinate move)",
    "keys.cmd.start_resize"        => "Resize pin",
    "keys.cmd.reorder"             => "Reorder pin",
    "keys.cmd.move_pin_slot"       => "Move pin one slot",
    "keys.cmd.confirm"             => "Confirm",
    "keys.cmd.cancel"              => "Cancel",
    "keys.cmd.commit"              => "Commit",
    "keys.cmd.backspace"           => "Delete character",
    "keys.cmd.clear_input"         => "Clear input",
    "keys.cmd.field_move"          => "Move between fields",
    "keys.cmd.caret_move"          => "Move caret",
    "keys.cmd.adjust_setting"      => "Change the focused setting",
    "keys.cmd.select_move"         => "Move selection",
    "keys.cmd.scroll"              => "Scroll",
    "keys.cmd.page"                => "Page up / down",
    "keys.cmd.jump_edge"           => "Jump to start / end",
    "keys.cmd.moving_step"         => "Move the pin",
    "keys.cmd.resize_wider"        => "Grow one column",
    "keys.cmd.resize_narrower"     => "Shrink one column",
    "keys.cmd.resize_taller"       => "Grow one row",
    "keys.cmd.resize_shorter"      => "Shrink one row",
    "keys.cmd.reset_span"          => "Reset to manifest footprint",

    // Pin context menu key hints reuse `keys.cmd.*` above.

    // Tool metadata translations. Keys take the shape
    // `tool.<tool_id>.label` / `tool.<tool_id>.description`. Missing
    // keys fall through to the static `ToolMeta` strings — adding a
    // tool here is optional.
    "tool.num.hex_to_decimal.label"       => "Hex → Decimal",
    "tool.num.hex_to_decimal.description" => "Convert a hexadecimal value to its decimal equivalent.",
    "tool.id.uuid_v7.label"                => "UUID v7",
    "tool.id.uuid_v7.description"          => "Generate a time-ordered UUID v7 identifier.",
    "tool.convert.json_format.label"      => "JSON format",
    "tool.convert.json_format.description" => "Pretty-print a JSON string with 2-space indentation.",
    "tool.text.regex_match.label"           => "Regex match",
    "tool.text.regex_match.description"     => "Test a regex pattern against a string and return the matches.",
};

static KO: Map<&'static str, &'static str> = phf_map! {
    "empty.board.title"            => "보드 \"{board_title}\" 가 비어 있습니다",
    "empty.tag.title"              => "이 태그({tag})에 핀된 도구가 없습니다",
    "empty.add_hint.before"        => "위쪽 ",
    "empty.add_hint.between"       => " 버튼 또는 ",
    "empty.add_hint.after"         => " 로 도구를 핀하세요.",
    "empty.suggestion_header"      => "추천 시작 도구",
    "empty.find_tool_button"       => "도구 핀하기",

    "embed.note.daemon_only"       => "Daemon WebView 명시 시작 후에만 호출됩니다 (PRD §5.4). ",
    "embed.note.selector_rebind"   => "Embed selector 매핑이 깨지면 사용자가 재지정 (PRD §4.3).",

    "desktop.tab.rename_prompt"     => "보드 이름 바꾸기",
    "desktop.tab.remove_confirm"    => "보드 \"{title}\"를 제거할까요?",
    "desktop.tab.rename_title"      => "보드 이름 바꾸기",
    "desktop.tab.remove_title"      => "보드 제거",
    "desktop.tab.new_placeholder"   => "보드 이름",
    "desktop.tab.add_confirm_title" => "보드 추가",
    "desktop.tab.add_cancel_title"  => "취소",
    "desktop.tab.rename_placeholder"   => "새 보드 이름",
    "desktop.tab.rename_confirm_title" => "이름 변경",
    "desktop.tab.rename_cancel_title"  => "이름 변경 취소",
    "desktop.tab.remove_confirm_title" => "보드를 삭제할까요?",
    "desktop.tab.remove_confirm_ok"    => "삭제",
    "desktop.tab.remove_cancel"        => "취소",
    "desktop.tab.add_board"         => "+ 보드",
    "desktop.tab.add_tool"          => "+ 핀",
    "desktop.tab.add_tool_title"    => "도구를 찾아 이 보드에 핀합니다",
    "desktop.tab.search"            => "검색 ",
    "desktop.tab.settings"          => "설정",
    "desktop.tab.done"              => "✓ 완료",
    "desktop.tab.edit"              => "✎ 편집",
    "desktop.tab.pin_color"         => "색상",
    "desktop.tab.pin_color_disabled" => "색상을 바꿀 핀을 선택하세요",
    "desktop.tags.header"           => "태그",
    "desktop.edit.banner"           => "편집 모드 · m 이동 · hjkl/화살표 포커스 · [ ] 교환 · p 핀 · D 삭제",
    "desktop.edit.moving_banner"    => "{tool} 이동 중 · hjkl/화살표 이동 · Enter 확정 · Esc 취소",
    "desktop.empty.add_tool"        => "+ 핀",
    "desktop.status.board_prefix"   => "보드 ",
    "desktop.status.pinned_suffix"  => " · {pinned}/{total} 핀됨",

    "palette.placeholder"           => "도구 검색, 붙여넣기 또는 명령 입력…",
    "palette.empty_toolbox"         => "툴박스가 비어 있습니다.",
    "palette.no_match"              => "\"{needle}\"와(과) 일치하는 도구가 없습니다",
    "palette.row.pin"               => "+ 핀",
    "palette.row.pinned"            => "핀됨",
    "palette.footer.navigate"       => "이동",
    "palette.footer.open"           => "열기",
    "palette.footer.open_pin"       => "열기 + 핀",
    "palette.footer.summary"        => "도구 {count}개 · 보드 {total}개",

    "settings.title"                => "설정",
    "settings.close"                => "닫기",
    "settings.section.theme"        => "테마",
    "settings.section.layout"       => "레이아웃",
    "settings.section.language"     => "언어",
    "settings.radio.mode"           => "모드",
    "settings.radio.accent"         => "강조색",
    "settings.radio.locale"         => "언어",
    "settings.theme.dark"           => "다크",
    "settings.theme.light"          => "라이트",
    "settings.accent.green"         => "그린",
    "settings.accent.amber"         => "앰버",
    "settings.accent.cyan"          => "시안",
    "settings.accent.pink"          => "핑크",
    "settings.toggle.peg_holes"     => "페그 구멍",
    "settings.toggle.on"            => "켬",
    "settings.toggle.off"           => "끔",
    "settings.section.host"                => "호스트",
    "settings.toggle.local_http_host"      => "로컬 HTTP host",
    "settings.toggle.local_http_host_help" => "이 앱이 REST/MCP host가 된다. 적용하려면 재시작한다",

    "popup.tag"                     => "upeg · 팝업",
    "popup.open_desktop_title"      => "이 보드를 데스크탑 페그보드에서 열기",
    "popup.open_desktop_button"     => "데스크탑 열기 ⤢",
    "popup.boards_aria"             => "보드",
    "popup.board_tab_title"         => "보드 {label}",
    "popup.search_placeholder"      => "도구 검색…",
    "popup.no_match"                => "\"{needle}\"와(과) 일치하는 도구가 없습니다",
    "popup.footer.summary"          => "도구 {total}개 표시 중",

    "pin.toolkit.convert"        => "변환",
    "pin.toolkit.generate"       => "생성",
    "pin.toolkit.memo"           => "메모",
    "pin.toolkit.live"           => "라이브",
    "pin.toolkit.live_eth"       => "라이브 · ETH",
    "pin.toolkit.embed_saas"     => "임베드 · SaaS",
    "pin.toolkit.action"         => "액션",

    "pin.menu.open"              => "열기",
    "pin.menu.edit_color"        => "색상 변경",
    "pin.menu.reset_size"        => "크기 재설정",
    "pin.menu.unpin"             => "핀 해제",

    "pin.resize.handle_tooltip"  => "핀 크기 조절",
    "pin.resize.banner"          => "{tool} 크기 조절 중 · {cols}x{rows} · Enter 확정 · Esc 취소 · 0 초기화",

    "a11y.pin.label"             => "{name} · {kind} 핀",
    "a11y.pin.label_plain"       => "{name} 핀",
    "a11y.pin.hint_run"          => "탭하면 실행",
    "a11y.pin.running"           => "실행 중",
    "a11y.pin.stale"             => "오래된 결과",
    "a11y.pin.result_ok"         => "성공 · {preview}",
    "a11y.pin.result_ok_empty"   => "성공",
    "a11y.pin.result_error"      => "오류 · {preview}",
    "a11y.pin.result_error_empty" => "오류",
    "a11y.pin.restored"          => "복원된 결과",

    "pin.last_run.just_now"      => "지난 실행 · 방금",
    "pin.last_run.minutes_ago"   => "지난 실행 · {minutes}분 전",
    "pin.last_run.hours_ago"     => "지난 실행 · {hours}시간 전",
    "pin.last_run.days_ago"      => "지난 실행 · {days}일 전",

    "pin.provider_not_configured.label"        => "설정 필요",
    "pin.provider_not_configured.hint"         => "프로바이더가 설정되지 않았습니다",
    "pin.provider_not_configured.cli_hint"     => "터미널에서 credential을 설정하세요:",
    "pin.provider_not_configured.copy_tooltip" => "명령 복사",

    "modal.action.run"              => "실행 [F1]",
    "modal.action.regen"            => "재생성 [F1]",
    "modal.action.copy"             => "복사 [F2]",
    "modal.action.pin"              => "핀 [F3]",
    "modal.action.pinned"           => "핀됨 ✓ [F3]",
    "modal.action.reload"           => "새로고침 [F1]",
    "modal.action.remap_selectors"  => "셀렉터 재매핑 [F2]",
    "modal.action.run_selectors"    => "셀렉터 실행 [F4]",
    "modal.header.close"            => "esc · 닫기",
    "modal.tag.input_hex"           => "입력 · hex",
    "modal.tag.output_u128"         => "출력 · u128",
    "modal.tag.output_uuid_v7"      => "출력 · uuid v7",
    "modal.tag.input"               => "입력",
    "modal.tag.output"              => "출력",
    "modal.pill.live_output"        => "실시간 출력",
    "modal.pill.history"            => "기록",
    "modal.pill.permissions_clipboard" => "권한: 클립보드",
    "modal.pill.permissions_none"   => "권한: —",
    "modal.pill.uuid_v7"            => "v7 · 시간 정렬",
    "modal.generic.no_inputs"       => "(입력 없음 — 인자 없이 실행됩니다)",
    "modal.generic.press_to_run"    => "([F1] 실행을 눌러 이 도구를 호출)",
    "modal.generic.required"        => "필수",
    "modal.generic.error_prefix"    => "오류: {msg}",
    "modal.presentation.search"     => "결과 검색",
    "modal.presentation.empty"      => "일치하는 행이 없습니다",
    "modal.presentation.board_changed_not_refreshed" => "보드가 변경되어 목록을 새로 고치지 못했습니다.",
    "modal.presentation.write_refresh_failed" => "쓰기는 성공했지만 목록을 새로 고치지 못했습니다.",
    "modal.presentation.board_changed_run_again" => "보드가 변경되었습니다. 목록을 다시 실행하세요.",

    "modal.validation.required"        => "필수 입력입니다",
    "modal.validation.invalid_value"   => "올바르지 않은 값입니다",
    "modal.validation.not_a_number"    => "숫자가 아닙니다",
    "modal.validation.not_an_integer"  => "정수가 아닙니다",
    "modal.validation.invalid_format"  => "형식이 올바르지 않습니다",
    "modal.validation.not_a_url"       => "URL이 아닙니다",
    "modal.validation.min"             => "최소 {value}",
    "modal.validation.max"             => "최대 {value}",

    "modal.output.open_url"         => "열기",
    "modal.footer.source_prefix"    => "출처: ",
    "modal.footer.source_body"      => "static · #[upeg::tool] · invoker={invoker}",
    "modal.footer.surfaces"         => "표면: {label}",
    "modal.embed.tag"               => "임베드 · WebView (샌드박스)",
    "modal.embed.bindings_header"   => "셀렉터 바인딩 · {count}",
    "modal.embed.no_url"            => "이 임베드 도구에 URL이 설정되어 있지 않습니다",
    "modal.embed.set_via"           => "다음으로 설정: ",
    "modal.embed.set_or_add"        => " (Rust 시작), 또는 ",
    "modal.embed.set_to_toml"       => " 를 TOML 매니페스트에 추가.",
    "modal.embed.url_arrow"         => "→ {url}",
    "modal.embed.iframe_title"      => "{tool_id} 샌드박스 임베드",

    "common.unknown_tool"           => "알 수 없는 도구: {tool_id}",
    "common.search_failed"          => "검색 실패: {msg}",

    "desktop.status.pinned_count"   => " · {count}개 핀됨",
    "desktop.status.paused"         => "일시정지",
    "desktop.status.imports"        => "임포트 {count}",
    "desktop.status.imports_loading" => "임포트 로딩 중…",

    "desktop.tab.load_failed"          => "보드 목록을 불러오지 못했습니다: {msg}",
    "desktop.tab.menu_rename"          => "이름 바꾸기…",
    "desktop.tab.menu_delete"          => "삭제",
    "desktop.tab.create_prompt"        => "새 보드",
    "desktop.tab.create_confirm"       => "만들기",
    "desktop.tab.rename_prompt_titled" => "\"{title}\" 이름 바꾸기",
    "desktop.tab.remove_confirm_body"  => "보드와 핀된 도구가 함께 삭제됩니다.",

    "empty.pegboard.title"          => "페그보드가 비어 있습니다",
    "empty.pegboard.hint"           => "이 보드는 비어 있습니다 — 팔레트에서 도구를 핀하세요.",

    "popup.open_desktop"            => "데스크탑 열기",
    "popup.catalogue_failed"        => "카탈로그 로드 실패: {msg}",
    "popup.pinned_section"          => "고정됨 · {board}",
    "popup.result.ok"               => "성공",
    "popup.result.error"            => "오류",
    "popup.result.empty_output"     => "(출력 없음)",
    "popup.result.copy_tooltip"     => "결과 복사 [F2]",

    "embed.url_unavailable"         => "임베드 URL이 없습니다: {tool_id}",
    "embed.open_externally"         => "외부 브라우저에서 열기",
    "embed.load_failed_hint"        => "이 페이지가 로드되지 않습니다. 외부 브라우저에서 여세요.",
    "controlled_embed.inline_run_unsupported" => "이 플랫폼에서는 인라인 실행 미지원 — 외부 브라우저에서 열기",
    "controlled_embed.press_run"    => "실행을 눌러 출력을 가져오세요",
    "controlled_embed.no_outputs"   => "(출력 없음)",
    "controlled_embed.debug_button" => "디버그",
    "controlled_embed.session_unavailable" => "브라우저 세션을 사용할 수 없습니다: {error}",

    "modal.no_output"               => "(출력 없음)",
    "modal.outcome.ok"              => "성공",
    "modal.outcome.error"           => "오류",
    "modal.outcome.details"         => "상세",
    "modal.output.primary"          => "프라이머리",
    "modal.pin.no_active_board"     => "핀할 활성 보드가 없습니다",
    "modal.pin.failed"              => "핀 실패: {msg}",
    "modal.pin.success"             => "{tool_id}을(를) {board}에 핀했습니다",
    "modal.footer.toolkit_prefix"   => "툴킷: ",
    "modal.footer.invoker_prefix"   => "인보커: ",
    "modal.action.run_short"        => "실행",
    "modal.action.cancel_run"       => "실행 취소",

    "modal.approval.title"          => "승인이 필요합니다",
    "modal.approval.body"           => "{tool}에는 사람이 승인해야 넘어가는 단계가 있습니다. 승인하고 지금 실행할까요?",
    "modal.approval.approve"        => "승인하고 실행",
    "modal.approval.cancel"         => "취소",
    "modal.approval.denied_body"    => "{tool}은(는) {surfaces}의 승인만 인정합니다. 데스크톱 앱은 여기에 없어서 실행이 승인 장벽에서 멈춥니다.",
    "modal.approval.dismiss"        => "알겠습니다",

    "modal.hex.hint"                => "예: ff 또는 0xCAFE",
    "modal.hex.empty"               => "(비어 있음)",
    "modal.hex.invalid"             => "올바른 hex 값이 아닙니다",
    "modal.hex.shortcut_hints"      => "[F1] 실행 · [F2] 복사",

    "pin_color.title"               => "핀 색상",
    "pin_color.invalid_hex"         => "잘못된 HEX 색상입니다",
    "pin_color.save"                => "저장",
    "pin_color.reset"               => "초기화",

    "settings.load_failed"          => "설정을 불러오지 못했습니다: {msg}",

    "surface.unsupported.label"       => "이 표면에서는 미지원",
    "surface.unsupported.attach_hint" => "호스트 연결로 실행 가능 — 설정에서 host를 연결하세요",
    "surface.unsupported.hint.no_process_spawn"  => "서브프로세스 필요 — 데스크탑 전용",
    "surface.unsupported.hint.no_loader_runtime" => "네이티브 런타임 필요 — 데스크탑 전용",
    "surface.unsupported.hint.no_wasm_host"      => "wasm 플러그인 호스트 필요 — 데스크탑 전용",
    "surface.unsupported.hint.native_only_tool"  => "네이티브 전용 도구 — 데스크탑 전용",

    "pin.provider_not_configured.message" => "설정 필요 (프로바이더가 설정되지 않았습니다)",

    // Keyboard cheatsheet — see the EN block for the catalog contract.
    "keys.title"                   => "키보드 단축키",
    "keys.footer.close"            => "닫기",
    "keys.requires_focus"          => "핀 포커스 필요",

    "keys.scope.board"             => "보드",
    "keys.scope.detail"            => "상세 / 모달",
    "keys.scope.form"              => "폼",
    "keys.scope.settings"          => "설정",
    "keys.scope.board_editor"      => "보드 편집기",
    "keys.scope.confirm"           => "확인 대화상자",
    "keys.scope.tool_picker"       => "도구 피커 / 팔레트",
    "keys.scope.filter_bar"        => "필터 바",
    "keys.scope.right_pane"        => "오른쪽 패널",
    "keys.scope.moving"            => "핀 이동",
    "keys.scope.resize"            => "핀 크기 조절",

    "keys.cmd.focus_move"          => "보드 포커스 이동",
    "keys.cmd.focus_cycle"         => "포커스 이동",
    "keys.cmd.run"                 => "실행",
    "keys.cmd.open"                => "열기 / 상세 보기",
    "keys.cmd.search"              => "검색",
    "keys.cmd.copy"                => "현재 출력 복사",
    "keys.cmd.close"               => "뒤로 / 닫기",
    "keys.cmd.quit"                => "종료 (확인 대화상자)",
    "keys.cmd.settings"            => "설정",
    "keys.cmd.cheatsheet"          => "키보드 치트시트",
    "keys.cmd.cycle_board_filter"  => "보드 필터 순환",
    "keys.cmd.cycle_tag_filter"    => "태그 필터 순환",
    "keys.cmd.clear_board_filter"  => "보드 필터 해제",
    "keys.cmd.switch_board"        => "보드 슬롯 전환",
    "keys.cmd.new_board"           => "새 보드",
    "keys.cmd.rename_board"        => "보드 이름 변경",
    "keys.cmd.delete_board"        => "보드 삭제",
    "keys.cmd.open_tool_picker"    => "도구 추가 피커 열기",
    "keys.cmd.toggle_pin"          => "핀 고정 / 해제",
    "keys.cmd.edit_pin_color"      => "핀 색상 편집",
    "keys.cmd.start_move"          => "핀 이동 (좌표 이동)",
    "keys.cmd.start_resize"        => "핀 크기 조절",
    "keys.cmd.reorder"             => "핀 순서 변경",
    "keys.cmd.move_pin_slot"       => "핀 한 슬롯 이동",
    "keys.cmd.confirm"             => "확인",
    "keys.cmd.cancel"              => "취소",
    "keys.cmd.commit"              => "확정",
    "keys.cmd.backspace"           => "글자 삭제",
    "keys.cmd.clear_input"         => "입력 지우기",
    "keys.cmd.field_move"          => "필드 간 이동",
    "keys.cmd.caret_move"          => "커서 이동",
    "keys.cmd.adjust_setting"      => "포커스된 설정 변경",
    "keys.cmd.select_move"         => "선택 이동",
    "keys.cmd.scroll"              => "스크롤",
    "keys.cmd.page"                => "페이지 위 / 아래",
    "keys.cmd.jump_edge"           => "처음 / 끝으로 이동",
    "keys.cmd.moving_step"         => "핀 이동",
    "keys.cmd.resize_wider"        => "한 열 넓히기",
    "keys.cmd.resize_narrower"     => "한 열 좁히기",
    "keys.cmd.resize_taller"       => "한 행 늘리기",
    "keys.cmd.resize_shorter"      => "한 행 줄이기",
    "keys.cmd.reset_span"          => "기본 크기로 재설정",

    "tool.num.hex_to_decimal.label"       => "Hex → 10진수",
    "tool.num.hex_to_decimal.description" => "16진수 값을 10진수로 변환합니다.",
    "tool.id.uuid_v7.label"                => "UUID v7",
    "tool.id.uuid_v7.description"          => "시간 정렬 UUID v7 식별자를 생성합니다.",
    "tool.convert.json_format.label"      => "JSON 정렬",
    "tool.convert.json_format.description" => "JSON 문자열을 2-스페이스 들여쓰기로 정렬합니다.",
    "tool.text.regex_match.label"           => "정규식 매치",
    "tool.text.regex_match.description"     => "문자열에 대해 정규식 패턴을 테스트하고 일치 결과를 반환합니다.",
};

/// Locales that ship with this catalog. The order is the order
/// surfaces should offer in any locale picker (English first as the
/// canonical fallback, Korean as the second shipping locale).
pub const SUPPORTED_LOCALES: &[Locale] = &[Locale::En, Locale::Ko];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_english_key_has_a_korean_translation() {
        // Adding an entry to EN without a matching KO entry would silently
        // serve English under Locale::Ko via the core fallback chain —
        // intentional for a future-language onboarding ramp, but unwanted
        // here because Ko is a shipping locale we want fully translated.
        let missing: Vec<&str> = EN
            .keys()
            .chain(surface_io::EN.keys())
            .chain(board_guidance::EN.keys())
            .chain(media::EN.keys())
            .copied()
            .filter(|key| catalog(Locale::Ko, key).is_none())
            .collect();
        assert!(
            missing.is_empty(),
            "Ko catalog is missing translations for: {missing:?}",
        );
    }

    #[test]
    fn there_are_no_orphan_korean_keys() {
        // A Ko-only key would never be reached — the core lookup hits Ko
        // first, but every call site identifies a key by the literal it
        // wrote in EN. An orphan Ko entry means a stale catalog row.
        let orphans: Vec<&str> = KO
            .keys()
            .chain(surface_io::KO.keys())
            .chain(board_guidance::KO.keys())
            .chain(media::KO.keys())
            .copied()
            .filter(|key| catalog(Locale::En, key).is_none())
            .collect();
        assert!(
            orphans.is_empty(),
            "Ko catalog has orphan keys (no matching En entry): {orphans:?}",
        );
    }

    #[test]
    fn all_cheatsheet_label_keys_exist_in_both_locales() {
        // The cheatsheet renders `upeg_core::binding_catalog()` labels
        // through this catalog — a missing key would leak the raw i18n
        // key (`???` marker chain) into the overlay.
        for locale in [Locale::En, Locale::Ko] {
            for scope_bindings in upeg_core::binding_catalog() {
                assert!(
                    catalog(locale, scope_bindings.label_key).is_some(),
                    "{locale:?} catalog is missing scope label {}",
                    scope_bindings.label_key,
                );
                for entry in &scope_bindings.entries {
                    assert!(
                        catalog(locale, entry.label_key).is_some(),
                        "{locale:?} catalog is missing entry label {}",
                        entry.label_key,
                    );
                }
            }
            // Overlay chrome keys are not part of the shared catalog
            // structure but still must exist in both locales.
            for key in ["keys.title", "keys.footer.close", "keys.requires_focus"] {
                assert!(
                    catalog(locale, key).is_some(),
                    "{locale:?} catalog is missing cheatsheet chrome key {key}",
                );
            }
        }
    }

    #[test]
    fn korean_locale_returns_korean_catalog_text() {
        assert_eq!(
            catalog(Locale::Ko, "empty.suggestion_header"),
            Some("추천 시작 도구"),
        );
    }

    #[test]
    fn english_locale_returns_english_catalog_text() {
        assert_eq!(
            catalog(Locale::En, "empty.suggestion_header"),
            Some("Suggested starters"),
        );
    }

    #[test]
    fn catalog_returns_none_for_unknown_keys() {
        assert_eq!(catalog(Locale::En, "no.such.key"), None);
        assert_eq!(catalog(Locale::Ko, "no.such.key"), None);
    }

    #[test]
    fn t_respects_the_locale_parameter() {
        assert_eq!(t("settings.theme.dark", Locale::En), "dark");
        assert_eq!(t("settings.theme.dark", Locale::Ko), "다크");
    }

    #[test]
    fn t_args_replaces_named_placeholders() {
        let out = t_args("popup.no_match", &[("needle", "uuid")], Locale::En);
        assert_eq!(out, "no tools match \"uuid\"");
    }

    #[test]
    fn supported_locales_are_exactly_english_and_korean() {
        // Pin the shipping locale set so adding a locale becomes a
        // deliberate change (catalog parity + Settings dropdown +
        // detect_from_str all have to update together).
        assert_eq!(SUPPORTED_LOCALES, &[Locale::En, Locale::Ko]);
    }

    #[test]
    fn pin_accessibility_keys_exist_in_both_locales() {
        // Keys consumed by Flutter Pin Semantics(label/value/hint), context-menu
        // labels, and CustomSemanticsAction labels. A missing key makes screen
        // readers announce the raw key.
        for key in [
            "a11y.pin.label",
            "a11y.pin.label_plain",
            "a11y.pin.hint_run",
            "a11y.pin.running",
            "a11y.pin.stale",
            "a11y.pin.result_ok",
            "a11y.pin.result_ok_empty",
            "a11y.pin.result_error",
            "a11y.pin.result_error_empty",
            "a11y.pin.restored",
            "pin.last_run.just_now",
            "pin.last_run.minutes_ago",
            "pin.last_run.hours_ago",
            "pin.last_run.days_ago",
            "pin.menu.open",
            "pin.menu.edit_color",
            "pin.menu.reset_size",
            "pin.menu.unpin",
            "pin.resize.handle_tooltip",
            "pin.resize.banner",
        ] {
            assert!(catalog(Locale::En, key).is_some(), "En missing {key}");
            assert!(catalog(Locale::Ko, key).is_some(), "Ko missing {key}");
        }
    }

    // ─── tool meta lookup ─────────────────────────────

    #[test]
    fn tool_namespace_keys_maintain_locale_parity() {
        let missing: Vec<&str> = EN
            .keys()
            .chain(surface_io::EN.keys())
            .chain(board_guidance::EN.keys())
            .chain(media::EN.keys())
            .copied()
            .filter(|key| key.starts_with("tool.") && catalog(Locale::Ko, key).is_none())
            .collect();
        assert!(
            missing.is_empty(),
            "Ko catalog missing tool-meta keys: {missing:?}",
        );
    }

    #[test]
    fn known_tool_keys_resolve_in_their_namespace() {
        for id in [
            "num.hex_to_decimal",
            "id.uuid_v7",
            "convert.json_format",
            "text.regex_match",
        ] {
            let label_key = format!("tool.{id}.label");
            let desc_key = format!("tool.{id}.description");
            assert!(catalog(Locale::En, &label_key).is_some());
            assert!(catalog(Locale::Ko, &label_key).is_some());
            assert!(catalog(Locale::En, &desc_key).is_some());
            assert!(catalog(Locale::Ko, &desc_key).is_some());
        }
    }
}
