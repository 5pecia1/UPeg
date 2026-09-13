//! `eth` toolkit — minimal Ethereum JSON-RPC reads (gas price, address
//! balance), promoted from `gui_meta.rs`'s non-functional `Invoker::Http`
//! placeholders (no dispatcher; running either failed outright) to real
//! `Invoker::Function` tools with a native HTTPS dispatcher.
//!
//! Native-only, same physical constraint as `net.rs`'s TCP probe: making
//! a real HTTP request needs a socket, which the `wasm32` (Flutter web /
//! PWA) sandbox doesn't have. Unlike `net.rs` (a pure helper module with
//! its meta living separately in `gui_meta.rs`), these are real
//! `#[tool]`-annotated functions, so the `pub fn` (and the `StaticToolMeta`
//! the macro emits alongside it) must still compile on every target —
//! only the actual network-calling implementation is native-gated, mirrored
//! on the `wasm32` stub in `toolkits::media` (`image_to_pdf`/`image_to_pdf_impl`).
//! The runtime *dispatcher* registration (`dispatch.rs::register_eth_dispatchers`)
//! is `#[cfg(not(target_arch = "wasm32"))]`-gated on top of that, exactly like
//! `net.status` — so on `wasm32` the tool is listed (discoverable, e.g. for a
//! paired native host to run via host-attach) but has no runtime dispatcher at all,
//! the `NativeOnlyTool` shape `upeg_core::capability` already models.

use upeg_core::tool;

/// Default public, keyless Ethereum JSON-RPC endpoint. Named so a caller
/// who needs a different provider (Infura/Alchemy/self-hosted node) can
/// override it per call via the optional `endpoint` input instead of the
/// tool being hardwired to one provider.
pub const DEFAULT_ETH_RPC_ENDPOINT: &str = "https://ethereum-rpc.publicnode.com";

/// Wei per gwei (10^9) — `eth.gas`'s gwei output is `wei / WEI_PER_GWEI`.
const WEI_PER_GWEI: u128 = 1_000_000_000;
/// Wei per ether (10^18) — `eth.address_lookup`'s ETH output is `wei / WEI_PER_ETHER`.
const WEI_PER_ETHER: u128 = 1_000_000_000_000_000_000;

/// Resolve the effective JSON-RPC endpoint: `endpoint` when non-blank,
/// [`DEFAULT_ETH_RPC_ENDPOINT`] otherwise. Shared by both tools so
/// "override optional, sane default" stays in one place.
fn resolve_endpoint(endpoint: &str) -> &str {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        DEFAULT_ETH_RPC_ENDPOINT
    } else {
        trimmed
    }
}

/// Parse a `0x`-prefixed hex quantity, as returned by every Ethereum
/// JSON-RPC numeric field, into a decimal `u128` of wei. Only called from
/// the native `eth_gas_price_wei`/`eth_balance_wei` impls — the `wasm32`
/// stubs never produce a wei value to parse.
#[cfg(not(target_arch = "wasm32"))]
fn parse_hex_wei(hex: &str) -> Result<u128, String> {
    let trimmed = hex.trim();
    let body = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    if body.is_empty() {
        return Err("empty hex quantity in RPC response".to_string());
    }
    u128::from_str_radix(body, 16)
        .map_err(|_| format!("invalid hex quantity `{hex}` in RPC response"))
}

/// Format a wei amount as a gwei decimal string (`eth.gas`'s output).
fn format_gwei(wei: u128) -> String {
    #[allow(
        clippy::cast_precision_loss,
        reason = "gwei is a display value; f64 precision is ample for realistic gas prices"
    )]
    let gwei = wei as f64 / WEI_PER_GWEI as f64;
    gwei.to_string()
}

/// Format a wei amount as an `<amount> ETH` string (`eth.address_lookup`'s output).
fn format_eth_balance(wei: u128) -> String {
    #[allow(
        clippy::cast_precision_loss,
        reason = "ETH balance is a display value; f64 precision is ample for realistic balances"
    )]
    let eth = wei as f64 / WEI_PER_ETHER as f64;
    format!("{eth} ETH")
}

