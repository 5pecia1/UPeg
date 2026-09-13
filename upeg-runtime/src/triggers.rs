#![allow(
    clippy::expect_used,
    reason = "infallible at construction (compile-time static ids) or fatal on failure (poisoned mutex)"
)]

pub mod schedule;

use std::str::FromStr;
use upeg_core::ToolId;

/// Diagnostic returned for a source string that does not name a known
/// [`TriggerSource`]. Kept as a named constant so the fallback text is not
/// duplicated across the delegating free functions.
const UNKNOWN_SOURCE_DIAGNOSTIC: &str = "unknown trigger source";

/// The closed set of trigger sources a tool may declare.
///
/// This enum is the single source of truth for source validation, built-in
/// runtime support, and host diagnostics. The `str`-based free functions below
/// delegate to it so existing callers (loader parse, CLI/HTTP surfaces) keep a
/// stable string-based API while the matching logic lives in exactly one place.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TriggerSource {
    Webhook,
    Schedule,
    File,
    Directory,
    Clipboard,
    Hotkey,
}

impl TriggerSource {
    /// Every source in declaration order (mirrors [`TRIGGER_SOURCES`]).
    pub const ALL: [Self; 6] = [
        Self::Webhook,
        Self::Schedule,
        Self::File,
        Self::Directory,
        Self::Clipboard,
        Self::Hotkey,
    ];

    /// The canonical wire string for this source.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Webhook => "webhook",
            Self::Schedule => "schedule",
            Self::File => "file",
            Self::Directory => "directory",
            Self::Clipboard => "clipboard",
            Self::Hotkey => "hotkey",
        }
    }

    /// Whether the trigger runtime ships a built-in adapter for this source.
    /// Host-bound sources (`hotkey`) require a platform adapter and therefore
    /// report `false`.
    pub const fn has_builtin_runtime(self) -> bool {
        matches!(
            self,
            Self::Webhook | Self::Schedule | Self::File | Self::Directory | Self::Clipboard
        )
    }

    /// What this source's `condition` field must carry.
    ///
    /// The closed set lives on the enum (beside [`Self::diagnostic`]) so the
    /// loader can validate every source with one total match instead of
    /// special-casing `schedule` and letting the rest through.
    pub const fn condition_requirement(self) -> TriggerConditionRequirement {
        match self {
            // Nothing to predicate on: the HTTP route *is* the condition, and
            // the clipboard gate watches the one system clipboard.
            Self::Webhook | Self::Clipboard => TriggerConditionRequirement::Forbidden,
            Self::Schedule => TriggerConditionRequirement::ScheduleExpression,
            Self::File => TriggerConditionRequirement::Required {
                expectation: "the watched file path",
            },
            Self::Directory => TriggerConditionRequirement::Required {
                expectation: "the watched directory path",
            },
            Self::Hotkey => TriggerConditionRequirement::Required {
                expectation: "an accelerator such as `ctrl+shift+u`",
            },
        }
    }

    /// A human-readable diagnostic describing how (or whether) this source is
    /// serviced on the current host.
    pub const fn diagnostic(self) -> &'static str {
        match self {
            Self::Webhook => "served by HTTP POST /v1/trigger/{tool_id}",
            Self::Schedule => {
                "supported by the CLI trigger runtime schedule gate (`now` or `every:<duration>`)"
            }
            Self::File => {
                "supported by the CLI trigger runtime path gate (fires on create and mtime change)"
            }
            Self::Directory => {
                "supported by the CLI trigger runtime path gate (fires on create and mtime change)"
            }
            Self::Clipboard => "supported when a host clipboard command is available",
            Self::Hotkey => {
                "requires a platform global-hotkey adapter; unsupported hosts report this diagnostic"
            }
        }
    }
}

/// What a [`TriggerSource`]'s `condition` field must carry.
///
/// Every source falls into exactly one of these, so a loader match over this
/// enum is total: no source can be added without deciding its condition
/// contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriggerConditionRequirement {
    /// The source carries no condition; declaring one is a load error.
    Forbidden,
    /// A `schedule` expression (`now` / `every:<duration>`). Optional: an
    /// absent condition means `now`.
    ScheduleExpression,
    /// A mandatory condition; `expectation` names what it must contain.
    Required { expectation: &'static str },
}

impl FromStr for TriggerSource {
    type Err = UnknownTriggerSource;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        match source.trim() {
            "webhook" => Ok(Self::Webhook),
            "schedule" => Ok(Self::Schedule),
            "file" => Ok(Self::File),
            "directory" => Ok(Self::Directory),
            "clipboard" => Ok(Self::Clipboard),
            "hotkey" => Ok(Self::Hotkey),
            other => Err(UnknownTriggerSource(other.to_string())),
        }
    }
}

