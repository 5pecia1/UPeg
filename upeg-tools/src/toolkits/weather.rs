//! `weather` toolkit — current weather and multi-day forecast lookups by
//! city name via the keyless Open-Meteo API (geocoding + forecast).
//!
//! Native-only, for the same physical reason as `eth.rs`/`net.rs`: making a
//! real HTTP request needs a socket, which the `wasm32` (Flutter web / PWA)
//! sandbox doesn't have. Like `eth`, these are real `#[tool]`-annotated
//! functions, so the `pub fn` (and the `StaticToolMeta` the macro emits
//! alongside it) must still compile on every target — only the actual
//! network-calling implementation is native-gated, with a `wasm32` stub that
//! returns an error. The runtime *dispatcher* registration
//! (`dispatch.rs::register_weather_dispatchers`) is
//! `#[cfg(not(target_arch = "wasm32"))]`-gated on top of that, exactly like
//! `eth`/`net.status` — so on `wasm32` the tool is listed (discoverable, e.g.
//! for a paired native host to run via host-attach) but has no runtime
//! dispatcher at all.

use upeg_core::tool;

/// Default number of forecast days when `weather.forecast`'s optional `days`
/// input is omitted. Public so the dispatcher can supply the same default.
pub const WEATHER_DEFAULT_FORECAST_DAYS: usize = 7;
/// Lower bound for `weather.forecast`'s `days` (at least one day of forecast).
const MIN_FORECAST_DAYS: usize = 1;
/// Upper bound for `weather.forecast`'s `days` — Open-Meteo caps the daily
/// forecast horizon at 16 days.
const MAX_FORECAST_DAYS: usize = 16;

/// Clamp a requested forecast horizon into the API-supported
/// `[MIN_FORECAST_DAYS, MAX_FORECAST_DAYS]` range. Kept target-agnostic so the
/// `pub fn` normalizes `days` identically on every build.
fn clamp_forecast_days(days: usize) -> usize {
    days.clamp(MIN_FORECAST_DAYS, MAX_FORECAST_DAYS)
}

// ─── native-only network implementation ─────────────────────────────

/// Open-Meteo geocoding endpoint (city name → latitude/longitude).
#[cfg(not(target_arch = "wasm32"))]
const GEOCODING_ENDPOINT: &str = "https://geocoding-api.open-meteo.com/v1/search";
/// Open-Meteo forecast endpoint (current weather and daily forecast).
#[cfg(not(target_arch = "wasm32"))]
const FORECAST_ENDPOINT: &str = "https://api.open-meteo.com/v1/forecast";
/// Number of geocoding matches to request — only the top hit is used.
#[cfg(not(target_arch = "wasm32"))]
const GEOCODING_RESULT_COUNT: usize = 1;
/// Geocoding response language for place/country names.
#[cfg(not(target_arch = "wasm32"))]
const GEOCODING_LANGUAGE: &str = "en";
/// Requested response encoding for the geocoding endpoint.
#[cfg(not(target_arch = "wasm32"))]
const RESPONSE_FORMAT: &str = "json";
/// Separator Open-Meteo expects between field names in `current=`/`daily=`.
#[cfg(not(target_arch = "wasm32"))]
const FIELD_SEPARATOR: &str = ",";
/// Value for the forecast endpoint's `timezone=` parameter.
///
/// Load-bearing, not cosmetic: Open-Meteo defaults to `timezone=GMT` and
/// buckets *daily* variables by day **in the requested timezone**. Without
/// this, a Seoul (UTC+9) forecast's `temperature_2m_max` for a given date is
/// the maximum over 09:00→09:00 KST — and the first `date` returned can be
/// the local yesterday — with no error to signal it. `auto` resolves the
/// timezone from the requested coordinates, which is what a caller asking for
/// "the forecast in Seoul" means. Also applied to the current-weather lookup
/// so its `time` is reported in the city's own clock.
#[cfg(not(target_arch = "wasm32"))]
const FORECAST_TIMEZONE: &str = "auto";

/// One field of the forecast endpoint's `current` object.
///
/// Single source of truth for the current-weather lookup: [`Self::wire_name`]
/// names each field for both the request URL (built from [`Self::REQUESTED`])
/// and [`parse_current_weather`], so the two can't name it differently — and
/// dropping a variant fails to compile in the parser rather than erroring at
/// runtime against a response that no longer carries the field.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CurrentField {
    Time,
    Temperature,
    Humidity,
    WeatherCode,
    WindSpeed,
}