/// `eth.gas` — current Ethereum gas price, in gwei, via JSON-RPC `eth_gasPrice`.
#[tool(
    id = "eth.gas",
    display_label = "ETH gas price",
    toolkit = "eth",
    description = "Current Ethereum gas price (gwei) via JSON-RPC `eth_gasPrice`.",
    inputs = [
        optional endpoint: String = "JSON-RPC endpoint override (default: a public keyless RPC)",
    ],
    outputs = [
        result: Number = "Current gas price in gwei",
    ],
    pin = Launcher,
    pegboard_units = U1,
    invoker = Function,
    boards = ["trading"],
)]
pub fn eth_gas(endpoint: &str) -> Result<String, String> {
    let wei = eth_gas_price_wei(resolve_endpoint(endpoint))?;
    Ok(format_gwei(wei))
}

/// `eth.address_lookup` — an Ethereum address's balance, in ETH, via
/// JSON-RPC `eth_getBalance`.
#[tool(
    id = "eth.address_lookup",
    display_label = "ETH address lookup",
    toolkit = "eth",
    description = "Look up an Ethereum address's balance (ETH) via JSON-RPC `eth_getBalance`.",
    inputs = [
        required address: String = "Ethereum address, e.g. 0x...",
        optional endpoint: String = "JSON-RPC endpoint override (default: a public keyless RPC)",
    ],
    pin = Launcher,
    pegboard_units = U1,
    invoker = Function,
    boards = ["trading"],
)]
pub fn eth_address_lookup(address: &str, endpoint: &str) -> Result<String, String> {
    let address = address.trim();
    if address.is_empty() {
        return Err("address must not be empty".to_string());
    }
    let wei = eth_balance_wei(resolve_endpoint(endpoint), address)?;
    Ok(format_eth_balance(wei))
}

#[cfg(target_arch = "wasm32")]
fn eth_gas_price_wei(_endpoint: &str) -> Result<u128, String> {
    Err("eth.gas requires a native network runtime".to_string())
}

#[cfg(target_arch = "wasm32")]
fn eth_balance_wei(_endpoint: &str, _address: &str) -> Result<u128, String> {
    Err("eth.address_lookup requires a native network runtime".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn eth_gas_price_wei(endpoint: &str) -> Result<u128, String> {
    let hex = json_rpc_call(endpoint, "eth_gasPrice", serde_json::json!([]))?;
    parse_hex_wei(&hex)
}

#[cfg(not(target_arch = "wasm32"))]
fn eth_balance_wei(endpoint: &str, address: &str) -> Result<u128, String> {
    let hex = json_rpc_call(
        endpoint,
        "eth_getBalance",
        serde_json::json!([address, "latest"]),
    )?;
    parse_hex_wei(&hex)
}

/// Connect/read/write deadline for the JSON-RPC HTTPS call. Short enough
/// that an unreachable/broken endpoint fails fast rather than hanging a
/// caller (or a test run) for a long time.
///
/// Deliberately tighter than `weather.rs`'s `WEATHER_HTTP_TIMEOUT`: this is a
/// single call to an endpoint the caller can override with their own node,
/// where `weather.*` chains two requests against a free best-effort public
/// service. The two deadlines stay separate consts for that reason rather
/// than collapsing into one shared number in `toolkits::http`.
#[cfg(not(target_arch = "wasm32"))]
const ETH_RPC_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Lazily-built HTTPS agent for JSON-RPC, carrying [`ETH_RPC_TIMEOUT`].
#[cfg(not(target_arch = "wasm32"))]
fn eth_http_agent() -> &'static ureq::Agent {
    static AGENT: std::sync::OnceLock<ureq::Agent> = std::sync::OnceLock::new();
    AGENT.get_or_init(|| super::http::build_agent(ETH_RPC_TIMEOUT))
}

/// POST a JSON-RPC 2.0 request `{method, params}` to `endpoint` and return
/// the response's `result` field as a raw string (every method this
/// toolkit calls — `eth_gasPrice`, `eth_getBalance` — returns a
/// `0x`-prefixed hex-quantity string).
#[cfg(not(target_arch = "wasm32"))]
fn json_rpc_call(
    endpoint: &str,
    method: &str,
    params: serde_json::Value,
) -> Result<String, String> {
    const JSON_RPC_VERSION: &str = "2.0";
    const JSON_RPC_REQUEST_ID: u32 = 1;

    let body = serde_json::json!({
        "jsonrpc": JSON_RPC_VERSION,
        "id": JSON_RPC_REQUEST_ID,
        "method": method,
        "params": params,
    })
    .to_string();

    let text = super::http::post_json(eth_http_agent(), endpoint, &body)
        .map_err(|error| format!("eth RPC {error} (endpoint: {endpoint})"))?;

    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|_| "eth RPC response is not valid JSON".to_string())?;
    if let Some(error) = value.get("error") {
        return Err(format!("eth RPC error: {error}"));
    }
    value
        .get("result")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "eth RPC response is missing a string `result` field".to_string())
}