/// Error returned when a string does not name a known [`TriggerSource`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownTriggerSource(pub String);

impl std::fmt::Display for UnknownTriggerSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown trigger source `{}`", self.0)
    }
}

impl std::error::Error for UnknownTriggerSource {}

/// Wire strings for every source in [`TriggerSource::ALL`], computed at compile
/// time so [`TRIGGER_SOURCES`] cannot drift from the enum.
const fn trigger_source_strs() -> [&'static str; TriggerSource::ALL.len()] {
    let mut out = [""; TriggerSource::ALL.len()];
    let mut i = 0;
    while i < TriggerSource::ALL.len() {
        out[i] = TriggerSource::ALL[i].as_str();
        i += 1;
    }
    out
}

/// Every valid trigger source as a wire string, in declaration order.
/// Derived from [`TriggerSource::ALL`] so the list cannot drift from the enum.
pub const TRIGGER_SOURCES: &[&str] = &trigger_source_strs();

pub fn is_valid_trigger_source(source: &str) -> bool {
    TriggerSource::from_str(source).is_ok()
}

pub fn trigger_source_has_builtin_runtime(source: &str) -> bool {
    TriggerSource::from_str(source)
        .map(TriggerSource::has_builtin_runtime)
        .unwrap_or(false)
}

pub fn trigger_source_diagnostic(source: &str) -> &'static str {
    TriggerSource::from_str(source)
        .map(TriggerSource::diagnostic)
        .unwrap_or(UNKNOWN_SOURCE_DIAGNOSTIC)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TriggerBinding {
    pub tool_id: &'static str,
    pub source: String,
    pub condition: Option<String>,
}

/// Separator between source and condition in a [`FiredTrigger`] label.
const TRIGGER_LABEL_SEPARATOR: char = ':';

/// Which of a tool's triggers fired a dispatch, as stamped into `_upeg.trigger`.
///
/// The tool id alone told the tool nothing it did not already know, and told it
/// nothing at all when the tool declares several triggers. This carries the
/// discriminating pair instead: the source, plus the condition that identifies
/// *which* binding of that source fired (which watched path, which schedule).
///
/// # Wire form
///
/// The label is a string — `source`, or `source:condition` — rather than a JSON
/// object, because `_upeg.trigger` is read as a string by the execution log's
/// `trigger` column and its `--trigger` filter
/// (`upeg-cli/src/adapters/execution_log.rs`); an object would silently drop the
/// trigger from every log record. A condition may itself contain `:`
/// (`schedule:every:30s`), so a consumer splitting the pair back apart splits on
/// the *first* separator only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FiredTrigger {
    source: TriggerSource,
    condition: Option<String>,
}

impl FiredTrigger {
    /// A fired trigger of `source`, discriminated by `condition`. A blank
    /// condition is normalized to `None`: it carries no information and would
    /// otherwise produce a dangling `source:` label.
    pub fn new(source: TriggerSource, condition: Option<String>) -> Self {
        Self {
            source,
            condition: condition
                .map(|condition| condition.trim().to_string())
                .filter(|condition| !condition.is_empty()),
        }
    }

    /// The fired trigger described by a registered binding.
    pub fn from_binding(binding: &TriggerBinding) -> Result<Self, UnknownTriggerSource> {
        Ok(Self::new(
            binding.source.parse()?,
            binding.condition.clone(),
        ))
    }

    pub const fn source(&self) -> TriggerSource {
        self.source
    }

    pub fn condition(&self) -> Option<&str> {
        self.condition.as_deref()
    }
}

impl std::fmt::Display for FiredTrigger {
    /// The `_upeg.trigger` label: `source`, or `source:condition`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.source.as_str())?;
        match &self.condition {
            Some(condition) => write!(f, "{TRIGGER_LABEL_SEPARATOR}{condition}"),
            None => Ok(()),
        }
    }
}

static TRIGGER_BINDINGS: std::sync::OnceLock<std::sync::Mutex<Vec<TriggerBinding>>> =
    std::sync::OnceLock::new();

fn trigger_bindings_lock() -> &'static std::sync::Mutex<Vec<TriggerBinding>> {
    TRIGGER_BINDINGS.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

fn canonical_tool_id(id: &str) {
    ToolId::parse_canonical(id).expect("tool id must be canonical and unpadded");
}