#[cfg(not(target_arch = "wasm32"))]
impl CurrentField {
    /// The fields asked for via `current=`, in request order. [`Self::Time`]
    /// is absent on purpose: Open-Meteo always returns `current.time`
    /// alongside whatever is requested, and naming it here would be rejected.
    const REQUESTED: [Self; 4] = [
        Self::Temperature,
        Self::Humidity,
        Self::WeatherCode,
        Self::WindSpeed,
    ];

    /// Open-Meteo's wire name for this field.
    fn wire_name(self) -> &'static str {
        match self {
            Self::Time => "time",
            Self::Temperature => "temperature_2m",
            Self::Humidity => "relative_humidity_2m",
            Self::WeatherCode => "weather_code",
            Self::WindSpeed => "wind_speed_10m",
        }
    }

    /// The `current=` parameter value: [`Self::REQUESTED`], comma-joined.
    fn request_list() -> String {
        Self::REQUESTED.map(Self::wire_name).join(FIELD_SEPARATOR)
    }
}

/// One field of the forecast endpoint's `daily` object, playing the same
/// single-source-of-truth role as [`CurrentField`] for the multi-day forecast.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DailyField {
    Time,
    WeatherCode,
    TemperatureMax,
    TemperatureMin,
}

#[cfg(not(target_arch = "wasm32"))]
impl DailyField {
    /// The fields asked for via `daily=`, in request order. [`Self::Time`] is
    /// absent for the same reason as [`CurrentField::Time`]: `daily.time` is
    /// always returned, never requested.
    const REQUESTED: [Self; 3] = [
        Self::WeatherCode,
        Self::TemperatureMax,
        Self::TemperatureMin,
    ];

    /// Open-Meteo's wire name for this field.
    fn wire_name(self) -> &'static str {
        match self {
            Self::Time => "time",
            Self::WeatherCode => "weather_code",
            Self::TemperatureMax => "temperature_2m_max",
            Self::TemperatureMin => "temperature_2m_min",
        }
    }

    /// The `daily=` parameter value: [`Self::REQUESTED`], comma-joined.
    fn request_list() -> String {
        Self::REQUESTED.map(Self::wire_name).join(FIELD_SEPARATOR)
    }
}

/// Human-readable description shown when a WMO weather code is unrecognized.
#[cfg(not(target_arch = "wasm32"))]
const WEATHER_CODE_UNKNOWN: &str = "Unknown";

/// WMO weather-interpretation code → human-readable description table.
/// Sourced from the Open-Meteo documentation; used to annotate both the
/// current-weather and forecast outputs.
#[cfg(not(target_arch = "wasm32"))]
const WEATHER_CODE_DESCRIPTIONS: &[(u64, &str)] = &[
    (0, "Clear sky"),
    (1, "Mainly clear"),
    (2, "Partly cloudy"),
    (3, "Overcast"),
    (45, "Fog"),
    (48, "Depositing rime fog"),
    (51, "Light drizzle"),
    (53, "Moderate drizzle"),
    (55, "Dense drizzle"),
    (56, "Light freezing drizzle"),
    (57, "Dense freezing drizzle"),
    (61, "Slight rain"),
    (63, "Moderate rain"),
    (65, "Heavy rain"),
    (66, "Light freezing rain"),
    (67, "Heavy freezing rain"),
    (71, "Slight snow fall"),
    (73, "Moderate snow fall"),
    (75, "Heavy snow fall"),
    (77, "Snow grains"),
    (80, "Slight rain showers"),
    (81, "Moderate rain showers"),
    (82, "Violent rain showers"),
    (85, "Slight snow showers"),
    (86, "Heavy snow showers"),
    (95, "Thunderstorm"),
    (96, "Thunderstorm with slight hail"),
    (99, "Thunderstorm with heavy hail"),
];

/// Map a WMO weather code to its description, defaulting to
/// [`WEATHER_CODE_UNKNOWN`] for codes outside the table.
#[cfg(not(target_arch = "wasm32"))]
fn weather_code_description(code: u64) -> &'static str {
    WEATHER_CODE_DESCRIPTIONS
        .iter()
        .find(|(candidate, _)| *candidate == code)
        .map_or(WEATHER_CODE_UNKNOWN, |(_, description)| *description)
}

