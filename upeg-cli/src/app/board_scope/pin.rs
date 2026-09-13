//! `upeg board <board> pin|unpin|move` — headless pegboard editing.
//!
//! These are the CLI half of the pin lifecycle whose GUI half lives in
//! Desktop's `p` key and drag-drop. They deliberately write through
//! [`upeg_sources::pegboard`] — the same facade over the same SQLite
//! store — rather than growing a CLI-only placement path, so a pin made
//! here is byte-identical to one made by a drag: same reconcile, same
//! push-forward collision rule, same `state_change_rev` bump that makes
//! a running Desktop/TUI pick the change up.
//!
//! Everything that can go wrong is a variant of [`BoardPinError`]
//! instead of a formatted string, so the four call sites cannot invent
//! four different wordings for "that tool is not pinned here".

use upeg_core::{BoardKey, PegboardUnits, PinSpan, Placement, Surface};
use upeg_sources::pegboard::{self, PegboardState};

use crate::error::CliError;

use super::super::normalize_tool_id_segments;

/// Separator between the two coordinates of `--at`.
const CELL_SEPARATOR: char = ',';
/// How many coordinates `--at` carries.
const CELL_FIELD_COUNT: usize = 2;

/// A zero-based cell on the canonical pegboard grid, in the order the
/// CLI spells it: `--at <row>,<col>`.
///
/// Storage is `(x, y)` = `(col, row)`, so this type exists purely to
/// stop the two orders being mixed up at a call site — the conversion
/// happens once, in [`Self::x`] / [`Self::y`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct BoardCell {
    row: u16,
    col: u16,
}

impl BoardCell {
    const fn x(self) -> u16 {
        self.col
    }

    const fn y(self) -> u16 {
        self.row
    }

    fn of(placement: &Placement) -> Self {
        Self {
            row: placement.y,
            col: placement.x,
        }
    }

    fn parse(raw: &str) -> Result<Self, BoardPinError> {
        let invalid = || BoardPinError::InvalidCell {
            value: raw.to_string(),
        };
        let parts: Vec<&str> = raw.split(CELL_SEPARATOR).collect();
        if parts.len() != CELL_FIELD_COUNT {
            return Err(invalid());
        }
        let row = parts[0].trim().parse::<u16>().map_err(|_| invalid())?;
        let col = parts[1].trim().parse::<u16>().map_err(|_| invalid())?;
        Ok(Self { row, col })
    }
}

impl std::fmt::Display for BoardCell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "row {}, col {}", self.row, self.col)
    }
}

fn parse_units(raw: &str) -> Result<PinSpan, BoardPinError> {
    let units = PegboardUnits::parse(raw.trim()).ok_or_else(|| BoardPinError::InvalidUnits {
        value: raw.to_string(),
    })?;
    let (cols, rows) = units.grid_span();
    // `PegboardUnits` only ever yields spans inside the canonical grid,
    // so the newtype constructors cannot fail here; map the error
    // anyway rather than unwrap, keeping this file panic-free.
    let cols = upeg_core::ColSpan::new(cols).map_err(|_| BoardPinError::InvalidUnits {
        value: raw.to_string(),
    })?;
    let rows = upeg_core::RowSpan::new(rows).map_err(|_| BoardPinError::InvalidUnits {
        value: raw.to_string(),
    })?;
    Ok(PinSpan::new(cols, rows))
}

/// Everything `pin` / `unpin` / `move` can refuse to do.
#[derive(Debug, thiserror::Error)]
pub(super) enum BoardPinError {
    #[error("unknown tool `{tool_id}` — `upeg tool list` shows every registered id")]
    UnknownTool { tool_id: String },
    #[error("tool `{tool_id}` is already pinned on board `{board}`")]
    AlreadyPinned { board: BoardKey, tool_id: String },
    #[error("tool `{tool_id}` is not pinned on board `{board}`")]
    NotPinned { board: BoardKey, tool_id: String },
    #[error("`--at {value}` must be `<row>,<col>` with non-negative whole numbers")]
    InvalidCell { value: String },
    #[error("`--units {value}` must be one of U1, U2, U2T")]
    InvalidUnits { value: String },
    #[error(
        "tool `{tool_id}` does not fit at {cell} on board `{board}` (the canonical board is {cols} columns wide)"
    )]
    CellUnavailable {
        board: BoardKey,
        tool_id: String,
        cell: BoardCell,
        cols: u16,
    },
    #[error("could not place tool `{tool_id}` on board `{board}`")]
    PinRefused { board: BoardKey, tool_id: String },
    #[error("save pegboard state: {0}")]
    Save(#[from] pegboard::PegboardStateError),
}

impl From<BoardPinError> for CliError {
    fn from(error: BoardPinError) -> Self {
        Self::tool_failed(error.to_string())
    }
}

/// What a completed edit did, for rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PinVerb {
    Pinned,
    Unpinned,
    Moved,
}

impl PinVerb {
    const fn label(self) -> &'static str {
        match self {
            Self::Pinned => "pinned",
            Self::Unpinned => "unpinned",
            Self::Moved => "moved",
        }
    }
}

