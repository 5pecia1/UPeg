//! Source-located tests for `upeg-core` toolbox, identity, and shared types.

use crate::*;

fn meta(id: &'static str, surfaces: &'static [Surface]) -> ToolMeta {
    let id = if id.contains('.') {
        id
    } else {
        Box::leak(format!("test.{id}").into_boxed_str())
    };
    let local_id = ToolId::parse_canonical_in_toolkit(id, "test")
        .expect("test ToolMeta ids must be canonical")
        .local();
    ToolMeta {
        id,
        toolkit: "test",
        local_id,
        tags: &[],
        display_label: "Test tool",
        description: "",
        input_spec: InputSpec::empty(),
        output_spec: OutputSpec::empty(),
        primary_output_id: None,
        source: Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: PegboardUnits::U1,
        invoker: Invoker::Function,
        surfaces,
        boards: &[],
    }
}

#[test]
fn 전체_표면은_일곱_개이다() {
    // PRD v2.1 §2.3 #5 + §6.2: there are exactly 7 surfaces.
    assert_eq!(ALL_SURFACES.len(), 7);
}

#[test]
fn 전체_표면은_모든_변형을_중복_없이_포함한다() {
    let labels: std::collections::HashSet<_> = ALL_SURFACES.iter().map(|s| s.label()).collect();
    assert_eq!(
        labels.len(),
        ALL_SURFACES.len(),
        "ALL_SURFACES must be unique"
    );
    for expected in ["cli", "tui", "desktop", "pwa", "ext", "mcp", "http"] {
        assert!(labels.contains(expected), "ALL_SURFACES missing {expected}");
    }
}

#[test]
fn 페그보드_단위_파싱과_크기는_그리드_계약과_일치한다() {
    assert_eq!(PegboardUnits::U1.grid_span(), (1, 1));
    assert_eq!(PegboardUnits::U2.grid_span(), (2, 1));
    assert_eq!(PegboardUnits::U2T.grid_span(), (1, 2));
    assert_eq!(PegboardUnits::parse("U1"), Some(PegboardUnits::U1));
    assert_eq!(PegboardUnits::parse("u2"), Some(PegboardUnits::U2));
    assert_eq!(PegboardUnits::parse("U2T"), Some(PegboardUnits::U2T));
    assert_eq!(PegboardUnits::parse("U2T "), None);
    assert_eq!(PegboardUnits::U2T.label(), "U2T");
}

#[test]
fn 도구_id_새타입은_동적_점_구분_id를_허용한다() {
    let id = ToolId::parse(" custom.tool.v2 ").expect("valid dynamic id");
    assert_eq!(id.as_str(), "custom.tool.v2");
    let identity =
        ToolId::parse_in_toolkit(" custom.tool.v2 ", "custom").expect("identity in toolkit");
    assert_eq!(identity.toolkit(), "custom");
    assert_eq!(identity.local(), "tool.v2");
    assert!(
        ToolId::parse("not.in.an.enum").is_ok(),
        "runtime ids are open-ended dynamic strings and do not infer Toolkit from first dot"
    );
}

#[test]
fn 도구_id_새타입은_정규_형식이_아닌_형태를_거부한다() {
    assert!(matches!(ToolId::parse(""), Err(ToolIdError::EmptyId)));
    assert!(matches!(
        ToolId::parse("tool_without_toolkit"),
        Err(ToolIdError::MissingSeparator)
    ));
    assert!(matches!(
        ToolId::parse("kit."),
        Err(ToolIdError::EmptyLocal)
    ));
    assert!(matches!(
        ToolId::parse(".tool"),
        Err(ToolIdError::EmptyToolkit)
    ));
    assert!(matches!(
        ToolId::parse_in_toolkit("other.tool", "expected"),
        Err(ToolIdError::ToolkitMismatch { .. })
    ));
}

#[test]
fn 도구_id_정규_파서는_공백_있는_식별자를_거부한다() {
    assert!(matches!(
        ToolId::parse_canonical(" custom.tool "),
        Err(ToolIdError::NonCanonicalId { .. })
    ));
    assert!(matches!(
        ToolId::parse_canonical("custom. tool"),
        Err(ToolIdError::NonCanonicalId { .. })
    ));
    assert!(matches!(
        ToolId::parse_canonical("custom .tool"),
        Err(ToolIdError::NonCanonicalId { .. })
    ));
    assert!(matches!(
        ToolId::parse_canonical_in_toolkit("custom.tool", " custom "),
        Err(ToolIdError::NonCanonicalToolkit { .. })
    ));
    let id =
        ToolId::parse_canonical_in_toolkit("custom.tool", "custom").expect("canonical id accepted");
    assert_eq!(id.as_str(), "custom.tool");
    assert_eq!(id.toolkit(), "custom");
    assert_eq!(id.local(), "tool");
}