/// A resolved geographic location from the geocoding endpoint.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, PartialEq)]
struct GeoLocation {
    name: String,
    country: String,
    latitude: f64,
    longitude: f64,
}

/// Current-weather snapshot parsed from the forecast endpoint's `current`.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, PartialEq)]
struct CurrentWeather {
    /// Observation timestamp in the city's own timezone, thanks to
    /// [`FORECAST_TIMEZONE`] — without it a caller can't tell how stale
    /// "current" is.
    observed_at: String,
    temperature: f64,
    humidity: f64,
    weather_code: u64,
    wind_speed: f64,
}

/// One day of forecast parsed from the forecast endpoint's `daily` arrays.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, PartialEq)]
struct DailyForecast {
    date: String,
    weather_code: u64,
    temperature_max: f64,
    temperature_min: f64,
}

/// Build the geocoding request URL for `city` (URL-encoded).
#[cfg(not(target_arch = "wasm32"))]
fn build_geocoding_url(city: &str) -> String {
    format!(
        "{GEOCODING_ENDPOINT}?name={}&count={GEOCODING_RESULT_COUNT}&language={GEOCODING_LANGUAGE}&format={RESPONSE_FORMAT}",
        urlencoding::encode(city.trim())
    )
}

/// Build the current-weather request URL for the given coordinates.
#[cfg(not(target_arch = "wasm32"))]
fn build_current_weather_url(latitude: f64, longitude: f64) -> String {
    format!(
        "{FORECAST_ENDPOINT}?latitude={latitude}&longitude={longitude}&current={}&timezone={FORECAST_TIMEZONE}",
        CurrentField::request_list()
    )
}

/// Build the daily-forecast request URL for the given coordinates and horizon.
#[cfg(not(target_arch = "wasm32"))]
fn build_forecast_url(latitude: f64, longitude: f64, days: usize) -> String {
    format!(
        "{FORECAST_ENDPOINT}?latitude={latitude}&longitude={longitude}&daily={}&forecast_days={days}&timezone={FORECAST_TIMEZONE}",
        DailyField::request_list()
    )
}

/// Read a required numeric field from a JSON object, erroring with the
/// field name if it is absent or not a number.
#[cfg(not(target_arch = "wasm32"))]
fn required_f64(object: &serde_json::Value, field: &str) -> Result<f64, String> {
    object
        .get(field)
        .and_then(serde_json::Value::as_f64)
        .ok_or_else(|| format!("weather response is missing numeric field `{field}`"))
}

/// Read a required unsigned-integer field from a JSON object.
#[cfg(not(target_arch = "wasm32"))]
fn required_u64(object: &serde_json::Value, field: &str) -> Result<u64, String> {
    object
        .get(field)
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| format!("weather response is missing integer field `{field}`"))
}

/// Read a required string field from a JSON object. Used for fields whose
/// absence would otherwise surface as an empty string in the output — a
/// `"city": ""` result is a silent lie, an error is not.
#[cfg(not(target_arch = "wasm32"))]
fn required_str(object: &serde_json::Value, field: &str) -> Result<String, String> {
    object
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("weather response is missing string field `{field}`"))
}

/// Parse the geocoding response, returning the top match or an error when no
/// location was found.
#[cfg(not(target_arch = "wasm32"))]
fn parse_geocoding(body: &str) -> Result<GeoLocation, String> {
    let value: serde_json::Value = serde_json::from_str(body)
        .map_err(|_| "geocoding response is not valid JSON".to_string())?;
    let first = value
        .get("results")
        .and_then(serde_json::Value::as_array)
        .and_then(|results| results.first())
        .ok_or_else(|| "no matching location found for the given city".to_string())?;
    Ok(GeoLocation {
        name: required_str(first, "name")?,
        country: required_str(first, "country")?,
        latitude: required_f64(first, "latitude")?,
        longitude: required_f64(first, "longitude")?,
    })
}