pub(super) fn run_pin(
    state: &mut PegboardState,
    board: &BoardKey,
    tool_id: &str,
    units: Option<&str>,
    at: Option<&str>,
    json: bool,
) -> Result<String, CliError> {
    let tool_id = normalize_tool_id_segments(tool_id);
    if upeg_runtime::toolbox_tool(&tool_id).is_none() {
        return Err(BoardPinError::UnknownTool { tool_id }.into());
    }
    if pegboard::placement_in(state, board.as_str(), &tool_id).is_some() {
        return Err(BoardPinError::AlreadyPinned {
            board: board.clone(),
            tool_id,
        }
        .into());
    }
    // Parse both options before mutating so a typo cannot leave a pin
    // half-configured in the store.
    let span = units.map(parse_units).transpose()?;
    let cell = at.map(BoardCell::parse).transpose()?;

    if pegboard::pin_tool(state, board.as_str(), &tool_id) != pegboard::PinAction::Pinned {
        return Err(BoardPinError::PinRefused {
            board: board.clone(),
            tool_id,
        }
        .into());
    }
    if let Some(span) = span {
        pegboard::set_pin_span(state, board.as_str(), &tool_id, Some(span));
    }
    if let Some(cell) = cell {
        apply_cell(state, board, &tool_id, cell)?;
    }
    pegboard::save_state(state).map_err(BoardPinError::Save)?;
    Ok(render(state, board, &tool_id, PinVerb::Pinned, json))
}

pub(super) fn run_unpin(
    state: &mut PegboardState,
    board: &BoardKey,
    tool_id: &str,
    json: bool,
) -> Result<String, CliError> {
    let tool_id = normalize_tool_id_segments(tool_id);
    if pegboard::placement_in(state, board.as_str(), &tool_id).is_none() {
        return Err(BoardPinError::NotPinned {
            board: board.clone(),
            tool_id,
        }
        .into());
    }
    pegboard::unpin_tool(state, board.as_str(), &tool_id);
    pegboard::save_state(state).map_err(BoardPinError::Save)?;
    Ok(render(state, board, &tool_id, PinVerb::Unpinned, json))
}

pub(super) fn run_move(
    state: &mut PegboardState,
    board: &BoardKey,
    tool_id: &str,
    at: &str,
    json: bool,
) -> Result<String, CliError> {
    let tool_id = normalize_tool_id_segments(tool_id);
    if pegboard::placement_in(state, board.as_str(), &tool_id).is_none() {
        return Err(BoardPinError::NotPinned {
            board: board.clone(),
            tool_id,
        }
        .into());
    }
    let cell = BoardCell::parse(at)?;
    apply_cell(state, board, &tool_id, cell)?;
    pegboard::save_state(state).map_err(BoardPinError::Save)?;
    Ok(render(state, board, &tool_id, PinVerb::Moved, json))
}

/// Move an existing pin to `cell`, treating "already there" as done.
///
/// `place_tool_with_push` reports `NoOp` both for "the layout is
/// unchanged" and for "that cell cannot hold this pin"; only the second
/// is an error, so the current coordinates are checked first.
fn apply_cell(
    state: &mut PegboardState,
    board: &BoardKey,
    tool_id: &str,
    cell: BoardCell,
) -> Result<(), BoardPinError> {
    let already_there = pegboard::placement_in(state, board.as_str(), tool_id)
        .is_some_and(|placement| BoardCell::of(placement) == cell);
    if already_there {
        return Ok(());
    }
    if pegboard::move_placement(state, board.as_str(), tool_id, cell.x(), cell.y()) {
        return Ok(());
    }
    Err(BoardPinError::CellUnavailable {
        board: board.clone(),
        tool_id: tool_id.to_string(),
        cell,
        cols: upeg_core::BOARD_COLS,
    })
}