#[test]
fn 도구_식별자는_점_구분_도구킷과_점_구분_로컬_이름을_허용한다() {
    let id = ToolId::parse_canonical_in_toolkit("github.com.admin.tools.list", "github.com")
        .expect("dotted toolkit and dotted local name should be unambiguous with explicit toolkit");
    assert_eq!(id.as_str(), "github.com.admin.tools.list");
    assert_eq!(id.toolkit(), "github.com");
    assert_eq!(id.local(), "admin.tools.list");
    assert_eq!(id.key().toolkit(), "github.com");
    assert_eq!(id.key().local(), "admin.tools.list");
}

#[test]
fn 도구_키는_구조화되어_있고_점을_경계로_파싱하지_않는다() {
    let key = ToolKey::parse_canonical("github.com", "admin.tools.list")
        .expect("dotted structured key parts are valid");
    assert_eq!(key.toolkit(), "github.com");
    assert_eq!(key.local(), "admin.tools.list");
    assert!(matches!(
        ToolKey::parse_canonical(" github.com ", "admin.tools.list"),
        Err(ToolIdError::NonCanonicalToolkit { .. })
    ));
    assert!(matches!(
        ToolKey::parse_canonical("github. com", "admin.tools.list"),
        Err(ToolIdError::NonCanonicalToolkit { .. })
    ));
    assert!(matches!(
        ToolKey::parse_canonical("github.com", " admin.tools.list "),
        Err(ToolIdError::NonCanonicalId { .. })
    ));
    assert!(matches!(
        ToolKey::parse_canonical("github.com", "admin. tools.list"),
        Err(ToolIdError::NonCanonicalId { .. })
    ));
}

#[test]
fn embed_표면은_그래픽_사용자_인터페이스_전용이다() {
    // PRD §4.3 LIMITS — Embed cannot reach CLI/TUI/MCP/HTTP because there's
    // no UI thread for the WebView.
    let labels: Vec<_> = EMBED_SURFACES.iter().map(|s| s.label()).collect();
    assert_eq!(labels, vec!["desktop", "pwa", "ext"]);
}

#[test]
fn 표면_라벨은_간결한_형식을_따른다() {
    let m = meta("x", ALL_SURFACES);
    assert_eq!(
        m.surfaces_label(),
        "cli · tui · desktop · pwa · ext · mcp · http"
    );
}

#[test]
fn 표면_라벨은_부분집합을_처리한다() {
    let m = meta("e", EMBED_SURFACES);
    assert_eq!(m.surfaces_label(), "desktop · pwa · ext");
}

#[test]
fn 표면_라벨은_빈_목록을_처리한다() {
    let m = meta("none", &[]);
    assert_eq!(m.surfaces_label(), "");
}

#[test]
fn 표면_라벨은_각_표면을_문자열로_반환한다() {
    // vec form parallel to surfaces_label's joined string.
    // Used by HTTP /v1/tools, OpenAPI x-surfaces, CLI tool list/show
    // --json. Pin the round-trip with surfaces_label so the two
    // helpers can't drift in their definition of "label".
    let m = meta("x", &[Surface::Cli, Surface::Http]);
    assert_eq!(m.surface_labels(), vec!["cli", "http"]);
    assert_eq!(m.surfaces_label(), m.surface_labels().join(" · "));
}

#[test]
fn 표면이_없으면_표면_라벨도_비어_있다() {
    let m = meta("x", &[]);
    assert!(m.surface_labels().is_empty());
}

#[test]
fn 사용자경험_결과_상태_라벨은_공유_표면_계약이다() {
    assert_eq!(ux::result_status_label(false), "OK");
    assert_eq!(ux::result_status_label(true), "ERROR");
}