/// Parse the `current` object of a forecast response.
#[cfg(not(target_arch = "wasm32"))]
fn parse_current_weather(body: &str) -> Result<CurrentWeather, String> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|_| "weather response is not valid JSON".to_string())?;
    let current = value
        .get("current")
        .ok_or_else(|| "weather response is missing `current` data".to_string())?;
    Ok(CurrentWeather {
        observed_at: required_str(current, CurrentField::Time.wire_name())?,
        temperature: required_f64(current, CurrentField::Temperature.wire_name())?,
        humidity: required_f64(current, CurrentField::Humidity.wire_name())?,
        weather_code: required_u64(current, CurrentField::WeatherCode.wire_name())?,
        wind_speed: required_f64(current, CurrentField::WindSpeed.wire_name())?,
    })
}

/// Parse the parallel `daily` arrays of a forecast response into one entry per
/// day, aligned by index.
///
/// Days whose entries aren't all present are skipped rather than failing the
/// whole response: Open-Meteo emits `null` for an individual daily value it
/// doesn't have yet at the edge of the forecast horizon, so a hard error there
/// would trade 15 good days for zero over one missing maximum. Skipping is the
/// shape [`DailyForecast`]'s output contract already promises — "date, weather
/// code/description, min/max temperature" per entry — so every emitted entry
/// stays complete by construction, rather than every consumer having to handle
/// a null in a field the contract says is there.
#[cfg(not(target_arch = "wasm32"))]
fn parse_daily_forecast(body: &str) -> Result<Vec<DailyForecast>, String> {
    let value: serde_json::Value = serde_json::from_str(body)
        .map_err(|_| "forecast response is not valid JSON".to_string())?;
    let daily = value
        .get("daily")
        .ok_or_else(|| "forecast response is missing `daily` data".to_string())?;

    let array = |field: DailyField| -> Result<&Vec<serde_json::Value>, String> {
        let field = field.wire_name();
        daily
            .get(field)
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| format!("forecast response is missing array field `{field}`"))
    };

    let times = array(DailyField::Time)?;
    let codes = array(DailyField::WeatherCode)?;
    let maxima = array(DailyField::TemperatureMax)?;
    let minima = array(DailyField::TemperatureMin)?;

    let mut entries = Vec::with_capacity(times.len());
    for (index, time) in times.iter().enumerate() {
        // A missing entry and an explicit `null` are the same thing here: the
        // day is incomplete either way, and `as_u64`/`as_f64` already map both
        // to `None`.
        let (Some(date), Some(weather_code), Some(temperature_max), Some(temperature_min)) = (
            time.as_str(),
            codes.get(index).and_then(serde_json::Value::as_u64),
            maxima.get(index).and_then(serde_json::Value::as_f64),
            minima.get(index).and_then(serde_json::Value::as_f64),
        ) else {
            continue;
        };
        entries.push(DailyForecast {
            date: date.to_string(),
            weather_code,
            temperature_max,
            temperature_min,
        });
    }
    Ok(entries)
}

/// Connect/read/write deadline for Open-Meteo HTTPS calls. Short enough that
/// an unreachable endpoint fails fast rather than hanging the caller.
///
/// Deliberately more generous than `eth.rs`'s `ETH_RPC_TIMEOUT`: both
/// `weather.*` tools chain two requests (geocode, then forecast) against a
/// free, unauthenticated, best-effort public service, where `eth` makes a
/// single call to an endpoint the caller can point at their own node. The two
/// deadlines stay separate consts for that reason rather than collapsing into
/// one shared number in `toolkits::http`.
#[cfg(not(target_arch = "wasm32"))]
const WEATHER_HTTP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// Lazily-built HTTPS agent for Open-Meteo, carrying [`WEATHER_HTTP_TIMEOUT`].
#[cfg(not(target_arch = "wasm32"))]
fn weather_http_agent() -> &'static ureq::Agent {
    static AGENT: std::sync::OnceLock<ureq::Agent> = std::sync::OnceLock::new();
    AGENT.get_or_init(|| super::http::build_agent(WEATHER_HTTP_TIMEOUT))
}

/// GET `url` and return the response body, erroring on transport failure or a
/// non-2xx status. Open-Meteo answers a rejected request with HTTP 400 and a
/// `{"error":true,"reason":"..."}` body, which `toolkits::http` keeps in the
/// error so the caller sees the reason rather than a bare status.
#[cfg(not(target_arch = "wasm32"))]
fn weather_http_get(url: &str) -> Result<String, String> {
    super::http::get(weather_http_agent(), url).map_err(|error| format!("weather {error}"))
}