pub fn set_trigger_bindings(tool_id: &'static str, triggers: Vec<TriggerBinding>) {
    canonical_tool_id(tool_id);
    for trigger in &triggers {
        canonical_tool_id(trigger.tool_id);
        assert_eq!(
            trigger.tool_id, tool_id,
            "trigger binding tool_id must match the toolbox key"
        );
    }
    let mut guard = trigger_bindings_lock()
        .lock()
        .expect("trigger registry poisoned");
    guard.retain(|trigger| trigger.tool_id != tool_id);
    guard.extend(triggers);
}

pub fn trigger_bindings_for(tool_id: &str) -> Vec<TriggerBinding> {
    trigger_bindings_lock()
        .lock()
        .expect("trigger registry poisoned")
        .iter()
        .filter(|trigger| trigger.tool_id == tool_id)
        .cloned()
        .collect()
}

/// The `_upeg.trigger` label the HTTP webhook route stamps for `tool_id`, or
/// `None` when the tool declares no `webhook` trigger.
///
/// The route already refuses tools without a webhook binding, so `None` there
/// means the tool was unregistered between the guard and the dispatch.
pub fn webhook_trigger_label(tool_id: &str) -> Option<String> {
    trigger_bindings_for(tool_id)
        .iter()
        .find(|binding| binding.source.trim() == TriggerSource::Webhook.as_str())
        .map(|binding| {
            FiredTrigger::new(TriggerSource::Webhook, binding.condition.clone()).to_string()
        })
}

/// The `_upeg.trigger` label a *manual* fire (`upeg trigger fire <tool_id>`)
/// stamps for `tool_id`, or `None` when the tool declares no trigger.
///
/// `fire` names a Tool, not a binding, so it cannot know which of several
/// declared triggers the operator meant to simulate. The rule is therefore the
/// simplest one that stays honest: **the first declared binding wins**, and a
/// tool with no binding at all is stamped with nothing — `_upeg.trigger`
/// answers "which trigger started this call", and for such a tool the answer is
/// "none; a human did". Inventing a synthetic source here would put a value in
/// the execution log's `trigger` column that no Tool could ever declare.
pub fn manual_fire_trigger_label(tool_id: &str) -> Option<String> {
    trigger_bindings_for(tool_id)
        .iter()
        .find_map(|binding| FiredTrigger::from_binding(binding).ok())
        .map(|fired| fired.to_string())
}

pub fn registered_trigger_bindings() -> Vec<TriggerBinding> {
    let mut triggers = trigger_bindings_lock()
        .lock()
        .expect("trigger registry poisoned")
        .clone();
    triggers.sort_by(|a, b| {
        a.tool_id
            .cmp(b.tool_id)
            .then_with(|| a.source.cmp(&b.source))
            .then_with(|| a.condition.cmp(&b.condition))
    });
    triggers
}

