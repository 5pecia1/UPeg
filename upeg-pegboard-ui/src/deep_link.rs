//! Desktop deep-link contract shared by the Chrome extension popup/content
//! entry points and the native Desktop launch path.
//!
//! The OS-level protocol registration is packaging-owned, but once the app is
//! launched with a `upeg://open?...` argument this module resolves it to the
//! same static Board/Tool ids the Flutter Desktop surface uses.

#[cfg(any(not(target_arch = "wasm32"), test))]
use upeg_core::Surface;
#[cfg(any(not(target_arch = "wasm32"), test))]
use upeg_runtime::toolbox_tool;

pub const DEFAULT_DESKTOP_BOARD: &str = "dev";
pub const DESKTOP_DEEP_LINK_BASE: &str = "upeg://open";
const DESKTOP_DEEP_LINK_QUERY_PREFIX: &str = "upeg://open?";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesktopLaunch {
    pub board: &'static str,
    pub tool: Option<&'static str>,
    pub input: Option<String>,
}

impl Default for DesktopLaunch {
    fn default() -> Self {
        Self {
            board: DEFAULT_DESKTOP_BOARD,
            tool: None,
            input: None,
        }
    }
}

pub fn encode_deep_link_value(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(*byte as char);
            }
            other => encoded.push_str(&format!("%{other:02X}")),
        }
    }
    encoded
}

pub fn desktop_deep_link(board: &str, tool: Option<&str>, input: Option<&str>) -> String {
    let mut link = String::from(DESKTOP_DEEP_LINK_QUERY_PREFIX);
    link.push_str("surface=ext");
    let board = board.trim();
    if !board.is_empty() {
        link.push_str("&board=");
        link.push_str(&encode_deep_link_value(board));
    }
    if let Some(tool) = tool.map(str::trim).filter(|tool| !tool.is_empty()) {
        link.push_str("&tool=");
        link.push_str(&encode_deep_link_value(tool));
    }
    if let Some(input) = input.map(str::trim).filter(|input| !input.is_empty()) {
        link.push_str("&input=");
        link.push_str(&encode_deep_link_value(input));
    }
    link
}

pub fn is_desktop_open_deep_link(raw: &str) -> bool {
    raw == DESKTOP_DEEP_LINK_BASE || raw.starts_with(DESKTOP_DEEP_LINK_QUERY_PREFIX)
}

#[cfg(any(not(target_arch = "wasm32"), test))]
pub fn desktop_launch_from_args_maybe<I, S>(args: I) -> Option<DesktopLaunch>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let raw = args
        .into_iter()
        .map(|arg| arg.as_ref().to_string())
        .find(|arg| is_desktop_open_deep_link(arg))?;
    let query = raw
        .strip_prefix(DESKTOP_DEEP_LINK_QUERY_PREFIX)
        .unwrap_or_default();
    let board = resolve_board(query_param(query, "board").as_deref());
    let tool = resolve_tool(query_param(query, "tool").as_deref());
    let input = query_param(query, "input");
    Some(DesktopLaunch { board, tool, input })
}

#[cfg(any(not(target_arch = "wasm32"), test))]
pub fn desktop_launch_from_args<I, S>(args: I) -> DesktopLaunch
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    desktop_launch_from_args_maybe(args).unwrap_or_default()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn desktop_launch_from_env() -> DesktopLaunch {
    desktop_launch_from_args(std::env::args().skip(1))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn desktop_launch_from_env_maybe() -> Option<DesktopLaunch> {
    desktop_launch_from_args_maybe(std::env::args().skip(1))
}

/// Resolve a single `upeg://open?...` URL to a [`DesktopLaunch`].
/// Used by the native popup view to route an in-app click/keyboard
/// activation through the same parser the CLI launch path uses,
/// keeping deep-link semantics in one place.
#[cfg(any(not(target_arch = "wasm32"), test))]
pub fn desktop_launch_from_url(url: &str) -> DesktopLaunch {
    desktop_launch_from_url_maybe(url).unwrap_or_default()
}

#[cfg(any(not(target_arch = "wasm32"), test))]
pub fn desktop_launch_from_url_maybe(url: &str) -> Option<DesktopLaunch> {
    desktop_launch_from_args_maybe([url])
}

#[cfg(target_arch = "wasm32")]
pub fn desktop_launch_from_env() -> DesktopLaunch {
    DesktopLaunch::default()
}

#[cfg(target_arch = "wasm32")]
pub fn desktop_launch_from_env_maybe() -> Option<DesktopLaunch> {
    None
}

#[cfg(any(not(target_arch = "wasm32"), test))]
fn query_param(query: &str, name: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        if key == name {
            percent_decode(value)
        } else {
            None
        }
    })
}

#[cfg(any(not(target_arch = "wasm32"), test))]
fn resolve_board(raw: Option<&str>) -> &'static str {
    let Some(candidate) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return DEFAULT_DESKTOP_BOARD;
    };
    crate::features::boards::load_boards()
        .unwrap_or_else(crate::features::boards::default_boards)
        .into_iter()
        .map(|board| board.key)
        .find(|board| *board == candidate)
        .unwrap_or(DEFAULT_DESKTOP_BOARD)
}