/// Serialize a JSON value to a compact string, mapping failures to `String`.
#[cfg(not(target_arch = "wasm32"))]
fn serialize(value: &serde_json::Value) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| format!("failed to serialize result: {error}"))
}

#[cfg(not(target_arch = "wasm32"))]
fn weather_lookup_impl(city: &str) -> Result<String, String> {
    let location = parse_geocoding(&weather_http_get(&build_geocoding_url(city))?)?;
    let current = parse_current_weather(&weather_http_get(&build_current_weather_url(
        location.latitude,
        location.longitude,
    ))?)?;
    serialize(&serde_json::json!({
        "city": location.name,
        "country": location.country,
        "latitude": location.latitude,
        "longitude": location.longitude,
        "observed_at": current.observed_at,
        "temperature": current.temperature,
        "humidity": current.humidity,
        "weather_code": current.weather_code,
        "weather_description": weather_code_description(current.weather_code),
        "wind_speed": current.wind_speed,
    }))
}

#[cfg(not(target_arch = "wasm32"))]
fn weather_forecast_impl(city: &str, days: usize) -> Result<String, String> {
    let location = parse_geocoding(&weather_http_get(&build_geocoding_url(city))?)?;
    let entries = parse_daily_forecast(&weather_http_get(&build_forecast_url(
        location.latitude,
        location.longitude,
        days,
    ))?)?;
    let daily: Vec<serde_json::Value> = entries
        .iter()
        .map(|entry| {
            serde_json::json!({
                "date": entry.date,
                "weather_code": entry.weather_code,
                "weather_description": weather_code_description(entry.weather_code),
                "temperature_max": entry.temperature_max,
                "temperature_min": entry.temperature_min,
            })
        })
        .collect();
    serialize(&serde_json::json!({
        "city": location.name,
        "country": location.country,
        "latitude": location.latitude,
        "longitude": location.longitude,
        "daily": daily,
    }))
}

// ─── wasm32 stubs ───────────────────────────────────────────────────

#[cfg(target_arch = "wasm32")]
fn weather_lookup_impl(_city: &str) -> Result<String, String> {
    Err("weather.lookup requires a native network runtime".to_string())
}

#[cfg(target_arch = "wasm32")]
fn weather_forecast_impl(_city: &str, _days: usize) -> Result<String, String> {
    Err("weather.forecast requires a native network runtime".to_string())
}

// ─── tool definitions ───────────────────────────────────────────────

/// `weather.lookup` — current weather for a city via Open-Meteo.
#[tool(
    id = "weather.lookup",
    display_label = "Weather lookup",
    toolkit = "weather",
    description = "Look up current weather for a city (Open-Meteo, no API key).",
    inputs = [
        required city: String = "City name to look up, e.g. Seoul",
    ],
    outputs = [
        result: Json = "Current weather: observation time (local), temperature, humidity, weather code/description, wind speed",
    ],
    pin = Launcher,
    pegboard_units = U1,
    invoker = Function,
    surfaces = [Cli, Tui, Desktop, Mcp, Http],
)]
pub fn weather_lookup(city: &str) -> Result<String, String> {
    let city = city.trim();
    if city.is_empty() {
        return Err("city must not be empty".to_string());
    }
    weather_lookup_impl(city)
}

/// `weather.forecast` — multi-day daily forecast for a city via Open-Meteo.
#[tool(
    id = "weather.forecast",
    display_label = "Weather forecast",
    toolkit = "weather",
    description = "Multi-day daily forecast for a city (Open-Meteo, no API key).",
    inputs = [
        required city: String = "City name to look up, e.g. Seoul",
        optional days: Integer = "Number of forecast days (default 7, valid 1..=16)",
    ],
    outputs = [
        result: Json = "Daily forecast entries: date, weather code/description, min/max temperature",
    ],
    pin = Launcher,
    pegboard_units = U1,
    invoker = Function,
    surfaces = [Cli, Tui, Desktop, Mcp, Http],
)]
pub fn weather_forecast(city: &str, days: usize) -> Result<String, String> {
    let city = city.trim();
    if city.is_empty() {
        return Err("city must not be empty".to_string());
    }
    weather_forecast_impl(city, clamp_forecast_days(days))
}