#[test]
fn 표면은_모든_라벨을_왕복_파싱한다() {
    // every Surface variant's `label()` must round-trip
    // through `parse()`. Catches a variant added with a label but
    // not wired into parse (or vice-versa) — used to be a
    // three-place silent-drift risk.
    for s in ALL_SURFACES {
        let label = s.label();
        assert_eq!(
            Surface::parse(label),
            Some(*s),
            "label `{label}` must round-trip via Surface::parse",
        );
    }
}

#[test]
fn 표면_파싱은_알수없는_값에_없음을_반환한다() {
    assert_eq!(Surface::parse("not-a-surface"), None);
    assert_eq!(Surface::parse(""), None);
    assert_eq!(Surface::parse("nope"), None);
}

#[test]
fn pin_종류는_모든_라벨을_왕복_파싱한다() {
    // Same round-trip contract for PinKind. Centralising the
    // match arms in upeg-core ensures any new variant is honoured by
    // upeg-loader and upeg-wasm without duplicated arms drifting.
    for wk in [
        PinKind::Inline,
        PinKind::Launcher,
        PinKind::Live,
        PinKind::Action,
        PinKind::Embed,
        PinKind::ControlledEmbed,
        PinKind::Chain,
        PinKind::Llm,
    ] {
        let label = wk.label();
        assert_eq!(
            PinKind::parse(label),
            Some(wk),
            "label `{label}` must round-trip via PinKind::parse",
        );
    }
}

#[test]
fn pin_종류_파싱은_알수없는_값에_없음을_반환한다() {
    assert_eq!(PinKind::parse("Mauve"), None);
    assert_eq!(PinKind::parse(""), None);
    assert_eq!(
        PinKind::parse("inline"),
        Some(PinKind::Inline),
        "parse is case-insensitive: lowercase labels accepted",
    );
}

#[test]
fn 호출자는_파스칼_케이스_형태를_왕복_파싱한다() {
    // `Invoker::parse` accepts the TOML/loader convention
    // (PascalCase) — same round-trip pattern Surface and PinKind
    // already have. Note the deliberate label/parse asymmetry:
    // `label()` emits lowercase ("function") for JSON/HTTP/MCP
    // output, but TOML authors write "Function" and the loader's
    // UnknownInvoker hint advertises that PascalCase form. This
    // test pins the parse-side convention.
    for (input, expected) in [
        ("Function", Invoker::Function),
        ("External", Invoker::External),
        ("Http", Invoker::Http),
        ("Static", Invoker::Static),
        ("Embed", Invoker::Embed),
        ("Chain", Invoker::Chain),
        ("Llm", Invoker::Llm),
        ("Wasm", Invoker::Wasm),
    ] {
        assert_eq!(
            Invoker::parse(input),
            Some(expected),
            "PascalCase `{input}` must parse to {expected:?}",
        );
    }
}

#[test]
fn 호출자_파싱은_알수없는_값에_없음을_반환한다() {
    assert_eq!(Invoker::parse("NotAnInvoker"), None);
    assert_eq!(Invoker::parse(""), None);
    assert_eq!(
        Invoker::parse("function"),
        Some(Invoker::Function),
        "parse is case-insensitive: lowercase label form accepted",
    );
}

#[test]
fn 도구_메타_동등성은_구조화된_키를_사용한다() {
    let a = meta("same", ALL_SURFACES);
    let mut b = meta("same", &[]);
    b.pin = PinKind::Live;
    assert_eq!(a, b);

    let c = meta("other", ALL_SURFACES);
    assert_ne!(a, c);
}

#[test]
fn 고정_여부_검사는_동작한다() {
    let mut m = meta("t", ALL_SURFACES);
    m.boards = &["dev", "trading"];
    assert!(m.is_on_board("dev"));
    assert!(m.is_on_board("trading"));
    assert!(!m.is_on_board("personal"));
}

#[test]
fn 표면_여부_검사는_선언된_표면과_일치한다() {
    let mcp_only = meta("x", &[Surface::Mcp]);
    assert!(mcp_only.is_on_surface(Surface::Mcp));
    assert!(!mcp_only.is_on_surface(Surface::Cli));
    assert!(!mcp_only.is_on_surface(Surface::Http));

    let everywhere = meta("y", ALL_SURFACES);
    for s in ALL_SURFACES {
        assert!(
            everywhere.is_on_surface(*s),
            "ALL_SURFACES tool must accept {}",
            s.label()
        );
    }
}