pub fn trigger_bindings_json_for(tool_id: &str) -> Vec<serde_json::Value> {
    trigger_bindings_for(tool_id)
        .into_iter()
        .map(|trigger| {
            let has_runtime = trigger_source_has_builtin_runtime(&trigger.source);
            let diagnostic = trigger_source_diagnostic(&trigger.source);
            serde_json::json!({
                "toolId": trigger.tool_id,
                "source": trigger.source,
                "condition": trigger.condition,
                "runtimeSupported": has_runtime,
                "diagnostic": diagnostic,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_source는_문자열_왕복이_가능하다() {
        for source in TriggerSource::ALL {
            let round_tripped = TriggerSource::from_str(source.as_str());
            assert_eq!(round_tripped, Ok(source), "{source:?}는 왕복해야 한다");
        }
    }

    #[test]
    fn trigger_source_문자열_파싱은_공백을_허용한다() {
        assert_eq!(
            TriggerSource::from_str("  clipboard  "),
            Ok(TriggerSource::Clipboard)
        );
    }

    #[test]
    fn 알_수_없는_trigger_source는_에러를_반환한다() {
        let err = TriggerSource::from_str("mouse").unwrap_err();
        assert_eq!(err, UnknownTriggerSource("mouse".to_string()));
        assert!(!is_valid_trigger_source("mouse"));
        assert_eq!(
            trigger_source_diagnostic("mouse"),
            UNKNOWN_SOURCE_DIAGNOSTIC
        );
        assert!(!trigger_source_has_builtin_runtime("mouse"));
    }

    #[test]
    fn 은퇴한_typing_소스는_더_이상_유효하지_않다() {
        // `typing` never had a runtime adapter (it needed an OS accessibility
        // API that was never built), so it is no longer a declarable source.
        assert!(!is_valid_trigger_source("typing"));
        assert_eq!(
            TriggerSource::from_str("typing"),
            Err(UnknownTriggerSource("typing".to_string()))
        );
        assert!(!TRIGGER_SOURCES.contains(&"typing"));
    }

    #[test]
    fn 발화한_트리거_라벨은_소스와_조건을_모두_담는다() {
        let label =
            FiredTrigger::new(TriggerSource::File, Some("/tmp/drop.txt".into())).to_string();
        assert_eq!(label, "file:/tmp/drop.txt");
    }

    #[test]
    fn 조건이_없는_트리거_라벨은_소스만_담는다() {
        assert_eq!(
            FiredTrigger::new(TriggerSource::Clipboard, None).to_string(),
            "clipboard"
        );
        // A blank condition carries nothing; it must not leave a dangling `:`.
        assert_eq!(
            FiredTrigger::new(TriggerSource::Clipboard, Some("  ".into())).to_string(),
            "clipboard"
        );
    }

    #[test]
    fn 같은_도구의_두_트리거는_서로_다른_라벨을_갖는다() {
        // The tool id alone could not tell these apart — the whole point of the
        // label carrying the condition.
        let first = TriggerBinding {
            tool_id: "demo.tool",
            source: TriggerSource::File.as_str().to_string(),
            condition: Some("/tmp/one.txt".into()),
        };
        let second = TriggerBinding {
            tool_id: "demo.tool",
            source: TriggerSource::File.as_str().to_string(),
            condition: Some("/tmp/two.txt".into()),
        };
        let first = FiredTrigger::from_binding(&first).expect("file은 알려진 소스다");
        let second = FiredTrigger::from_binding(&second).expect("file은 알려진 소스다");
        assert_ne!(first.to_string(), second.to_string());
        assert_eq!(first.source(), TriggerSource::File);
        assert_eq!(second.condition(), Some("/tmp/two.txt"));
    }

    #[test]
    fn 조건이_콜론을_품어도_소스는_첫_구분자로_갈린다() {
        let label =
            FiredTrigger::new(TriggerSource::Schedule, Some("every:30s".into())).to_string();
        assert_eq!(label, "schedule:every:30s");
        assert_eq!(
            label.split_once(TRIGGER_LABEL_SEPARATOR),
            Some(("schedule", "every:30s"))
        );
    }

    #[test]
    fn 알_수_없는_소스의_바인딩은_라벨을_만들_수_없다() {
        let binding = TriggerBinding {
            tool_id: "demo.tool",
            source: "typing".to_string(),
            condition: None,
        };
        assert_eq!(
            FiredTrigger::from_binding(&binding),
            Err(UnknownTriggerSource("typing".to_string()))
        );
    }

    #[test]
    fn 모든_소스는_조건_요구사항을_선언한다() {
        use TriggerConditionRequirement as Requirement;
        assert_eq!(
            TriggerSource::Webhook.condition_requirement(),
            Requirement::Forbidden
        );
        assert_eq!(
            TriggerSource::Clipboard.condition_requirement(),
            Requirement::Forbidden
        );
        assert_eq!(
            TriggerSource::Schedule.condition_requirement(),
            Requirement::ScheduleExpression
        );
        for source in [
            TriggerSource::File,
            TriggerSource::Directory,
            TriggerSource::Hotkey,
        ] {
            assert!(
                matches!(source.condition_requirement(), Requirement::Required { .. }),
                "{source:?}는 조건이 필수여야 한다"
            );
        }
    }

    #[test]
    fn 수동_발화_라벨은_먼저_선언된_binding을_고른다() {
        // `upeg trigger fire` names a tool, not a binding, so the rule must be
        // deterministic: declaration order decides.
        const TOOL_ID: &str = "manualfire.first";
        set_trigger_bindings(
            TOOL_ID,
            vec![
                TriggerBinding {
                    tool_id: TOOL_ID,
                    source: TriggerSource::Schedule.as_str().to_string(),
                    condition: Some("every:30s".into()),
                },
                TriggerBinding {
                    tool_id: TOOL_ID,
                    source: TriggerSource::Clipboard.as_str().to_string(),
                    condition: None,
                },
            ],
        );
        assert_eq!(
            manual_fire_trigger_label(TOOL_ID).as_deref(),
            Some("schedule:every:30s")
        );
        set_trigger_bindings(TOOL_ID, Vec::new());
    }

    #[test]
    fn binding이_없는_도구는_수동_발화_라벨이_없다() {
        // Nothing declared means no trigger fired; the field stays absent
        // rather than carrying a source no tool could declare.
        assert_eq!(manual_fire_trigger_label("manualfire.bare"), None);
    }

    #[test]
    fn trigger_sources_상수는_enum과_동일한_순서다() {
        let from_enum: Vec<&str> = TriggerSource::ALL.iter().map(|s| s.as_str()).collect();
        assert_eq!(TRIGGER_SOURCES, from_enum.as_slice());
    }
}