// Native-only: the tests reference endpoint consts, field lists, and the
// geocoding/weather response structs, all `#[cfg(not(target_arch = "wasm32"))]`-
// gated. Gating the module keeps `--all-targets` clippy clean for wasm builds.
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    // ─── clamp_forecast_days ────────────────────────────────────

    #[test]
    fn forecast_days_clamp_to_valid_range() {
        assert_eq!(clamp_forecast_days(0), MIN_FORECAST_DAYS);
        assert_eq!(clamp_forecast_days(7), 7);
        assert_eq!(clamp_forecast_days(1000), MAX_FORECAST_DAYS);
        assert_eq!(clamp_forecast_days(MAX_FORECAST_DAYS), MAX_FORECAST_DAYS);
    }

    // ─── build_*_url ────────────────────────────────────────────

    // Endpoints and field lists are pinned as literals, never as
    // `format!("...{THE_CONST}")` — asserting a URL contains the very const it
    // was built from passes for any value, including one that breaks every
    // live call. The literal is the independent statement of what Open-Meteo
    // is documented to accept.

    #[test]
    fn geocoding_url_encodes_city_name() {
        let url = build_geocoding_url("São Paulo");
        assert!(url.starts_with("https://geocoding-api.open-meteo.com/v1/search?"));
        assert!(url.contains("name=S%C3%A3o%20Paulo"));
        assert!(url.contains("count=1"));
        assert!(url.contains("language=en"));
        assert!(url.contains("format=json"));
    }

    #[test]
    fn geocoding_url_trims_whitespace_around_city_name() {
        let url = build_geocoding_url("  Seoul  ");
        assert!(url.contains("name=Seoul"));
    }

    #[test]
    fn current_weather_url_carries_coordinates_and_fields() {
        let url = build_current_weather_url(37.5665, 126.978);
        assert!(url.starts_with("https://api.open-meteo.com/v1/forecast?"));
        assert!(url.contains("latitude=37.5665"));
        assert!(url.contains("longitude=126.978"));
        assert!(
            url.contains("current=temperature_2m,relative_humidity_2m,weather_code,wind_speed_10m")
        );
    }

    #[test]
    fn forecast_url_carries_coordinates_and_days() {
        let url = build_forecast_url(37.5665, 126.978, 5);
        assert!(url.starts_with("https://api.open-meteo.com/v1/forecast?"));
        assert!(url.contains("daily=weather_code,temperature_2m_max,temperature_2m_min"));
        assert!(url.contains("forecast_days=5"));
    }

    // `time` is always returned, never requested — naming it in `current=`
    // or `daily=` is rejected by Open-Meteo.
    #[test]
    fn request_field_lists_do_not_include_time() {
        assert!(!CurrentField::request_list().contains(CurrentField::Time.wire_name()));
        assert!(!DailyField::request_list().contains(DailyField::Time.wire_name()));
    }

    // ─── timezone ───────────────────────────────────────────────

    // Without `timezone=auto` Open-Meteo aggregates daily values over GMT
    // days, so a Seoul forecast's max would span 09:00→09:00 KST and the
    // first date could be the local yesterday — wrong, and silently so.
    #[test]
    fn forecast_url_requests_local_timezone_aggregation() {
        assert!(build_forecast_url(37.5665, 126.978, 5).contains("timezone=auto"));
    }

    #[test]
    fn current_weather_url_also_requests_local_timezone() {
        assert!(build_current_weather_url(37.5665, 126.978).contains("timezone=auto"));
    }

    // ─── weather_code_description ───────────────────────────────

    #[test]
    fn weather_code_maps_to_human_readable_description() {
        assert_eq!(weather_code_description(0), "Clear sky");
        assert_eq!(weather_code_description(95), "Thunderstorm");
    }

    #[test]
    fn unknown_weather_code_maps_to_unknown() {
        assert_eq!(weather_code_description(4242), WEATHER_CODE_UNKNOWN);
    }

    // ─── parse_geocoding ────────────────────────────────────────

    #[test]
    fn geocoding_response_parses_top_result() {
        let body = r#"{
            "results": [
                {"name": "Seoul", "country": "South Korea", "latitude": 37.5665, "longitude": 126.978},
                {"name": "Seoul", "country": "United States", "latitude": 0.0, "longitude": 0.0}
            ]
        }"#;
        let location = parse_geocoding(body).expect("parses top result");
        assert_eq!(
            location,
            GeoLocation {
                name: "Seoul".to_string(),
                country: "South Korea".to_string(),
                latitude: 37.5665,
                longitude: 126.978,
            }
        );
    }

    #[test]
    fn geocoding_result_missing_name_or_country_errors() {
        let missing_name =
            r#"{"results": [{"country": "South Korea", "latitude": 37.5, "longitude": 127.0}]}"#;
        let missing_country =
            r#"{"results": [{"name": "Seoul", "latitude": 37.5, "longitude": 127.0}]}"#;
        assert!(parse_geocoding(missing_name).is_err());
        assert!(parse_geocoding(missing_country).is_err());
    }

    #[test]
    fn geocoding_empty_results_error() {
        assert!(parse_geocoding(r#"{"results": []}"#).is_err());
        assert!(parse_geocoding(r#"{"generationtime_ms": 0.1}"#).is_err());
    }

    #[test]
    fn geocoding_invalid_json_errors() {
        assert!(parse_geocoding("not json").is_err());
    }

    // ─── parse_current_weather ──────────────────────────────────

    #[test]
    fn current_weather_response_parses() {
        let body = r#"{
            "current": {
                "time": "2026-07-17T10:00",
                "temperature_2m": 21.3,
                "relative_humidity_2m": 55,
                "weather_code": 2,
                "wind_speed_10m": 3.4
            }
        }"#;
        let current = parse_current_weather(body).expect("parses current weather");
        assert_eq!(
            current,
            CurrentWeather {
                observed_at: "2026-07-17T10:00".to_string(),
                temperature: 21.3,
                humidity: 55.0,
                weather_code: 2,
                wind_speed: 3.4,
            }
        );
    }

    #[test]
    fn current_weather_missing_fields_error() {
        let body = r#"{"current": {"temperature_2m": 21.3}}"#;
        assert!(parse_current_weather(body).is_err());
    }

    // ─── parse_daily_forecast ───────────────────────────────────

    #[test]
    fn daily_forecast_response_parses() {
        let body = r#"{
            "daily": {
                "time": ["2026-07-14", "2026-07-15"],
                "weather_code": [1, 61],
                "temperature_2m_max": [28.5, 26.0],
                "temperature_2m_min": [19.0, 20.1]
            }
        }"#;
        let entries = parse_daily_forecast(body).expect("parses daily forecast");
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[0],
            DailyForecast {
                date: "2026-07-14".to_string(),
                weather_code: 1,
                temperature_max: 28.5,
                temperature_min: 19.0,
            }
        );
        assert_eq!(entries[1].date, "2026-07-15");
        assert_eq!(entries[1].weather_code, 61);
    }

    #[test]
    fn daily_forecast_missing_daily_errors() {
        assert!(parse_daily_forecast(r#"{"latitude": 37.5}"#).is_err());
    }

    // Open-Meteo emits `null` for a value it doesn't have yet at the edge of
    // the horizon; one such day must not cost the caller every other day.
    #[test]
    fn null_valued_days_are_skipped_and_others_kept() {
        let body = r#"{
            "daily": {
                "time": ["2026-07-14", "2026-07-15", "2026-07-16"],
                "weather_code": [1, 61, 2],
                "temperature_2m_max": [28.5, null, 27.0],
                "temperature_2m_min": [19.0, 20.1, null]
            }
        }"#;
        let entries = parse_daily_forecast(body).expect("skips incomplete days");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].date, "2026-07-14");
    }

    #[test]
    fn shorter_arrays_skip_trailing_days() {
        let body = r#"{
            "daily": {
                "time": ["2026-07-14", "2026-07-15"],
                "weather_code": [1],
                "temperature_2m_max": [28.5],
                "temperature_2m_min": [19.0]
            }
        }"#;
        let entries = parse_daily_forecast(body).expect("skips days without values");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].date, "2026-07-14");
    }

    // ─── input validation (no network) ──────────────────────────

    #[test]
    fn empty_city_is_rejected_before_network_call() {
        assert_eq!(
            weather_lookup(""),
            Err("city must not be empty".to_string())
        );
        assert_eq!(
            weather_forecast("   ", 7),
            Err("city must not be empty".to_string())
        );
    }
}