/// One-line text, or a JSON object with the post-edit placement.
///
/// `visibleOnCli` is derived HERE, once, from the toolbox — every verb
/// gets the same answer. It used to ride in as a parameter, and the
/// unpin path passed a hard-coded `true`: `upeg board <b> unpin --json`
/// reported a desktop-only tool as CLI-visible, which is the one thing
/// the field exists to deny.
fn render(
    state: &PegboardState,
    board: &BoardKey,
    tool_id: &str,
    verb: PinVerb,
    json: bool,
) -> String {
    let placement = pegboard::placement_in(state, board.as_str(), tool_id);
    let on_cli =
        upeg_runtime::toolbox_tool(tool_id).is_some_and(|tool| tool.is_on_surface(Surface::Cli));
    if json {
        let cell = placement.map(BoardCell::of);
        let value = serde_json::json!({
            "board": board.as_str(),
            "tool": tool_id,
            "action": verb.label(),
            "cell": cell.map(|cell| serde_json::json!({ "row": cell.row, "col": cell.col })),
            "span": placement.and_then(|p| p.span).map(|span| {
                serde_json::json!({ "cols": span.cols.get(), "rows": span.rows.get() })
            }),
            "visibleOnCli": on_cli,
        });
        let mut out = serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string());
        out.push('\n');
        return out;
    }
    let mut out = match placement {
        Some(placement) => format!(
            "{} {tool_id} on board {board} at {}\n",
            verb.label(),
            BoardCell::of(placement)
        ),
        None => format!("{} {tool_id} from board {board}\n", verb.label()),
    };
    // Only a pin that still exists can be invisible to `list`/`call`;
    // after an unpin there is nothing left to warn about.
    if !on_cli && placement.is_some() {
        out.push_str(&format!(
            "note: {tool_id} is not registered on the cli surface, so `upeg board {board} list` \
             and `upeg board {board} call` will not show it\n"
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 셀은_행_열_순서로_파싱된다() {
        let cell = BoardCell::parse("2,3").expect("셀 파싱");
        assert_eq!(cell.y(), 2, "첫 값은 행이다");
        assert_eq!(cell.x(), 3, "둘째 값은 열이다");
    }

    #[test]
    fn 셀은_공백을_허용한다() {
        assert_eq!(
            BoardCell::parse(" 1 , 4 ").expect("셀 파싱"),
            BoardCell { row: 1, col: 4 }
        );
    }

    #[test]
    fn 잘못된_셀은_거부된다() {
        for raw in ["", "1", "1,2,3", "a,b", "-1,0", "1;2"] {
            assert!(
                matches!(
                    BoardCell::parse(raw),
                    Err(BoardPinError::InvalidCell { .. })
                ),
                "{raw:?}는 거부되어야 한다"
            );
        }
    }

    /// The board id every render fixture below edits. Never persisted —
    /// `render` only reads the in-memory state it is handed.
    const RENDER_FIXTURE_BOARD: &str = "pin-render-fixture";
    const DESKTOP_ONLY_TOOL: &str = "zz_pin_render.desktop_only";

    fn desktop_only_tool() -> upeg_runtime::ToolboxRegistration {
        upeg_runtime::toolbox_add_tool_managed(upeg_core::ToolMeta {
            id: DESKTOP_ONLY_TOOL,
            toolkit: "zz_pin_render",
            local_id: "desktop_only",
            tags: &[],
            display_label: "Desktop only render fixture",
            description: "board pin render fixture",
            input_spec: upeg_core::InputSpec::empty(),
            output_spec: upeg_core::OutputSpec::empty(),
            primary_output_id: None,
            source: upeg_core::Source::UserInput,
            pin: upeg_core::PinKind::Inline,
            pegboard_units: upeg_core::PegboardUnits::U1,
            invoker: upeg_core::Invoker::Function,
            surfaces: &[Surface::Desktop],
            boards: &[],
        })
    }

    fn board_with_no_pins() -> (PegboardState, BoardKey) {
        let mut state = pegboard::default_state();
        let board = BoardKey::parse(RENDER_FIXTURE_BOARD).expect("board key");
        state.boards.push(pegboard::BoardData {
            key: RENDER_FIXTURE_BOARD.to_string(),
            title: "Pin render fixture".to_string(),
            guidance: upeg_core::BoardGuidance::default(),
        });
        state
            .layouts
            .insert(RENDER_FIXTURE_BOARD.to_string(), Vec::new());
        (state, board)
    }

    /// `unpin --json` used to hard-code `visibleOnCli: true`, so a
    /// desktop-only tool was reported as callable from the CLI. Every
    /// verb must derive the same answer from the toolbox.
    #[test]
    fn unpin_json은_cli에_없는_도구를_visible로_보고하지_않는다() {
        let _guard = desktop_only_tool();
        let (state, board) = board_with_no_pins();

        let rendered = render(&state, &board, DESKTOP_ONLY_TOOL, PinVerb::Unpinned, true);
        let value: serde_json::Value = serde_json::from_str(&rendered).expect("json");

        assert_eq!(
            value["visibleOnCli"],
            serde_json::Value::Bool(false),
            "desktop 전용 도구는 cli 에서 보이지 않는다: {rendered}"
        );
        assert_eq!(value["action"], PinVerb::Unpinned.label());
    }

    /// The text rendering's counterpart: an unpin leaves no pin behind,
    /// so the "this pin will not show up" note has nothing to describe.
    #[test]
    fn unpin_텍스트는_핀이_사라진_뒤_가시성_주의를_붙이지_않는다() {
        let _guard = desktop_only_tool();
        let (state, board) = board_with_no_pins();

        let rendered = render(&state, &board, DESKTOP_ONLY_TOOL, PinVerb::Unpinned, false);

        assert!(
            !rendered.contains("not registered on the cli surface"),
            "사라진 핀에 대해 가시성 주의를 붙이면 안 된다: {rendered}"
        );
    }

    #[test]
    fn units는_pegboard_units를_그대로_따른다() {
        assert_eq!(parse_units("U2").expect("U2").grid_span(), (2, 1));
        assert_eq!(parse_units("u2t").expect("U2T").grid_span(), (1, 2));
        assert!(matches!(
            parse_units("U9"),
            Err(BoardPinError::InvalidUnits { .. })
        ));
    }
}