#[test]
fn pin_종류_라벨은_정규_파스칼_케이스이다() {
    // Lexicon §4 spells these in Pascal — they go on UI badges as-is.
    assert_eq!(PinKind::Inline.label(), "Inline");
    assert_eq!(PinKind::Launcher.label(), "Launcher");
    assert_eq!(PinKind::Live.label(), "Live");
    assert_eq!(PinKind::Action.label(), "Action");
    assert_eq!(PinKind::Embed.label(), "Embed");
    assert_eq!(PinKind::ControlledEmbed.label(), "ControlledEmbed");
    assert_eq!(PinKind::Chain.label(), "Chain");
    assert_eq!(PinKind::Llm.label(), "Llm");
}

#[test]
fn 호출자_라벨은_소문자이다() {
    // Footer tag `invoker={x}` reads better lowercase.
    assert_eq!(Invoker::Function.label(), "function");
    assert_eq!(Invoker::External.label(), "external");
    assert_eq!(Invoker::Http.label(), "http");
    assert_eq!(Invoker::Static.label(), "static");
    assert_eq!(Invoker::Embed.label(), "embed");
    assert_eq!(Invoker::Chain.label(), "chain");
    assert_eq!(Invoker::Llm.label(), "llm");
    assert_eq!(Invoker::Wasm.label(), "wasm");
}

#[cfg(feature = "serde")]
#[test]
fn pin_종류_직렬화_역직렬화_왕복을_검증한다() {
    let json = serde_json::to_string(&PinKind::Live).unwrap();
    assert_eq!(json, "\"Live\"");
    let back: PinKind = serde_json::from_str(&json).unwrap();
    assert_eq!(back, PinKind::Live);
}

#[test]
fn 입출력_타입_목록_상수는_닫힌_입출력_타입_배열과_일치한다() {
    let derived = CLOSED_IO_TYPES.join(", ");
    assert_eq!(
        IO_TYPE_LIST, derived,
        "IO_TYPE_LIST must stay in sync with CLOSED_IO_TYPES (single PRD v2.1 vocabulary)",
    );
}

#[cfg(feature = "serde")]
#[test]
fn placement_신규_필드가_none이면_기존_json_형태와_바이트_동일하다() {
    let legacy_json = r#"{"tool_id":"clock.now","x":1,"y":2}"#;
    let placement = Placement::new("clock.now", 1, 2);

    assert_eq!(serde_json::to_string(&placement).unwrap(), legacy_json);
    let back: Placement = serde_json::from_str(legacy_json).unwrap();
    assert_eq!(back, placement);

    let legacy_colored_json = r##"{"tool_id":"clock.now","x":1,"y":2,"color":"#A1B2C3"}"##;
    let colored =
        Placement::new("clock.now", 1, 2).with_color(Some(PinColorHex::parse("#A1B2C3").unwrap()));
    assert_eq!(
        serde_json::to_string(&colored).unwrap(),
        legacy_colored_json
    );
    let back: Placement = serde_json::from_str(legacy_colored_json).unwrap();
    assert_eq!(back, colored);
}

#[cfg(feature = "serde")]
#[test]
fn placement_span과_args_preset은_직렬화_왕복을_보존한다() {
    let span = PinSpan::new(ColSpan::new(2).unwrap(), RowSpan::new(3).unwrap());
    let preset = ArgsPreset::parse(r#"{"city":"Seoul"}"#).unwrap();
    let placement = Placement::new("weather.now", 0, 1)
        .with_span(Some(span))
        .with_args_preset(Some(preset));

    let json = serde_json::to_string(&placement).unwrap();
    assert!(json.contains(r#""span":{"cols":2,"rows":3}"#));
    assert!(json.contains(r#""args_preset":{"city":"Seoul"}"#));

    let back: Placement = serde_json::from_str(&json).unwrap();
    assert_eq!(back, placement);
}

#[cfg(feature = "serde")]
#[test]
fn placement_역직렬화는_손상된_span과_예약_키_preset을_거부한다() {
    let corrupted_span = r#"{"tool_id":"a.b","x":0,"y":0,"span":{"cols":0,"rows":1}}"#;
    assert!(serde_json::from_str::<Placement>(corrupted_span).is_err());

    let reserved_preset =
        format!(r#"{{"tool_id":"a.b","x":0,"y":0,"args_preset":{{"{EXECUTION_CONTEXT_ARG}":1}}}}"#);
    assert!(serde_json::from_str::<Placement>(&reserved_preset).is_err());
}