// Native-only: the tests exercise `parse_hex_wei`/`resolve_endpoint` and other
// `#[cfg(not(target_arch = "wasm32"))]`-gated items, which don't exist on
// wasm32. Gating the module keeps `--all-targets` clippy clean for wasm builds.
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    // ─── resolve_endpoint ───────────────────────────────────────

    #[test]
    fn 엔드포인트가_비어있으면_기본값을_사용한다() {
        assert_eq!(resolve_endpoint(""), DEFAULT_ETH_RPC_ENDPOINT);
        assert_eq!(resolve_endpoint("   "), DEFAULT_ETH_RPC_ENDPOINT);
    }

    #[test]
    fn 엔드포인트가_주어지면_그대로_사용한다() {
        assert_eq!(
            resolve_endpoint("https://example.test/rpc"),
            "https://example.test/rpc"
        );
    }

    #[test]
    fn 엔드포인트_주변_공백을_잘라낸다() {
        assert_eq!(
            resolve_endpoint("  https://example.test/rpc  "),
            "https://example.test/rpc"
        );
    }

    // ─── parse_hex_wei ──────────────────────────────────────────

    #[test]
    fn hex_수량을_십진_wei로_파싱한다() {
        assert_eq!(parse_hex_wei("0xff"), Ok(255));
        assert_eq!(parse_hex_wei("0x0"), Ok(0));
    }

    #[test]
    fn hex_수량_파싱은_대문자_접두사를_허용한다() {
        assert_eq!(parse_hex_wei("0XFF"), Ok(255));
    }

    #[test]
    fn hex_수량_파싱은_빈_입력을_거부한다() {
        assert!(parse_hex_wei("").is_err());
        assert!(parse_hex_wei("0x").is_err());
    }

    #[test]
    fn hex_수량_파싱은_유효하지_않은_문자를_거부한다() {
        assert!(parse_hex_wei("0xzz").is_err());
    }

    // ─── format_gwei / format_eth_balance ───────────────────────

    #[test]
    fn wei를_gwei_문자열로_포맷한다() {
        assert_eq!(format_gwei(WEI_PER_GWEI), "1");
        assert_eq!(format_gwei(0), "0");
        assert_eq!(format_gwei(WEI_PER_GWEI / 2), "0.5");
    }

    #[test]
    fn wei를_eth_잔액_문자열로_포맷한다() {
        assert_eq!(format_eth_balance(WEI_PER_ETHER), "1 ETH");
        assert_eq!(format_eth_balance(0), "0 ETH");
        assert_eq!(format_eth_balance(WEI_PER_ETHER / 2), "0.5 ETH");
    }

    // ─── eth_address_lookup input validation (no network) ───────

    #[test]
    fn 주소가_비어있으면_네트워크_호출_전에_거부한다() {
        assert_eq!(
            eth_address_lookup("", ""),
            Err("address must not be empty".to_string())
        );
        assert_eq!(
            eth_address_lookup("   ", ""),
            Err("address must not be empty".to_string())
        );
    }
}