#[cfg(any(not(target_arch = "wasm32"), test))]
fn resolve_tool(raw: Option<&str>) -> Option<&'static str> {
    let _ = upeg_toolkit_catalog::register_embedded_metadata();
    raw.map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(toolbox_tool)
        .filter(|tool| tool.is_on_surface(Surface::Desktop))
        .map(|tool| tool.id)
}

#[cfg(any(not(target_arch = "wasm32"), test))]
fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let hi = bytes.get(i + 1).copied().and_then(hex_value)?;
                let lo = bytes.get(i + 2).copied().and_then(hex_value)?;
                out.push((hi << 4) | lo);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(any(not(target_arch = "wasm32"), test))]
const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_deep_link_encodes_board_tool_and_input() {
        let link = desktop_deep_link("개발용", Some("num.hex_to_decimal"), Some("0xff"));
        assert!(link.starts_with("upeg://open?"), "{link}");
        assert!(link.contains("surface=ext"), "{link}");
        assert!(link.contains("board=%EA%B0%9C%EB%B0%9C%EC%9A%A9"), "{link}");
        assert!(link.contains("tool=num.hex_to_decimal"), "{link}");
        assert!(link.contains("input=0xff"), "{link}");
    }

    #[test]
    fn desktop_deep_link_omits_empty_tool_and_input() {
        let link = desktop_deep_link("dev", Some("   "), Some(""));
        assert_eq!(link, "upeg://open?surface=ext&board=dev");
    }

    #[test]
    fn desktop_launch_resolves_known_board_tool_and_input() {
        let launch = desktop_launch_from_args([
            "upeg://open?surface=ext&board=dev&tool=num.hex_to_decimal&input=0x2a",
        ]);
        assert_eq!(launch.board, "dev");
        assert_eq!(launch.tool, Some("num.hex_to_decimal"));
        assert_eq!(launch.input.as_deref(), Some("0x2a"));
    }

    #[test]
    fn desktop_launch_percent_decodes_input() {
        let launch = desktop_launch_from_args([
            "upeg://open?surface=ext&board=dev&tool=num.hex_to_decimal&input=hello%20world",
        ]);
        assert_eq!(launch.input.as_deref(), Some("hello world"));
    }

    #[test]
    fn desktop_launch_drops_unknown_tool() {
        let launch =
            desktop_launch_from_args(["upeg://open?surface=ext&board=dev&tool=nope.missing"]);
        assert_eq!(launch.board, "dev");
        assert_eq!(launch.tool, None);
    }

    #[test]
    fn desktop_launch_drops_registered_non_desktop_tool() {
        let id = "test.deep_link.http_only";
        upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
            id,
            toolkit: "test",
            local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "test")
                .expect("test ToolMeta id must be canonical")
                .local(),
            tags: &[],
            display_label: "Test tool",
            description: "not available on desktop",
            input_spec: upeg_core::InputSpec::empty(),
            output_spec: upeg_core::OutputSpec::empty(),
            primary_output_id: None,
            effect: upeg_core::ToolEffect::Unknown,
            presentation: None,
            source: upeg_core::Source::UserInput,
            pin: upeg_core::PinKind::Inline,
            pegboard_units: upeg_core::PegboardUnits::U1,
            invoker: upeg_core::Invoker::External,
            surfaces: &[upeg_core::Surface::Http],
            boards: &[],
        });

        let launch =
            desktop_launch_from_args([format!("upeg://open?surface=ext&board=dev&tool={id}")]);
        assert_eq!(launch.board, "dev");
        assert_eq!(launch.tool, None);
    }

    #[test]
    fn desktop_launch_ignores_non_upeg_args() {
        let launch = desktop_launch_from_args(["--some-flag", "https://example.test"]);
        assert_eq!(launch, DesktopLaunch::default());
    }

    #[test]
    fn desktop_launch_maybe_returns_default_board_for_queryless_open_uri() {
        let launch = desktop_launch_from_url_maybe("upeg://open").expect("explicit open link");
        assert_eq!(launch, DesktopLaunch::default());
    }

    #[test]
    fn desktop_launch_maybe_rejects_uri_that_only_resembles_open() {
        let launch = desktop_launch_from_url_maybe("upeg://openly?tool=num.hex_to_decimal");
        assert!(launch.is_none());
    }

    #[test]
    fn desktop_launch_from_url_matches_args_parser() {
        let url = "upeg://open?surface=ext&board=dev&tool=num.hex_to_decimal";
        let from_url = desktop_launch_from_url(url);
        let from_args = desktop_launch_from_args([url]);
        assert_eq!(from_url, from_args);
        assert_eq!(from_url.tool, Some("num.hex_to_decimal"));
    }

    #[test]
    fn desktop_launch_from_url_rejects_non_upeg_input() {
        let launch = desktop_launch_from_url("https://example.test");
        assert_eq!(launch, DesktopLaunch::default());
    }
}
