//! Deep-link parsing/encoding FRB surface.
//!
//! `parse_launch_intent` turns a `upeg://open?...` URL into a typed
//! [`LaunchIntentDto`] the Dart observer reads field-by-field. The DTO
//! mirrors `upeg_pegboard_ui::deep_link::DesktopLaunch` but widens the
//! borrowed `&'static str` board/tool fields to owned strings so the
//! FRB sse codec can ferry them across the boundary.
//!
//! `parse_launch_intent` returns `None` for non-`upeg://` URIs so the
//! Dart `app_links` listener can silently drop unknown schemes
//! without round-tripping through the FrbError lane.
//!
//! `encode_launch_intent` is the inverse — it produces the canonical
//! `upeg://open?...` URI from a `LaunchIntentDto`, so the popup can
//! mint outgoing deep links without re-encoding query params in Dart.

#[cfg(any(not(target_arch = "wasm32"), test))]
use upeg_pegboard_ui::deep_link::{desktop_deep_link, desktop_launch_from_url_maybe};

/// Typed launch intent shared by the three intent sources (cold-boot
/// argv, second-instance `app_links` event, in-process popup encode).
///
/// `Option<String>` for board / tool / input_json — FRB v2 surfaces
/// nullable fields as `String?` on the Dart side, which the
/// `LaunchIntent` Dart class consumes directly without a sentinel.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct LaunchIntentDto {
    /// Static board key (e.g. `"dev"`). `None` ⇒ leave the
    /// currently-selected board alone.
    pub board: Option<String>,
    /// Tool id (e.g. `"num.hex_to_decimal"`). `None` ⇒ no tool
    /// activation; only board switch.
    pub tool: Option<String>,
    /// Pre-fill inputs for the tool, encoded as a JSON object string.
    /// `None` ⇒ activate with `"{}"`.
    pub input_json: Option<String>,
}

/// Parse a single `upeg://open?...` URL into a [`LaunchIntentDto`].
///
/// Returns `None` for any URL that doesn't begin with the `upeg://open?`
/// prefix so non-upeg deep-link schemes (e.g. `https://`) can be
/// dropped silently by the Dart listener.
#[cfg(not(target_arch = "wasm32"))]
#[flutter_rust_bridge::frb(sync)]
pub fn parse_launch_intent(uri: String) -> Option<LaunchIntentDto> {
    let launch = desktop_launch_from_url_maybe(&uri)?;
    Some(LaunchIntentDto {
        board: Some(launch.board.to_string()),
        tool: launch.tool.map(|t| t.to_string()),
        input_json: launch.input,
    })
}

/// Encode a [`LaunchIntentDto`] back into a canonical
/// `upeg://open?...` URI. Empty / `None` fields are dropped per
/// `desktop_deep_link` semantics so the popup never produces
/// degenerate `&board=` pairs.
#[cfg(not(target_arch = "wasm32"))]
#[flutter_rust_bridge::frb(sync)]
pub fn encode_launch_intent(intent: LaunchIntentDto) -> String {
    let board = intent.board.as_deref().unwrap_or("");
    desktop_deep_link(board, intent.tool.as_deref(), intent.input_json.as_deref())
}

#[cfg(target_arch = "wasm32")]
#[flutter_rust_bridge::frb(sync)]
pub fn parse_launch_intent(uri: String) -> Option<LaunchIntentDto> {
    let _ = uri;
    None
}

#[cfg(target_arch = "wasm32")]
#[flutter_rust_bridge::frb(sync)]
pub fn encode_launch_intent(intent: LaunchIntentDto) -> String {
    let _ = intent;
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_launch_intent는_board와_tool과_input을_포함한_uri를_만든다() {
        let intent = LaunchIntentDto {
            board: Some("dev".into()),
            tool: Some("num.hex_to_decimal".into()),
            input_json: Some(r#"{"value":"ff"}"#.into()),
        };
        let uri = encode_launch_intent(intent);
        assert!(uri.starts_with("upeg://open?"), "got {uri}");
        assert!(uri.contains("board=dev"), "got {uri}");
        assert!(uri.contains("tool=num.hex_to_decimal"), "got {uri}");
        let parsed = parse_launch_intent(uri).expect("round-trip");
        assert_eq!(parsed.tool.as_deref(), Some("num.hex_to_decimal"));
        assert_eq!(parsed.board.as_deref(), Some("dev"));
    }

    #[test]
    fn parse_launch_intent는_upeg가_아닌_uri에_none을_반환한다() {
        assert!(parse_launch_intent("https://example.test".into()).is_none());
    }

    #[test]
    fn parse_launch_intent는_쿼리_없는_open_uri를_기본_보드_intent로_파싱한다() {
        let parsed = parse_launch_intent("upeg://open".into()).expect("explicit open");
        assert_eq!(parsed.board.as_deref(), Some("dev"));
        assert_eq!(parsed.tool, None);
        assert_eq!(parsed.input_json, None);
    }

    #[test]
    fn parse_launch_intent는_open을_닮은_다른_uri를_거부한다() {
        assert!(parse_launch_intent("upeg://openly?tool=num.hex_to_decimal".into()).is_none());
    }

    #[test]
    fn encode_launch_intent는_빈_tool과_input을_생략한다() {
        let intent = LaunchIntentDto {
            board: Some("dev".into()),
            tool: None,
            input_json: None,
        };
        let uri = encode_launch_intent(intent);
        assert!(!uri.contains("tool="), "got {uri}");
        assert!(!uri.contains("input="), "got {uri}");
        assert!(uri.contains("board=dev"), "got {uri}");
    }
}
